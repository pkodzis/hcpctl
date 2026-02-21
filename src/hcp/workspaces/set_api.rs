//! Workspace set (assign to project) API operations

use log::debug;

use crate::config::api;
use crate::error::{Result, TfeError};
use crate::hcp::TfeClient;

use super::models::Workspace;

impl TfeClient {
    /// Assign a workspace to a project
    ///
    /// Uses PATCH /workspaces/:workspace_id with JSON:API relationship body
    pub async fn assign_workspace_to_project(
        &self,
        workspace_id: &str,
        project_id: &str,
    ) -> Result<Workspace> {
        let url = format!("{}/{}/{}", self.base_url(), api::WORKSPACES, workspace_id);

        debug!(
            "Assigning workspace {} to project {}",
            workspace_id, project_id
        );

        let body = serde_json::json!({
            "data": {
                "type": "workspaces",
                "relationships": {
                    "project": {
                        "data": {
                            "type": "projects",
                            "id": project_id
                        }
                    }
                }
            }
        });

        let response = self.patch(&url).json(&body).send().await?;

        match response.status().as_u16() {
            200 => {
                let raw: serde_json::Value = response.json().await?;
                let workspace: Workspace =
                    serde_json::from_value(raw["data"].clone()).map_err(|e| TfeError::Api {
                        status: 200,
                        message: format!("Failed to parse workspace response: {}", e),
                    })?;
                Ok(workspace)
            }
            404 => Err(TfeError::Api {
                status: 404,
                message: format!(
                    "Workspace '{}' or project '{}' not found",
                    workspace_id, project_id
                ),
            }),
            422 => {
                let body = response.text().await.unwrap_or_default();
                Err(TfeError::Api {
                    status: 422,
                    message: format!(
                        "Cannot assign workspace '{}' to project '{}': {}. \
                         Hint: you need admin permissions on both the source and destination project",
                        workspace_id, project_id, body
                    ),
                })
            }
            status => {
                let body = response.text().await.unwrap_or_default();
                Err(TfeError::Api {
                    status,
                    message: format!(
                        "Failed to assign workspace '{}' to project '{}': {}",
                        workspace_id, project_id, body
                    ),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hcp::traits::TfeResource;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn workspace_response(ws_id: &str, ws_name: &str, prj_id: &str) -> serde_json::Value {
        serde_json::json!({
            "data": {
                "id": ws_id,
                "type": "workspaces",
                "attributes": {
                    "name": ws_name,
                    "execution-mode": "remote",
                    "resource-count": 10,
                    "locked": false,
                    "terraform-version": "1.5.0"
                },
                "relationships": {
                    "project": {
                        "data": {
                            "id": prj_id,
                            "type": "projects"
                        }
                    }
                }
            }
        })
    }

    fn expected_request_body(prj_id: &str) -> serde_json::Value {
        serde_json::json!({
            "data": {
                "type": "workspaces",
                "relationships": {
                    "project": {
                        "data": {
                            "type": "projects",
                            "id": prj_id
                        }
                    }
                }
            }
        })
    }

    #[tokio::test]
    async fn test_assign_workspace_to_project_success() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("PATCH"))
            .and(path("/workspaces/ws-abc123"))
            .and(body_json(expected_request_body("prj-xyz789")))
            .respond_with(ResponseTemplate::new(200).set_body_json(workspace_response(
                "ws-abc123",
                "my-workspace",
                "prj-xyz789",
            )))
            .mount(&mock_server)
            .await;

        let result = client
            .assign_workspace_to_project("ws-abc123", "prj-xyz789")
            .await;

        assert!(result.is_ok());
        let workspace = result.unwrap();
        assert_eq!(workspace.id, "ws-abc123");
        assert_eq!(workspace.name(), "my-workspace");
        assert_eq!(workspace.project_id(), Some("prj-xyz789"));
    }

    #[tokio::test]
    async fn test_assign_workspace_to_project_not_found() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("PATCH"))
            .and(path("/workspaces/ws-notfound"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let result = client
            .assign_workspace_to_project("ws-notfound", "prj-xyz789")
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            TfeError::Api { status, message } => {
                assert_eq!(status, 404);
                assert!(message.contains("ws-notfound"));
            }
            _ => panic!("Expected TfeError::Api"),
        }
    }

    #[tokio::test]
    async fn test_assign_workspace_to_project_forbidden() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("PATCH"))
            .and(path("/workspaces/ws-abc123"))
            .respond_with(ResponseTemplate::new(403).set_body_string("Forbidden"))
            .mount(&mock_server)
            .await;

        let result = client
            .assign_workspace_to_project("ws-abc123", "prj-xyz789")
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            TfeError::Api { status, message } => {
                assert_eq!(status, 403);
                assert!(message.contains("ws-abc123"));
                assert!(message.contains("prj-xyz789"));
            }
            _ => panic!("Expected TfeError::Api"),
        }
    }

    #[tokio::test]
    async fn test_assign_workspace_to_project_server_error() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("PATCH"))
            .and(path("/workspaces/ws-abc123"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock_server)
            .await;

        let result = client
            .assign_workspace_to_project("ws-abc123", "prj-xyz789")
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            TfeError::Api { status, .. } => {
                assert_eq!(status, 500);
            }
            _ => panic!("Expected TfeError::Api"),
        }
    }

    #[tokio::test]
    async fn test_assign_workspace_to_project_unprocessable() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("PATCH"))
            .and(path("/workspaces/ws-abc123"))
            .respond_with(
                ResponseTemplate::new(422)
                    .set_body_string("Project not found, or you are not authorized to use it."),
            )
            .mount(&mock_server)
            .await;

        let result = client
            .assign_workspace_to_project("ws-abc123", "prj-xyz789")
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            TfeError::Api { status, message } => {
                assert_eq!(status, 422);
                assert!(message.contains("ws-abc123"));
                assert!(message.contains("prj-xyz789"));
                assert!(message.contains("admin permissions"));
            }
            _ => panic!("Expected TfeError::Api"),
        }
    }
}
