//! Tag binding API operations for workspaces and projects

use log::debug;

use crate::config::api;
use crate::error::{Result, TfeError};
use crate::hcp::TfeClient;

use super::models::{OrgTag, TagBinding, TagBindingsResponse};
use crate::hcp::traits::ApiListResponse;

/// Target type for tag operations
#[derive(Debug, Clone)]
pub enum TagTargetKind {
    Workspace,
    Project,
}

/// Resolved target for tag operations
#[derive(Debug)]
pub struct TagTarget {
    pub kind: TagTargetKind,
    pub id: String,
    pub display_name: String,
}

impl TfeClient {
    /// Get tag bindings for a workspace or project
    pub async fn get_tag_bindings(&self, target: &TagTarget) -> Result<Vec<TagBinding>> {
        let resource_path = match target.kind {
            TagTargetKind::Workspace => api::WORKSPACES,
            TagTargetKind::Project => api::PROJECTS,
        };

        let url = format!(
            "{}/{}/{}/tag-bindings",
            self.base_url(),
            resource_path,
            target.id
        );

        debug!("Fetching tag bindings from: {}", url);

        let response = self.get(&url).send().await?;

        match response.status().as_u16() {
            200 => {
                let resp: TagBindingsResponse = response.json().await?;
                Ok(resp.data)
            }
            404 => Err(TfeError::Api {
                status: 404,
                message: format!(
                    "{} '{}' not found",
                    match target.kind {
                        TagTargetKind::Workspace => "Workspace",
                        TagTargetKind::Project => "Project",
                    },
                    target.display_name
                ),
            }),
            status => {
                let body = response.text().await.unwrap_or_default();
                Err(TfeError::Api {
                    status,
                    message: format!(
                        "Failed to fetch tag bindings for '{}': {}",
                        target.display_name, body
                    ),
                })
            }
        }
    }

    /// Add or update tag bindings (additive PATCH)
    pub async fn add_tag_bindings(
        &self,
        target: &TagTarget,
        tags: &[(String, String)],
    ) -> Result<Vec<TagBinding>> {
        let resource_path = match target.kind {
            TagTargetKind::Workspace => api::WORKSPACES,
            TagTargetKind::Project => api::PROJECTS,
        };

        let url = format!(
            "{}/{}/{}/tag-bindings",
            self.base_url(),
            resource_path,
            target.id
        );

        debug!(
            "Adding {} tag binding(s) to '{}': {}",
            tags.len(),
            target.display_name,
            url
        );

        let data: Vec<serde_json::Value> = tags
            .iter()
            .map(|(key, value)| {
                serde_json::json!({
                    "type": "tag-bindings",
                    "attributes": {
                        "key": key,
                        "value": value
                    }
                })
            })
            .collect();

        let body = serde_json::json!({ "data": data });

        let response = self.patch(&url).json(&body).send().await?;

        match response.status().as_u16() {
            200 => {
                let resp: TagBindingsResponse = response.json().await?;
                Ok(resp.data)
            }
            404 => Err(TfeError::Api {
                status: 404,
                message: format!(
                    "{} '{}' not found",
                    match target.kind {
                        TagTargetKind::Workspace => "Workspace",
                        TagTargetKind::Project => "Project",
                    },
                    target.display_name
                ),
            }),
            422 => {
                let body = response.text().await.unwrap_or_default();
                Err(TfeError::Api {
                    status: 422,
                    message: format!("Cannot set tags on '{}': {}", target.display_name, body),
                })
            }
            status => {
                let body = response.text().await.unwrap_or_default();
                Err(TfeError::Api {
                    status,
                    message: format!("Failed to set tags on '{}': {}", target.display_name, body),
                })
            }
        }
    }

