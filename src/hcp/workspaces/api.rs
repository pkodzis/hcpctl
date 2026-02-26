//! Workspace API operations

use futures::stream::{self, StreamExt};
use log::debug;

use crate::config::api;
use crate::error::{Result, TfeError};
use crate::hcp::{PaginationInfo, TfeClient};

use super::models::{Workspace, WorkspaceQuery};
use crate::hcp::traits::ApiListResponse;

/// Build the API path for workspaces with optional query params
fn build_workspaces_path(org: &str, query: &WorkspaceQuery<'_>) -> String {
    let mut path = format!("/{}/{}/{}", api::ORGANIZATIONS, org, api::WORKSPACES);

    let mut query_parts = Vec::new();
    if let Some(s) = query.search {
        query_parts.push(format!("search[name]={}", urlencoding::encode(s)));
    }
    if let Some(prj) = query.project_id {
        query_parts.push(format!("filter[project][id]={}", urlencoding::encode(prj)));
    }
    if let Some(tags) = query.search_tags {
        query_parts.push(format!("search[tags]={}", urlencoding::encode(tags)));
    }

    if !query_parts.is_empty() {
        path.push('?');
        path.push_str(&query_parts.join("&"));
    }

    path
}

impl TfeClient {
    /// Get workspaces for an organization with optional filters
    ///
    /// Uses API query parameters for efficient server-side filtering:
    /// - `search[name]` for fuzzy name search
    /// - `filter[project][id]` for project filtering
    pub async fn get_workspaces(
        &self,
        org: &str,
        query: WorkspaceQuery<'_>,
    ) -> Result<Vec<Workspace>> {
        let path = build_workspaces_path(org, &query);

        let error_context = format!(
            "workspaces for organization '{}' (search: {:?}, project: {:?})",
            org, query.search, query.project_id
        );

        self.fetch_all_pages::<Workspace, ApiListResponse<Workspace>>(&path, &error_context)
            .await
    }

    /// Prefetch pagination info for workspaces without fetching all data
    ///
    /// Use this to check the scale of an operation before committing to full fetch.
    pub async fn prefetch_workspaces_pagination_info(
        &self,
        org: &str,
        query: WorkspaceQuery<'_>,
    ) -> Result<Option<PaginationInfo>> {
        let path = build_workspaces_path(org, &query);
        let error_context = format!("workspaces pagination info for organization '{}'", org);

        self.prefetch_pagination_info::<Workspace, ApiListResponse<Workspace>>(
            &path,
            &error_context,
        )
        .await
    }

    /// Get a single workspace by ID (direct API call, no org needed)
    /// Returns both the typed model and raw JSON for flexible output
    pub async fn get_workspace_by_id(
        &self,
        workspace_id: &str,
    ) -> Result<Option<(Workspace, serde_json::Value)>> {
        let path = format!("/{}/{}", api::WORKSPACES, workspace_id);
        self.fetch_resource_by_path::<Workspace>(&path, &format!("workspace '{}'", workspace_id))
            .await
    }

    /// Get a single workspace by name (requires org)
    /// Returns both the typed model and raw JSON for flexible output
    pub async fn get_workspace_by_name(
        &self,
        org: &str,
        name: &str,
    ) -> Result<Option<(Workspace, serde_json::Value)>> {
        let path = format!(
            "/{}/{}/{}/{}",
            api::ORGANIZATIONS,
            org,
            api::WORKSPACES,
            name
        );
        self.fetch_resource_by_path::<Workspace>(&path, &format!("workspace '{}'", name))
            .await
    }

    /// Fetch a subresource by its API URL
    /// Used to fetch related resources like current-run, current-state-version, etc.
    pub async fn get_subresource(&self, url: &str) -> Result<serde_json::Value> {
        let full_url = format!("https://{}{}", self.host(), url);
        debug!("Fetching subresource: {}", full_url);

        let response = self.get(&full_url).send().await?;

        match response.status().as_u16() {
            200 => {
                let raw: serde_json::Value = response.json().await?;
                Ok(raw)
            }
            404 => Err(TfeError::Api {
                status: 404,
                message: format!("Subresource not found at '{}'", url),
            }),
            status => Err(TfeError::Api {
                status,
                message: format!("Failed to fetch subresource from '{}'", url),
            }),
        }
    }

