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

    /// Update workspace settings (terraform version, project assignment, etc.)
    ///
    /// Uses PATCH /workspaces/:workspace_id with JSON:API body
    /// Only includes fields that are Some — callers pass None for unchanged settings
    pub async fn update_workspace(
        &self,
        workspace_id: &str,
        terraform_version: Option<&str>,
        project_id: Option<&str>,
    ) -> Result<Workspace> {
        let url = format!("{}/{}/{}", self.base_url(), api::WORKSPACES, workspace_id);

        debug!(
            "Updating workspace {} (terraform_version={:?}, project_id={:?})",
            workspace_id, terraform_version, project_id
        );

        let mut data = serde_json::json!({
            "type": "workspaces"
        });

        if let Some(tf_version) = terraform_version {
            data["attributes"] = serde_json::json!({
                "terraform-version": tf_version
            });
        }

        if let Some(prj_id) = project_id {
            data["relationships"] = serde_json::json!({
                "project": {
                    "data": {
                        "type": "projects",
                        "id": prj_id
                    }
                }
            });
        }

        let body = serde_json::json!({ "data": data });

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
                message: format!("Workspace '{}' not found", workspace_id),
            }),
            403 => {
                let body = response.text().await.unwrap_or_default();
                Err(TfeError::Api {
                    status: 403,
                    message: format!(
                        "Forbidden: cannot update workspace '{}': {}",
                        workspace_id, body
                    ),
                })
            }
            422 => {
                let body = response.text().await.unwrap_or_default();
                Err(TfeError::Api {
                    status: 422,
                    message: format!(
                        "Invalid update for workspace '{}': {}. \
                         Hint: check that the terraform version and project ID are valid",
                        workspace_id, body
                    ),
                })
            }
            status => {
                let body = response.text().await.unwrap_or_default();
                Err(TfeError::Api {
                    status,
                    message: format!("Failed to update workspace '{}': {}", workspace_id, body),
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

    // === update_workspace tests ===

    fn update_workspace_response(
        ws_id: &str,
        ws_name: &str,
        tf_version: &str,
        prj_id: &str,
    ) -> serde_json::Value {
        serde_json::json!({
            "data": {
                "id": ws_id,
                "type": "workspaces",
                "attributes": {
                    "name": ws_name,
                    "execution-mode": "remote",
                    "resource-count": 10,
                    "locked": false,
                    "terraform-version": tf_version
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

    #[tokio::test]
    async fn test_update_workspace_terraform_version_only_success() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        let expected_body = serde_json::json!({
            "data": {
                "type": "workspaces",
                "attributes": {
                    "terraform-version": "1.7.0"
                }
            }
        });

        Mock::given(method("PATCH"))
            .and(path("/workspaces/ws-abc123"))
            .and(body_json(expected_body))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(update_workspace_response(
                    "ws-abc123",
                    "my-workspace",
                    "1.7.0",
                    "prj-xyz789",
                )),
            )
            .mount(&mock_server)
            .await;

        let result = client
            .update_workspace("ws-abc123", Some("1.7.0"), None)
            .await;

        assert!(result.is_ok());
        let workspace = result.unwrap();
        assert_eq!(workspace.id, "ws-abc123");
        assert_eq!(workspace.name(), "my-workspace");
        assert_eq!(workspace.terraform_version(), "1.7.0");
    }

    #[tokio::test]
    async fn test_update_workspace_project_only_success() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        let expected_body = serde_json::json!({
            "data": {
                "type": "workspaces",
                "relationships": {
                    "project": {
                        "data": {
                            "type": "projects",
                            "id": "prj-new789"
                        }
                    }
                }
            }
        });

        Mock::given(method("PATCH"))
            .and(path("/workspaces/ws-abc123"))
            .and(body_json(expected_body))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(update_workspace_response(
                    "ws-abc123",
                    "my-workspace",
                    "1.5.0",
                    "prj-new789",
                )),
            )
            .mount(&mock_server)
            .await;

        let result = client
            .update_workspace("ws-abc123", None, Some("prj-new789"))
            .await;

        assert!(result.is_ok());
        let workspace = result.unwrap();
        assert_eq!(workspace.id, "ws-abc123");
        assert_eq!(workspace.project_id(), Some("prj-new789"));
    }

    #[tokio::test]
    async fn test_update_workspace_combined_success() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        let expected_body = serde_json::json!({
            "data": {
                "type": "workspaces",
                "attributes": {
                    "terraform-version": "1.8.0"
                },
                "relationships": {
                    "project": {
                        "data": {
                            "type": "projects",
                            "id": "prj-new789"
                        }
                    }
                }
            }
        });

        Mock::given(method("PATCH"))
            .and(path("/workspaces/ws-abc123"))
            .and(body_json(expected_body))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(update_workspace_response(
                    "ws-abc123",
                    "my-workspace",
                    "1.8.0",
                    "prj-new789",
                )),
            )
            .mount(&mock_server)
            .await;

        let result = client
            .update_workspace("ws-abc123", Some("1.8.0"), Some("prj-new789"))
            .await;

        assert!(result.is_ok());
        let workspace = result.unwrap();
        assert_eq!(workspace.id, "ws-abc123");
        assert_eq!(workspace.terraform_version(), "1.8.0");
        assert_eq!(workspace.project_id(), Some("prj-new789"));
    }

    #[tokio::test]
    async fn test_update_workspace_no_changes() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        let expected_body = serde_json::json!({
            "data": {
                "type": "workspaces"
            }
        });

        Mock::given(method("PATCH"))
            .and(path("/workspaces/ws-abc123"))
            .and(body_json(expected_body))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(update_workspace_response(
                    "ws-abc123",
                    "my-workspace",
                    "1.5.0",
                    "prj-xyz789",
                )),
            )
            .mount(&mock_server)
            .await;

        let result = client.update_workspace("ws-abc123", None, None).await;

        assert!(result.is_ok());
        let workspace = result.unwrap();
        assert_eq!(workspace.id, "ws-abc123");
        assert_eq!(workspace.name(), "my-workspace");
    }

    #[tokio::test]
    async fn test_update_workspace_not_found() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("PATCH"))
            .and(path("/workspaces/ws-notfound"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let result = client
            .update_workspace("ws-notfound", Some("1.7.0"), None)
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
    async fn test_update_workspace_forbidden() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("PATCH"))
            .and(path("/workspaces/ws-abc123"))
            .respond_with(ResponseTemplate::new(403).set_body_string("Forbidden"))
            .mount(&mock_server)
            .await;

        let result = client
            .update_workspace("ws-abc123", Some("1.7.0"), None)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            TfeError::Api { status, message } => {
                assert_eq!(status, 403);
                assert!(message.contains("ws-abc123"));
            }
            _ => panic!("Expected TfeError::Api"),
        }
    }

    #[tokio::test]
    async fn test_update_workspace_invalid() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("PATCH"))
            .and(path("/workspaces/ws-abc123"))
            .respond_with(ResponseTemplate::new(422).set_body_string("Invalid terraform version"))
            .mount(&mock_server)
            .await;

        let result = client
            .update_workspace("ws-abc123", Some("invalid"), None)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            TfeError::Api { status, message } => {
                assert_eq!(status, 422);
                assert!(message.contains("ws-abc123"));
                assert!(message.contains("Hint"));
            }
            _ => panic!("Expected TfeError::Api"),
        }
    }

    #[tokio::test]
    async fn test_update_workspace_server_error() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("PATCH"))
            .and(path("/workspaces/ws-abc123"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock_server)
            .await;

        let result = client
            .update_workspace("ws-abc123", Some("1.7.0"), None)
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
}