    /// Remove specific tag bindings by replacing with filtered set
    ///
    /// This works by:
    /// 1. Fetching current tag bindings
    /// 2. Filtering out the specified keys
    /// 3. Replacing all tag bindings on the resource
    pub async fn remove_tag_bindings(
        &self,
        target: &TagTarget,
        keys_to_remove: &[String],
    ) -> Result<Vec<TagBinding>> {
        // 1. Get current tags
        let current_tags = self.get_tag_bindings(target).await?;

        // Check which keys actually exist
        let missing_keys: Vec<&str> = keys_to_remove
            .iter()
            .filter(|k| !current_tags.iter().any(|t| t.attributes.key == **k))
            .map(|k| k.as_str())
            .collect();

        if !missing_keys.is_empty() {
            return Err(TfeError::Api {
                status: 404,
                message: format!(
                    "Tag key(s) not found on '{}': {}",
                    target.display_name,
                    missing_keys.join(", ")
                ),
            });
        }

        // 2. Filter out the keys to remove
        let remaining_tags: Vec<serde_json::Value> = current_tags
            .iter()
            .filter(|t| !keys_to_remove.contains(&t.attributes.key))
            .map(|t| {
                serde_json::json!({
                    "type": "tag-bindings",
                    "attributes": {
                        "key": t.attributes.key,
                        "value": t.attributes.value
                    }
                })
            })
            .collect();

        // 3. Replace all tags via resource update
        let resource_path = match target.kind {
            TagTargetKind::Workspace => api::WORKSPACES,
            TagTargetKind::Project => api::PROJECTS,
        };
        let resource_type = match target.kind {
            TagTargetKind::Workspace => "workspaces",
            TagTargetKind::Project => "projects",
        };

        let url = format!("{}/{}/{}", self.base_url(), resource_path, target.id);

        debug!(
            "Replacing tag bindings on '{}' (removing {}): {}",
            target.display_name,
            keys_to_remove.join(", "),
            url
        );

        let body = serde_json::json!({
            "data": {
                "type": resource_type,
                "relationships": {
                    "tag-bindings": {
                        "data": remaining_tags
                    }
                }
            }
        });

        let response = self.patch(&url).json(&body).send().await?;

        match response.status().as_u16() {
            200 => {
                // Fetch the updated tag bindings
                self.get_tag_bindings(target).await
            }
            404 => Err(TfeError::Api {
                status: 404,
                message: format!(
                    "{} '{}' not found",
                    match target.kind {
                        TagTargetKind::Workspace => "Workspace",
                        TagTargetKind::Project => "Project",
                    },
                    target.display_name
                ),
            }),
            status => {
                let body = response.text().await.unwrap_or_default();
                Err(TfeError::Api {
                    status,
                    message: format!(
                        "Failed to remove tags from '{}': {}",
                        target.display_name, body
                    ),
                })
            }
        }
    }

    /// Get flat string tags for a workspace (via relationships/tags endpoint)
    pub async fn get_workspace_tags(&self, workspace_id: &str) -> Result<Vec<OrgTag>> {
        let url = format!(
            "{}/{}/{}/relationships/tags",
            self.base_url(),
            api::WORKSPACES,
            workspace_id
        );

        debug!("Fetching workspace flat string tags from: {}", url);

        let response = self.get(&url).send().await?;

        match response.status().as_u16() {
            200 => {
                let resp: ApiListResponse<OrgTag> = response.json().await?;
                Ok(resp.data)
            }
            404 => Err(TfeError::Api {
                status: 404,
                message: format!("Workspace '{}' not found", workspace_id),
            }),
            status => {
                let body = response.text().await.unwrap_or_default();
                Err(TfeError::Api {
                    status,
                    message: format!(
                        "Failed to fetch tags for workspace '{}': {}",
                        workspace_id, body
                    ),
                })
            }
        }
    }

    /// Get all organization-level tags (paginated)
    pub async fn get_org_tags(
        &self,
        org: &str,
        search: Option<&str>,
    ) -> crate::error::Result<Vec<OrgTag>> {
        let mut path = format!("/{}/{}/tags", api::ORGANIZATIONS, org);
        if let Some(q) = search {
            path = format!("{}?q={}", path, urlencoding::encode(q));
        }
        let error_context = format!("tags for organization '{}'", org);

        self.fetch_all_pages::<OrgTag, ApiListResponse<OrgTag>>(&path, &error_context)
            .await
    }

