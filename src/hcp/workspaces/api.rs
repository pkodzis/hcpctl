//! Workspace API operations

use log::debug;

use crate::config::api;
use crate::error::{Result, TfeError};
use crate::hcp::TfeClient;

use super::models::{Workspace, WorkspaceQuery, WorkspacesResponse};

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
        // Build path with optional query params
        let mut path = format!("/{}/{}/{}", api::ORGANIZATIONS, org, api::WORKSPACES);

        let mut query_parts = Vec::new();
        if let Some(s) = query.search {
            query_parts.push(format!("search[name]={}", urlencoding::encode(s)));
        }
        if let Some(prj) = query.project_id {
            query_parts.push(format!("filter[project][id]={}", urlencoding::encode(prj)));
        }

        if !query_parts.is_empty() {
            path.push('?');
            path.push_str(&query_parts.join("&"));
        }

        let error_context = format!(
            "workspaces for organization '{}' (search: {:?}, project: {:?})",
            org, query.search, query.project_id
        );

        self.fetch_all_pages::<Workspace, WorkspacesResponse>(&path, &error_context)
            .await
    }

    /// Get a single workspace by ID (direct API call, no org needed)
    /// Returns both the typed model and raw JSON for flexible output
    pub async fn get_workspace_by_id(
        &self,
        workspace_id: &str,
    ) -> Result<Option<(Workspace, serde_json::Value)>> {
        let url = format!("{}/{}/{}", self.base_url(), api::WORKSPACES, workspace_id);
        debug!("Fetching workspace directly by ID: {}", url);

        let response = self.get(&url).send().await?;

        match response.status().as_u16() {
            200 => {
                // First get raw JSON
                let raw: serde_json::Value = response.json().await?;
                // Then deserialize model from the same data
                let workspace: Workspace =
                    serde_json::from_value(raw["data"].clone()).map_err(|e| TfeError::Api {
                        status: 200,
                        message: format!("Failed to parse workspace: {}", e),
                    })?;
                Ok(Some((workspace, raw)))
            }
            404 => Ok(None),
            status => Err(TfeError::Api {
                status,
                message: format!("Failed to fetch workspace '{}'", workspace_id),
            }),
        }
    }

    /// Get a single workspace by name (requires org)
    /// Returns both the typed model and raw JSON for flexible output
    pub async fn get_workspace_by_name(
        &self,
        org: &str,
        name: &str,
    ) -> Result<Option<(Workspace, serde_json::Value)>> {
        let url = format!(
            "{}/{}/{}/{}/{}",
            self.base_url(),
            api::ORGANIZATIONS,
            org,
            api::WORKSPACES,
            name
        );

        debug!("Fetching workspace by name: {}", url);

        let response = self.get(&url).send().await?;

        match response.status().as_u16() {
            200 => {
                // First get raw JSON
                let raw: serde_json::Value = response.json().await?;
                // Then deserialize model from the same data
                let workspace: Workspace =
                    serde_json::from_value(raw["data"].clone()).map_err(|e| TfeError::Api {
                        status: 200,
                        message: format!("Failed to parse workspace: {}", e),
                    })?;
                Ok(Some((workspace, raw)))
            }
            404 => Ok(None),
            status => Err(TfeError::Api {
                status,
                message: format!("Failed to fetch workspace '{}'", name),
            }),
        }
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

    fn create_test_client(base_url: &str) -> TfeClient {
        TfeClient::with_base_url(
            "test-token".to_string(),
            "mock.terraform.io".to_string(),
            base_url.to_string(),
        )
    }

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
        let client = create_test_client(&mock_server.uri());

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
        let client = create_test_client(&mock_server.uri());

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
            project_id: None,
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
        let client = create_test_client(&mock_server.uri());

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
            search: None,
            project_id: Some("prj-123"),
        };
        let result = client.get_workspaces("my-org", query).await;

        assert!(result.is_ok());
        let workspaces = result.unwrap();
        assert_eq!(workspaces.len(), 1);
    }

    #[tokio::test]
    async fn test_get_workspaces_api_error() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(&mock_server.uri());

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
        let client = create_test_client(&mock_server.uri());

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
        let client = create_test_client(&mock_server.uri());

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
        let client = create_test_client(&mock_server.uri());

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
        let client = create_test_client(&mock_server.uri());

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
        let client = create_test_client(&mock_server.uri());

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

        let client = create_test_client(&mock_server.uri());
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

        let client = create_test_client(&mock_server.uri());
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

        let client = create_test_client(&mock_server.uri());
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

        let client = create_test_client(&mock_server.uri());
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

        let client = create_test_client(&mock_server.uri());
        let result = client.unlock_workspace("ws-123").await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("not locked"));
    }
}
