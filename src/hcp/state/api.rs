//! State API operations

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use log::debug;
use md5::{Digest, Md5};

use crate::config::api;
use crate::error::{Result, TfeError};
use crate::hcp::TfeClient;

use super::models::{
    CurrentStateVersionResponse, EmptyTerraformState, StateVersionRequest, TerraformState,
};

impl TfeClient {
    /// Get current state version download URL for a workspace
    pub async fn get_current_state_version(
        &self,
        workspace_id: &str,
    ) -> Result<CurrentStateVersionResponse> {
        let url = format!(
            "{}/{}/{}/current-state-version",
            self.base_url(),
            api::WORKSPACES,
            workspace_id
        );

        debug!("Fetching current state version for: {}", workspace_id);

        let response = self.get(&url).send().await?;

        match response.status().as_u16() {
            200 => {
                let data: CurrentStateVersionResponse = response.json().await?;
                Ok(data)
            }
            404 => Err(TfeError::Api {
                status: 404,
                message: format!(
                    "No state version found for workspace '{}'. The workspace may be empty.",
                    workspace_id
                ),
            }),
            status => {
                let body = response.text().await.unwrap_or_default();
                Err(TfeError::Api {
                    status,
                    message: format!(
                        "Failed to get state version for '{}': {}",
                        workspace_id, body
                    ),
                })
            }
        }
    }

    /// Download state file from URL
    pub async fn download_state(&self, download_url: &str) -> Result<TerraformState> {
        debug!("Downloading state from: {}", download_url);

        let response = self.get(download_url).send().await?;

        match response.status().as_u16() {
            200 => {
                let state: TerraformState = response.json().await.map_err(|e| TfeError::Api {
                    status: 200,
                    message: format!("Failed to parse state file: {}", e),
                })?;
                Ok(state)
            }
            status => {
                let body = response.text().await.unwrap_or_default();
                Err(TfeError::Api {
                    status,
                    message: format!("Failed to download state: {}", body),
                })
            }
        }
    }

    /// Upload a new state version to a workspace
    pub async fn upload_state_version(
        &self,
        workspace_id: &str,
        empty_state: &EmptyTerraformState,
    ) -> Result<()> {
        let url = format!(
            "{}/{}/{}/state-versions",
            self.base_url(),
            api::WORKSPACES,
            workspace_id
        );

        debug!(
            "Uploading empty state version (serial: {}) for: {}",
            empty_state.serial, workspace_id
        );

        // Serialize state to JSON
        let state_json =
            serde_json::to_string(empty_state).map_err(|e| TfeError::Json(e.to_string()))?;

        // Calculate MD5 hash
        let mut hasher = Md5::new();
        hasher.update(state_json.as_bytes());
        let md5_hash = format!("{:x}", hasher.finalize());

        // Base64 encode the state
        let state_base64 = BASE64.encode(state_json.as_bytes());

        // Build request
        let request = StateVersionRequest::new(
            empty_state.serial,
            &md5_hash,
            &empty_state.lineage,
            &state_base64,
        );

        let response = self.post(&url).json(&request).send().await?;

        match response.status().as_u16() {
            200 | 201 => {
                debug!("Successfully uploaded empty state version");
                Ok(())
            }
            404 => Err(TfeError::Api {
                status: 404,
                message: format!("Workspace '{}' not found", workspace_id),
            }),
            409 => Err(TfeError::Api {
                status: 409,
                message: format!(
                    "State version conflict for '{}'. Another state may have been uploaded.",
                    workspace_id
                ),
            }),
            422 => {
                let error_body: serde_json::Value =
                    response.json().await.unwrap_or(serde_json::json!({}));
                let error_msg = error_body["errors"][0]["detail"]
                    .as_str()
                    .unwrap_or("Validation error");
                Err(TfeError::Api {
                    status: 422,
                    message: format!("Invalid state version: {}", error_msg),
                })
            }
            status => {
                let body = response.text().await.unwrap_or_default();
                Err(TfeError::Api {
                    status,
                    message: format!("Failed to upload state version: {}", body),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn create_test_client(base_url: &str) -> TfeClient {
        TfeClient::with_base_url(
            "test-token".to_string(),
            "mock.terraform.io".to_string(),
            base_url.to_string(),
        )
    }

    #[tokio::test]
    async fn test_get_current_state_version_success() {
        let mock_server = MockServer::start().await;

        let response = serde_json::json!({
            "data": {
                "id": "sv-456",
                "attributes": {
                    "serial": 10,
                    "terraform-version": "1.5.0",
                    "hosted-state-download-url": "https://example.com/state",
                    "lineage": "abc-123"
                }
            }
        });

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-123/current-state-version"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server.uri());
        let result = client.get_current_state_version("ws-123").await;

        assert!(result.is_ok());
        let sv = result.unwrap();
        assert_eq!(sv.data.id, "sv-456");
        assert_eq!(sv.data.attributes.serial, 10);
    }

    #[tokio::test]
    async fn test_get_current_state_version_empty_workspace() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-empty/current-state-version"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server.uri());
        let result = client.get_current_state_version("ws-empty").await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("No state version found"));
    }

    #[tokio::test]
    async fn test_download_state_success() {
        let mock_server = MockServer::start().await;

        let state = serde_json::json!({
            "version": 4,
            "terraform_version": "1.5.0",
            "serial": 10,
            "lineage": "abc-123",
            "outputs": {},
            "resources": [
                {"type": "aws_instance", "name": "test"}
            ]
        });

        Mock::given(method("GET"))
            .and(path("/state-download"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&state))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server.uri());
        let download_url = format!("{}/state-download", mock_server.uri());
        let result = client.download_state(&download_url).await;

        assert!(result.is_ok());
        let downloaded = result.unwrap();
        assert_eq!(downloaded.serial, 10);
        assert_eq!(downloaded.lineage, "abc-123");
        assert_eq!(downloaded.resources.len(), 1);
    }

    #[tokio::test]
    async fn test_upload_state_version_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/workspaces/ws-123/state-versions"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "data": {
                    "id": "sv-new",
                    "attributes": {
                        "serial": 11
                    }
                }
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server.uri());

        let empty_state = EmptyTerraformState {
            version: 4,
            terraform_version: "1.5.0".to_string(),
            serial: 11,
            lineage: "abc-123".to_string(),
            outputs: serde_json::json!({}),
            resources: vec![],
        };

        let result = client.upload_state_version("ws-123", &empty_state).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_upload_state_version_conflict() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/workspaces/ws-123/state-versions"))
            .respond_with(ResponseTemplate::new(409))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server.uri());

        let empty_state = EmptyTerraformState {
            version: 4,
            terraform_version: "1.5.0".to_string(),
            serial: 11,
            lineage: "abc-123".to_string(),
            outputs: serde_json::json!({}),
            resources: vec![],
        };

        let result = client.upload_state_version("ws-123", &empty_state).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("conflict"));
    }
}