    /// Add flat string tags to a workspace (POST /workspaces/:id/relationships/tags)
    pub async fn add_workspace_tags(&self, workspace_id: &str, tag_names: &[String]) -> Result<()> {
        let url = format!(
            "{}/{}/{}/relationships/tags",
            self.base_url(),
            api::WORKSPACES,
            workspace_id
        );

        debug!(
            "Adding {} flat string tag(s) to workspace '{}': {}",
            tag_names.len(),
            workspace_id,
            url
        );

        let data: Vec<serde_json::Value> = tag_names
            .iter()
            .map(|name| {
                serde_json::json!({
                    "type": "tags",
                    "attributes": {
                        "name": name
                    }
                })
            })
            .collect();

        let body = serde_json::json!({ "data": data });

        let response = self.post(&url).json(&body).send().await?;

        match response.status().as_u16() {
            204 => Ok(()),
            404 => Err(TfeError::Api {
                status: 404,
                message: format!("Workspace '{}' not found", workspace_id),
            }),
            status => {
                let body = response.text().await.unwrap_or_default();
                Err(TfeError::Api {
                    status,
                    message: format!(
                        "Failed to add tags to workspace '{}': {}",
                        workspace_id, body
                    ),
                })
            }
        }
    }

    /// Remove flat string tags from a workspace (DELETE /workspaces/:id/relationships/tags)
    pub async fn remove_workspace_tags(
        &self,
        workspace_id: &str,
        tag_names: &[String],
    ) -> Result<()> {
        let url = format!(
            "{}/{}/{}/relationships/tags",
            self.base_url(),
            api::WORKSPACES,
            workspace_id
        );

        debug!(
            "Removing {} flat string tag(s) from workspace '{}': {}",
            tag_names.len(),
            workspace_id,
            url
        );

        let data: Vec<serde_json::Value> = tag_names
            .iter()
            .map(|name| {
                serde_json::json!({
                    "type": "tags",
                    "attributes": {
                        "name": name
                    }
                })
            })
            .collect();

        let body = serde_json::json!({ "data": data });

        let response = self.delete(&url).json(&body).send().await?;

        match response.status().as_u16() {
            204 => Ok(()),
            404 => Err(TfeError::Api {
                status: 404,
                message: format!("Workspace '{}' not found", workspace_id),
            }),
            status => {
                let body = response.text().await.unwrap_or_default();
                Err(TfeError::Api {
                    status,
                    message: format!(
                        "Failed to remove tags from workspace '{}': {}",
                        workspace_id, body
                    ),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn tag_bindings_response(tags: Vec<(&str, &str)>) -> serde_json::Value {
        let data: Vec<serde_json::Value> = tags
            .iter()
            .enumerate()
            .map(|(i, (key, value))| {
                serde_json::json!({
                    "id": format!("tb-{}", i),
                    "type": "tag-bindings",
                    "attributes": {
                        "key": key,
                        "value": value
                    }
                })
            })
            .collect();
        serde_json::json!({ "data": data })
    }

    fn ws_target(id: &str, name: &str) -> TagTarget {
        TagTarget {
            kind: TagTargetKind::Workspace,
            id: id.to_string(),
            display_name: name.to_string(),
        }
    }

    fn prj_target(id: &str, name: &str) -> TagTarget {
        TagTarget {
            kind: TagTargetKind::Project,
            id: id.to_string(),
            display_name: name.to_string(),
        }
    }

    // ===== Get tag bindings tests =====

    #[tokio::test]
    async fn test_get_workspace_tag_bindings() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-abc123/tag-bindings"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(tag_bindings_response(vec![
                    ("env", "prod"),
                    ("team", "backend"),
                ])),
            )
            .mount(&mock_server)
            .await;

        let target = ws_target("ws-abc123", "my-workspace");
        let result = client.get_tag_bindings(&target).await;

        assert!(result.is_ok());
        let tags = result.unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].attributes.key, "env");
        assert_eq!(tags[0].attributes.value, "prod");
        assert_eq!(tags[1].attributes.key, "team");
        assert_eq!(tags[1].attributes.value, "backend");
    }

    #[tokio::test]
    async fn test_get_project_tag_bindings() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/projects/prj-abc123/tag-bindings"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(tag_bindings_response(vec![("env", "staging")])),
            )
            .mount(&mock_server)
            .await;

        let target = prj_target("prj-abc123", "my-project");
        let result = client.get_tag_bindings(&target).await;

        assert!(result.is_ok());
        let tags = result.unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].attributes.key, "env");
    }

    #[tokio::test]
    async fn test_get_tag_bindings_empty() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-abc123/tag-bindings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": []})))
            .mount(&mock_server)
            .await;

        let target = ws_target("ws-abc123", "my-workspace");
        let result = client.get_tag_bindings(&target).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_get_tag_bindings_not_found() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-notfound/tag-bindings"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let target = ws_target("ws-notfound", "missing-ws");
        let result = client.get_tag_bindings(&target).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            TfeError::Api { status, message } => {
                assert_eq!(status, 404);
                assert!(message.contains("missing-ws"));
            }
            _ => panic!("Expected TfeError::Api"),
        }
    }

    #[tokio::test]
    async fn test_get_tag_bindings_server_error() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-abc123/tag-bindings"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock_server)
            .await;

        let target = ws_target("ws-abc123", "my-workspace");
        let result = client.get_tag_bindings(&target).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            TfeError::Api { status, .. } => assert_eq!(status, 500),
            _ => panic!("Expected TfeError::Api"),
        }
    }

    // ===== Add tag bindings tests =====

    #[tokio::test]
    async fn test_add_workspace_tag_bindings() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        let expected_body = serde_json::json!({
            "data": [
                {
                    "type": "tag-bindings",
                    "attributes": { "key": "env", "value": "prod" }
                },
                {
                    "type": "tag-bindings",
                    "attributes": { "key": "team", "value": "backend" }
                }
            ]
        });

        Mock::given(method("PATCH"))
            .and(path("/workspaces/ws-abc123/tag-bindings"))
            .and(body_json(&expected_body))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(tag_bindings_response(vec![
                    ("env", "prod"),
                    ("team", "backend"),
                ])),
            )
            .mount(&mock_server)
            .await;

        let target = ws_target("ws-abc123", "my-workspace");
        let tags = vec![
            ("env".to_string(), "prod".to_string()),
            ("team".to_string(), "backend".to_string()),
        ];
        let result = client.add_tag_bindings(&target, &tags).await;

        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.len(), 2);
    }

    #[tokio::test]
    async fn test_add_project_tag_bindings() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("PATCH"))
            .and(path("/projects/prj-abc123/tag-bindings"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(tag_bindings_response(vec![("env", "staging")])),
            )
            .mount(&mock_server)
            .await;

        let target = prj_target("prj-abc123", "my-project");
        let tags = vec![("env".to_string(), "staging".to_string())];
        let result = client.add_tag_bindings(&target, &tags).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_add_tag_bindings_not_found() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("PATCH"))
            .and(path("/workspaces/ws-notfound/tag-bindings"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let target = ws_target("ws-notfound", "missing-ws");
        let tags = vec![("env".to_string(), "prod".to_string())];
        let result = client.add_tag_bindings(&target, &tags).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            TfeError::Api { status, .. } => assert_eq!(status, 404),
            _ => panic!("Expected TfeError::Api"),
        }
    }

    #[tokio::test]
    async fn test_add_tag_bindings_unprocessable() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("PATCH"))
            .and(path("/workspaces/ws-abc123/tag-bindings"))
            .respond_with(ResponseTemplate::new(422).set_body_string("Validation failed"))
            .mount(&mock_server)
            .await;

        let target = ws_target("ws-abc123", "my-workspace");
        let tags = vec![("hc:reserved".to_string(), "value".to_string())];
        let result = client.add_tag_bindings(&target, &tags).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            TfeError::Api { status, message } => {
                assert_eq!(status, 422);
                assert!(message.contains("my-workspace"));
            }
            _ => panic!("Expected TfeError::Api"),
        }
    }

    // ===== Remove tag bindings tests =====

    #[tokio::test]
    async fn test_remove_workspace_tag_bindings() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        // Step 1: GET current tags
        Mock::given(method("GET"))
            .and(path("/workspaces/ws-abc123/tag-bindings"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(tag_bindings_response(vec![
                    ("env", "prod"),
                    ("team", "backend"),
                    ("region", "us-east"),
                ])),
            )
            .expect(2) // Called twice: once to get current, once after update
            .mount(&mock_server)
            .await;

        // Step 2: PATCH workspace with remaining tags (full replacement)
        let expected_body = serde_json::json!({
            "data": {
                "type": "workspaces",
                "relationships": {
                    "tag-bindings": {
                        "data": [
                            {
                                "type": "tag-bindings",
                                "attributes": { "key": "env", "value": "prod" }
                            },
                            {
                                "type": "tag-bindings",
                                "attributes": { "key": "region", "value": "us-east" }
                            }
                        ]
                    }
                }
            }
        });

        Mock::given(method("PATCH"))
            .and(path("/workspaces/ws-abc123"))
            .and(body_json(&expected_body))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "id": "ws-abc123",
                    "type": "workspaces",
                    "attributes": { "name": "my-workspace" }
                }
            })))
            .mount(&mock_server)
            .await;

        let target = ws_target("ws-abc123", "my-workspace");
        let result = client
            .remove_tag_bindings(&target, &["team".to_string()])
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_remove_tag_bindings_key_not_found() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-abc123/tag-bindings"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(tag_bindings_response(vec![("env", "prod")])),
            )
            .mount(&mock_server)
            .await;

        let target = ws_target("ws-abc123", "my-workspace");
        let result = client
            .remove_tag_bindings(&target, &["nonexistent".to_string()])
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            TfeError::Api { status, message } => {
                assert_eq!(status, 404);
                assert!(message.contains("nonexistent"));
            }
            _ => panic!("Expected TfeError::Api"),
        }
    }

    // ===== Organization-level tags tests =====

    fn org_tags_response(tags: Vec<(&str, u32)>) -> serde_json::Value {
        let data: Vec<serde_json::Value> = tags
            .iter()
            .enumerate()
            .map(|(i, (name, count))| {
                serde_json::json!({
                    "id": format!("tag-{}", i),
                    "type": "tags",
                    "attributes": {
                        "name": name,
                        "instance-count": count,
                        "created-at": "2024-01-01T00:00:00Z"
                    }
                })
            })
            .collect();
        serde_json::json!({
            "data": data,
            "meta": {
                "pagination": {
                    "current-page": 1,
                    "total-pages": 1,
                    "total-count": data.len()
                }
            }
        })
    }

    #[tokio::test]
    async fn test_get_org_tags() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/tags"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(org_tags_response(vec![("env", 5), ("team", 3)])),
            )
            .mount(&mock_server)
            .await;

        let result = client.get_org_tags("my-org", None).await;
        assert!(result.is_ok());
        let tags = result.unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].attributes.name, "env");
        assert_eq!(tags[0].attributes.instance_count, 5);
        assert_eq!(tags[1].attributes.name, "team");
        assert_eq!(tags[1].attributes.instance_count, 3);
    }

    #[tokio::test]
    async fn test_get_org_tags_empty() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/tags"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [],
                "meta": {
                    "pagination": {
                        "current-page": 1,
                        "total-pages": 1,
                        "total-count": 0
                    }
                }
            })))
            .mount(&mock_server)
            .await;

        let result = client.get_org_tags("my-org", None).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_get_org_tags_with_search() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/tags"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(org_tags_response(vec![("env", 5)])),
            )
            .mount(&mock_server)
            .await;

        let result = client.get_org_tags("my-org", Some("env")).await;
        assert!(result.is_ok());
        let tags = result.unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].attributes.name, "env");
    }

    // ===== Workspace flat string tags tests =====

    #[tokio::test]
    async fn test_get_workspace_tags_success() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-abc123/relationships/tags"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {
                        "id": "tag-1",
                        "type": "tags",
                        "attributes": {
                            "name": "model__env",
                            "instance-count": 4,
                            "created-at": "2026-01-23T14:55:13.733Z"
                        }
                    },
                    {
                        "id": "tag-2",
                        "type": "tags",
                        "attributes": {
                            "name": "team-infra",
                            "instance-count": 2,
                            "created-at": "2026-01-20T10:00:00Z"
                        }
                    }
                ]
            })))
            .mount(&mock_server)
            .await;

        let result = client.get_workspace_tags("ws-abc123").await;
        assert!(result.is_ok());
        let tags = result.unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].attributes.name, "model__env");
        assert_eq!(tags[0].attributes.instance_count, 4);
        assert_eq!(tags[1].attributes.name, "team-infra");
    }

    #[tokio::test]
    async fn test_get_workspace_tags_empty() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-abc123/relationships/tags"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": []
            })))
            .mount(&mock_server)
            .await;

        let result = client.get_workspace_tags("ws-abc123").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_get_workspace_tags_not_found() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-notfound/relationships/tags"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let result = client.get_workspace_tags("ws-notfound").await;
        assert!(result.is_err());
        match result.unwrap_err() {
            TfeError::Api { status, message } => {
                assert_eq!(status, 404);
                assert!(message.contains("ws-notfound"));
            }
            _ => panic!("Expected TfeError::Api"),
        }
    }

    // ===== Add workspace flat string tags tests =====

    #[tokio::test]
    async fn test_add_workspace_tags_success() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        let expected_body = serde_json::json!({
            "data": [
                {
                    "type": "tags",
                    "attributes": { "name": "env" }
                },
                {
                    "type": "tags",
                    "attributes": { "name": "team" }
                }
            ]
        });

        Mock::given(method("POST"))
            .and(path("/workspaces/ws-abc123/relationships/tags"))
            .and(body_json(&expected_body))
            .respond_with(ResponseTemplate::new(204))
            .mount(&mock_server)
            .await;

        let tag_names = vec!["env".to_string(), "team".to_string()];
        let result = client.add_workspace_tags("ws-abc123", &tag_names).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_add_workspace_tags_single() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("POST"))
            .and(path("/workspaces/ws-abc123/relationships/tags"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&mock_server)
            .await;

        let tag_names = vec!["DUPA".to_string()];
        let result = client.add_workspace_tags("ws-abc123", &tag_names).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_add_workspace_tags_not_found() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("POST"))
            .and(path("/workspaces/ws-notfound/relationships/tags"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let tag_names = vec!["env".to_string()];
        let result = client.add_workspace_tags("ws-notfound", &tag_names).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            TfeError::Api { status, message } => {
                assert_eq!(status, 404);
                assert!(message.contains("ws-notfound"));
            }
            _ => panic!("Expected TfeError::Api"),
        }
    }

    #[tokio::test]
    async fn test_add_workspace_tags_server_error() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("POST"))
            .and(path("/workspaces/ws-abc123/relationships/tags"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock_server)
            .await;

        let tag_names = vec!["env".to_string()];
        let result = client.add_workspace_tags("ws-abc123", &tag_names).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            TfeError::Api { status, message } => {
                assert_eq!(status, 500);
                assert!(message.contains("ws-abc123"));
            }
            _ => panic!("Expected TfeError::Api"),
        }
    }

    // ===== Remove workspace flat string tags tests =====

    #[tokio::test]
    async fn test_remove_workspace_tags_success() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        let expected_body = serde_json::json!({
            "data": [
                {
                    "type": "tags",
                    "attributes": { "name": "env" }
                },
                {
                    "type": "tags",
                    "attributes": { "name": "team" }
                }
            ]
        });

        Mock::given(method("DELETE"))
            .and(path("/workspaces/ws-abc123/relationships/tags"))
            .and(body_json(&expected_body))
            .respond_with(ResponseTemplate::new(204))
            .mount(&mock_server)
            .await;

        let tag_names = vec!["env".to_string(), "team".to_string()];
        let result = client.remove_workspace_tags("ws-abc123", &tag_names).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_remove_workspace_tags_single() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("DELETE"))
            .and(path("/workspaces/ws-abc123/relationships/tags"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&mock_server)
            .await;

        let tag_names = vec!["DUPA".to_string()];
        let result = client.remove_workspace_tags("ws-abc123", &tag_names).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_remove_workspace_tags_not_found() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("DELETE"))
            .and(path("/workspaces/ws-notfound/relationships/tags"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let tag_names = vec!["env".to_string()];
        let result = client
            .remove_workspace_tags("ws-notfound", &tag_names)
            .await;
        assert!(result.is_err());
        match result.unwrap_err() {
            TfeError::Api { status, message } => {
                assert_eq!(status, 404);
                assert!(message.contains("ws-notfound"));
            }
            _ => panic!("Expected TfeError::Api"),
        }
    }

    #[tokio::test]
    async fn test_remove_workspace_tags_server_error() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("DELETE"))
            .and(path("/workspaces/ws-abc123/relationships/tags"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock_server)
            .await;

        let tag_names = vec!["env".to_string()];
        let result = client.remove_workspace_tags("ws-abc123", &tag_names).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            TfeError::Api { status, message } => {
                assert_eq!(status, 500);
                assert!(message.contains("ws-abc123"));
            }
            _ => panic!("Expected TfeError::Api"),
        }
    }
}