    /// Fetch multiple workspaces by their IDs concurrently
    ///
    /// Uses `buffer_unordered` with `MAX_CONCURRENT_PAGE_REQUESTS` for concurrency.
    /// Skips 404s and errors (deleted workspaces) with debug warnings.
    /// Returns (Workspace, org_name) tuples.
    pub async fn fetch_workspaces_by_ids(
        &self,
        workspace_ids: &[String],
    ) -> Vec<(Workspace, String)> {
        if workspace_ids.is_empty() {
            return Vec::new();
        }

        let results: Vec<Option<(Workspace, String)>> = stream::iter(workspace_ids)
            .map(|ws_id| async move {
                match self.get_workspace_by_id(ws_id).await {
                    Ok(Some((ws, _raw))) => {
                        let org_name = ws.organization_name().unwrap_or("unknown").to_string();
                        Some((ws, org_name))
                    }
                    Ok(None) => {
                        debug!("Workspace '{}' not found (404), skipping", ws_id);
                        None
                    }
                    Err(e) => {
                        debug!("Error fetching workspace '{}': {}, skipping", ws_id, e);
                        None
                    }
                }
            })
            .buffer_unordered(api::MAX_CONCURRENT_PAGE_REQUESTS)
            .collect()
            .await;

        results.into_iter().flatten().collect()
    }

    /// Lock a workspace to prevent concurrent modifications
    pub async fn lock_workspace(&self, workspace_id: &str) -> Result<()> {
        let url = format!(
            "{}/{}/{}/actions/lock",
            self.base_url(),
            api::WORKSPACES,
            workspace_id
        );

        debug!("Locking workspace: {}", workspace_id);

        let response = self.post(&url).send().await?;

        match response.status().as_u16() {
            200 => Ok(()),
            404 => Err(TfeError::Api {
                status: 404,
                message: format!("Workspace '{}' not found", workspace_id),
            }),
            409 => Err(TfeError::Api {
                status: 409,
                message: format!(
                    "Workspace '{}' is already locked or has an active run",
                    workspace_id
                ),
            }),
            status => {
                let body = response.text().await.unwrap_or_default();
                Err(TfeError::Api {
                    status,
                    message: format!("Failed to lock workspace '{}': {}", workspace_id, body),
                })
            }
        }
    }

    /// Unlock a workspace
    pub async fn unlock_workspace(&self, workspace_id: &str) -> Result<()> {
        let url = format!(
            "{}/{}/{}/actions/unlock",
            self.base_url(),
            api::WORKSPACES,
            workspace_id
        );

        debug!("Unlocking workspace: {}", workspace_id);

        let response = self.post(&url).send().await?;

        match response.status().as_u16() {
            200 => Ok(()),
            404 => Err(TfeError::Api {
                status: 404,
                message: format!("Workspace '{}' not found", workspace_id),
            }),
            409 => Err(TfeError::Api {
                status: 409,
                message: format!(
                    "Workspace '{}' is not locked or locked by another user/run",
                    workspace_id
                ),
            }),
            status => {
                let body = response.text().await.unwrap_or_default();
                Err(TfeError::Api {
                    status,
                    message: format!("Failed to unlock workspace '{}': {}", workspace_id, body),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hcp::traits::TfeResource;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn workspace_json(id: &str, name: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "attributes": {
                "name": name,
                "execution-mode": "remote",
                "resource-count": 10,
                "locked": false,
                "terraform-version": "1.5.0"
            }
        })
    }

    #[tokio::test]
    async fn test_get_workspaces_success() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        let response_body = serde_json::json!({
            "data": [
                workspace_json("ws-1", "workspace-1"),
                workspace_json("ws-2", "workspace-2")
            ]
        });

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/workspaces"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&mock_server)
            .await;

        let result = client
            .get_workspaces("my-org", WorkspaceQuery::default())
            .await;

        assert!(result.is_ok());
        let workspaces = result.unwrap();
        assert_eq!(workspaces.len(), 2);
        assert_eq!(workspaces[0].name(), "workspace-1");
        assert_eq!(workspaces[1].name(), "workspace-2");
    }

    #[tokio::test]
    async fn test_get_workspaces_with_search() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        let response_body = serde_json::json!({
            "data": [workspace_json("ws-prod", "production")]
        });

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/workspaces"))
            .and(query_param("search[name]", "prod"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&mock_server)
            .await;

        let query = WorkspaceQuery {
            search: Some("prod"),
            ..Default::default()
        };
        let result = client.get_workspaces("my-org", query).await;

        assert!(result.is_ok());
        let workspaces = result.unwrap();
        assert_eq!(workspaces.len(), 1);
        assert_eq!(workspaces[0].name(), "production");
    }

    #[tokio::test]
    async fn test_get_workspaces_with_project_filter() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        let response_body = serde_json::json!({
            "data": [workspace_json("ws-prj", "project-ws")]
        });

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/workspaces"))
            .and(query_param("filter[project][id]", "prj-123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&mock_server)
            .await;

        let query = WorkspaceQuery {
            project_id: Some("prj-123"),
            ..Default::default()
        };
        let result = client.get_workspaces("my-org", query).await;

        assert!(result.is_ok());
        let workspaces = result.unwrap();
        assert_eq!(workspaces.len(), 1);
    }

    #[tokio::test]
    async fn test_get_workspaces_with_tag_filter() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        let response_body = serde_json::json!({
            "data": [
                workspace_json("ws-tagged1", "tagged-ws-1"),
                workspace_json("ws-tagged2", "tagged-ws-2"),
            ]
        });

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/workspaces"))
            .and(query_param("search[tags]", "env"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&mock_server)
            .await;

        let query = WorkspaceQuery {
            search_tags: Some("env"),
            ..Default::default()
        };
        let result = client.get_workspaces("my-org", query).await;

        assert!(result.is_ok());
        let workspaces = result.unwrap();
        assert_eq!(workspaces.len(), 2);
        assert_eq!(workspaces[0].name(), "tagged-ws-1");
        assert_eq!(workspaces[1].name(), "tagged-ws-2");
    }

    #[tokio::test]
    async fn test_get_workspaces_api_error() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/workspaces"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&mock_server)
            .await;

        let result = client
            .get_workspaces("my-org", WorkspaceQuery::default())
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            TfeError::Api { status, message } => {
                assert_eq!(status, 403);
                assert!(message.contains("my-org"));
            }
            _ => panic!("Expected TfeError::Api"),
        }
    }

    #[tokio::test]
    async fn test_get_workspaces_empty() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        let response_body = serde_json::json!({
            "data": []
        });

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/workspaces"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&mock_server)
            .await;

        let result = client
            .get_workspaces("my-org", WorkspaceQuery::default())
            .await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_get_workspace_by_id_success() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        let response_body = serde_json::json!({
            "data": workspace_json("ws-abc123", "my-workspace")
        });

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-abc123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&mock_server)
            .await;

        let result = client.get_workspace_by_id("ws-abc123").await;

        assert!(result.is_ok());
        let (workspace, _raw) = result.unwrap().unwrap();
        assert_eq!(workspace.id, "ws-abc123");
        assert_eq!(workspace.name(), "my-workspace");
    }

    #[tokio::test]
    async fn test_get_workspace_by_id_not_found() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-nonexistent"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let result = client.get_workspace_by_id("ws-nonexistent").await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_get_workspace_by_name_success() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        let response_body = serde_json::json!({
            "data": workspace_json("ws-xyz", "production")
        });

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/workspaces/production"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&mock_server)
            .await;

        let result = client.get_workspace_by_name("my-org", "production").await;

        assert!(result.is_ok());
        let (workspace, _raw) = result.unwrap().unwrap();
        assert_eq!(workspace.name(), "production");
    }

    #[tokio::test]
    async fn test_get_workspace_by_name_not_found() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/workspaces/nonexistent"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let result = client.get_workspace_by_name("my-org", "nonexistent").await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_lock_workspace_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/workspaces/ws-123/actions/lock"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let client = TfeClient::test_client(&mock_server.uri());
        let result = client.lock_workspace("ws-123").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_lock_workspace_already_locked() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/workspaces/ws-123/actions/lock"))
            .respond_with(ResponseTemplate::new(409))
            .mount(&mock_server)
            .await;

        let client = TfeClient::test_client(&mock_server.uri());
        let result = client.lock_workspace("ws-123").await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("already locked"));
    }

    #[tokio::test]
    async fn test_lock_workspace_not_found() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/workspaces/ws-notfound/actions/lock"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let client = TfeClient::test_client(&mock_server.uri());
        let result = client.lock_workspace("ws-notfound").await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_unlock_workspace_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/workspaces/ws-123/actions/unlock"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let client = TfeClient::test_client(&mock_server.uri());
        let result = client.unlock_workspace("ws-123").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_unlock_workspace_not_locked() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/workspaces/ws-123/actions/unlock"))
            .respond_with(ResponseTemplate::new(409))
            .mount(&mock_server)
            .await;

        let client = TfeClient::test_client(&mock_server.uri());
        let result = client.unlock_workspace("ws-123").await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("not locked"));
    }

    fn workspace_json_with_org(id: &str, name: &str, org: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "attributes": {
                "name": name,
                "execution-mode": "remote",
                "resource-count": 10,
                "locked": false,
                "terraform-version": "1.5.0"
            },
            "relationships": {
                "organization": {
                    "data": {
                        "id": org,
                        "type": "organizations"
                    }
                }
            }
        })
    }

    #[tokio::test]
    async fn test_fetch_workspaces_by_ids_success() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-aaa"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": workspace_json_with_org("ws-aaa", "workspace-a", "org-alpha")
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-bbb"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": workspace_json_with_org("ws-bbb", "workspace-b", "org-beta")
            })))
            .mount(&mock_server)
            .await;

        let ids = vec!["ws-aaa".to_string(), "ws-bbb".to_string()];
        let results = client.fetch_workspaces_by_ids(&ids).await;

        assert_eq!(results.len(), 2);

        let names: Vec<&str> = results.iter().map(|(ws, _)| ws.name()).collect();
        assert!(names.contains(&"workspace-a"));
        assert!(names.contains(&"workspace-b"));

        let orgs: Vec<&str> = results.iter().map(|(_, org)| org.as_str()).collect();
        assert!(orgs.contains(&"org-alpha"));
        assert!(orgs.contains(&"org-beta"));
    }

    #[tokio::test]
    async fn test_fetch_workspaces_by_ids_partial_not_found() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-exists"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": workspace_json_with_org("ws-exists", "good-ws", "my-org")
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-gone"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let ids = vec!["ws-exists".to_string(), "ws-gone".to_string()];
        let results = client.fetch_workspaces_by_ids(&ids).await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.name(), "good-ws");
        assert_eq!(results[0].1, "my-org");
    }

    #[tokio::test]
    async fn test_fetch_workspaces_by_ids_empty_input() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        let ids: Vec<String> = vec![];
        let results = client.fetch_workspaces_by_ids(&ids).await;

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_fetch_workspaces_by_ids_all_not_found() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-gone1"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-gone2"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let ids = vec!["ws-gone1".to_string(), "ws-gone2".to_string()];
        let results = client.fetch_workspaces_by_ids(&ids).await;

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_fetch_workspaces_by_ids_server_error_skipped() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-ok"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": workspace_json_with_org("ws-ok", "healthy-ws", "my-org")
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-err"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let ids = vec!["ws-ok".to_string(), "ws-err".to_string()];
        let results = client.fetch_workspaces_by_ids(&ids).await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.name(), "healthy-ws");
        assert_eq!(results[0].1, "my-org");
    }

    #[tokio::test]
    async fn test_fetch_workspaces_by_ids_single_id() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-solo"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": workspace_json_with_org("ws-solo", "solo-ws", "org-one")
            })))
            .mount(&mock_server)
            .await;

        let ids = vec!["ws-solo".to_string()];
        let results = client.fetch_workspaces_by_ids(&ids).await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.id, "ws-solo");
        assert_eq!(results[0].0.name(), "solo-ws");
        assert_eq!(results[0].1, "org-one");
    }

    #[tokio::test]
    async fn test_fetch_workspaces_by_ids_missing_org_relationship() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        // Use workspace_json which has NO organization relationship
        Mock::given(method("GET"))
            .and(path("/workspaces/ws-noorg"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": workspace_json("ws-noorg", "orphan-ws")
            })))
            .mount(&mock_server)
            .await;

        let ids = vec!["ws-noorg".to_string()];
        let results = client.fetch_workspaces_by_ids(&ids).await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.name(), "orphan-ws");
        assert_eq!(results[0].1, "unknown");
    }

    #[tokio::test]
    async fn test_fetch_workspaces_by_ids_mixed_success_404_500() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-good"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": workspace_json_with_org("ws-good", "good-ws", "org-a")
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-deleted"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-broken"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-also-good"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": workspace_json_with_org("ws-also-good", "another-ws", "org-b")
            })))
            .mount(&mock_server)
            .await;

        let ids = vec![
            "ws-good".to_string(),
            "ws-deleted".to_string(),
            "ws-broken".to_string(),
            "ws-also-good".to_string(),
        ];
        let results = client.fetch_workspaces_by_ids(&ids).await;

        assert_eq!(results.len(), 2);
        let names: Vec<&str> = results.iter().map(|(ws, _)| ws.name()).collect();
        assert!(names.contains(&"good-ws"));
        assert!(names.contains(&"another-ws"));
    }
}
