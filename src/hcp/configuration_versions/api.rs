//! Configuration versions API operations

use log::debug;
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use crate::config::api;
use crate::error::{Result, TfeError};
use crate::hcp::TfeClient;

use super::models::{
    ConfigurationVersion, ConfigurationVersionResponse, ConfigurationVersionsResponse,
};

impl TfeClient {
    /// Get configuration versions for a workspace
    ///
    /// # Arguments
    /// * `workspace_id` - The workspace ID (e.g., "ws-abc123")
    pub async fn get_configuration_versions(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<ConfigurationVersion>> {
        let path = format!(
            "/{}/{}/configuration-versions",
            api::WORKSPACES,
            workspace_id
        );
        let error_context = format!("configuration versions for workspace '{}'", workspace_id);

        self.fetch_all_pages::<ConfigurationVersion, ConfigurationVersionsResponse>(
            &path,
            &error_context,
        )
        .await
    }

    /// Get a single configuration version by ID
    ///
    /// # Arguments
    /// * `cv_id` - The configuration version ID (e.g., "cv-abc123")
    pub async fn get_configuration_version(&self, cv_id: &str) -> Result<ConfigurationVersion> {
        let url = format!("{}/configuration-versions/{}", self.base_url(), cv_id);
        debug!("Fetching configuration version: {}", url);

        let response = self.get(&url).send().await?;

        match response.status().as_u16() {
            200 => {
                let data: ConfigurationVersionResponse = response.json().await?;
                Ok(data.data)
            }
            404 => Err(TfeError::Api {
                status: 404,
                message: format!("Configuration version '{}' not found", cv_id),
            }),
            status => {
                let body = response.text().await.unwrap_or_default();
                Err(TfeError::Api {
                    status,
                    message: format!(
                        "Failed to fetch configuration version '{}': {}",
                        cv_id, body
                    ),
                })
            }
        }
    }

    /// Download configuration files as tar.gz
    ///
    /// # Arguments
    /// * `cv_id` - The configuration version ID
    /// * `output_path` - Path where to save the tar.gz file
    ///
    /// # Returns
    /// Number of bytes downloaded
    pub async fn download_configuration(&self, cv_id: &str, output_path: &Path) -> Result<u64> {
        let url = format!(
            "{}/configuration-versions/{}/download",
            self.base_url(),
            cv_id
        );
        debug!("Downloading configuration from: {}", url);

        // The /download endpoint returns 302 redirect to actual file
        // reqwest follows redirects by default
        let response = self.get(&url).send().await?;

        match response.status().as_u16() {
            200 => {
                let bytes = response.bytes().await?;
                let size = bytes.len() as u64;

                // Write to file
                let mut file = File::create(output_path).await.map_err(|e| TfeError::Io {
                    message: format!("Failed to create file '{}': {}", output_path.display(), e),
                })?;

                file.write_all(&bytes).await.map_err(|e| TfeError::Io {
                    message: format!("Failed to write to '{}': {}", output_path.display(), e),
                })?;

                // Ensure all data is flushed to disk
                file.flush().await.map_err(|e| TfeError::Io {
                    message: format!("Failed to flush file '{}': {}", output_path.display(), e),
                })?;

                debug!("Downloaded {} bytes to {}", size, output_path.display());
                Ok(size)
            }
            204 => Err(TfeError::Api {
                status: 204,
                message: format!(
                    "Configuration version '{}' has no downloadable content (empty or not uploaded)",
                    cv_id
                ),
            }),
            404 => Err(TfeError::Api {
                status: 404,
                message: format!("Configuration version '{}' not found", cv_id),
            }),
            status => {
                let body = response.text().await.unwrap_or_default();
                Err(TfeError::Api {
                    status,
                    message: format!("Failed to download configuration '{}': {}", cv_id, body),
                })
            }
        }
    }

    /// Get the latest downloadable configuration version for a workspace
    ///
    /// Returns the most recent configuration version with status "uploaded"
    pub async fn get_latest_configuration_version(
        &self,
        workspace_id: &str,
    ) -> Result<Option<ConfigurationVersion>> {
        let cvs = self.get_configuration_versions(workspace_id).await?;

        // CVs are returned in reverse chronological order (newest first)
        // Find the first one that is downloadable
        Ok(cvs.into_iter().find(|cv| cv.is_downloadable()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn create_test_client(base_url: &str) -> TfeClient {
        TfeClient::with_base_url(
            "test-token".to_string(),
            "mock.terraform.io".to_string(),
            base_url.to_string(),
        )
    }

    #[tokio::test]
    async fn test_get_configuration_versions() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-abc123/configuration-versions"))
            .and(query_param("page[number]", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {
                        "id": "cv-123",
                        "type": "configuration-versions",
                        "attributes": {
                            "source": "tfe-api",
                            "status": "uploaded",
                            "speculative": false,
                            "provisional": false
                        },
                        "links": {
                            "self": "/api/v2/configuration-versions/cv-123",
                            "download": "/api/v2/configuration-versions/cv-123/download"
                        }
                    },
                    {
                        "id": "cv-456",
                        "type": "configuration-versions",
                        "attributes": {
                            "source": "gitlab",
                            "status": "pending",
                            "speculative": false,
                            "provisional": false
                        },
                        "links": {
                            "self": "/api/v2/configuration-versions/cv-456"
                        }
                    }
                ],
                "meta": {
                    "pagination": {
                        "current-page": 1,
                        "total-pages": 1,
                        "total-count": 2
                    }
                }
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server.uri());
        let result = client
            .get_configuration_versions("ws-abc123")
            .await
            .unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, "cv-123");
        assert_eq!(result[0].attributes.status, "uploaded");
        assert!(result[0].is_downloadable());
        assert_eq!(result[1].id, "cv-456");
        assert!(!result[1].is_downloadable());
    }

    #[tokio::test]
    async fn test_get_configuration_versions_paginated() {
        let mock_server = MockServer::start().await;

        // Page 1
        Mock::given(method("GET"))
            .and(path("/workspaces/ws-abc123/configuration-versions"))
            .and(query_param("page[number]", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [{"id": "cv-1", "type": "configuration-versions", "attributes": {"status": "uploaded", "speculative": false, "provisional": false}, "links": {}}],
                "meta": {"pagination": {"current-page": 1, "total-pages": 2, "total-count": 2}}
            })))
            .mount(&mock_server)
            .await;

        // Page 2
        Mock::given(method("GET"))
            .and(path("/workspaces/ws-abc123/configuration-versions"))
            .and(query_param("page[number]", "2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [{"id": "cv-2", "type": "configuration-versions", "attributes": {"status": "uploaded", "speculative": false, "provisional": false}, "links": {}}],
                "meta": {"pagination": {"current-page": 2, "total-pages": 2, "total-count": 2}}
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server.uri());
        let result = client
            .get_configuration_versions("ws-abc123")
            .await
            .unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, "cv-1");
        assert_eq!(result[1].id, "cv-2");
    }

    #[tokio::test]
    async fn test_get_configuration_version() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/configuration-versions/cv-abc123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "id": "cv-abc123",
                    "type": "configuration-versions",
                    "attributes": {
                        "source": "gitlab",
                        "status": "uploaded",
                        "speculative": false,
                        "provisional": false
                    },
                    "links": {
                        "self": "/api/v2/configuration-versions/cv-abc123",
                        "download": "/api/v2/configuration-versions/cv-abc123/download"
                    }
                }
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server.uri());
        let result = client.get_configuration_version("cv-abc123").await.unwrap();

        assert_eq!(result.id, "cv-abc123");
        assert_eq!(result.attributes.status, "uploaded");
        assert_eq!(result.source(), "gitlab");
    }

    #[tokio::test]
    async fn test_get_configuration_version_not_found() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/configuration-versions/cv-nonexistent"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server.uri());
        let result = client.get_configuration_version("cv-nonexistent").await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, TfeError::Api { status: 404, .. }));
    }

    #[tokio::test]
    async fn test_download_configuration() {
        let mock_server = MockServer::start().await;

        let tar_content = b"fake tar.gz content";

        Mock::given(method("GET"))
            .and(path("/configuration-versions/cv-abc123/download"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(tar_content.to_vec()))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server.uri());
        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = temp_dir.path().join("config.tar.gz");

        let size = client
            .download_configuration("cv-abc123", &output_path)
            .await
            .unwrap();

        assert_eq!(size, tar_content.len() as u64);
        assert!(output_path.exists());

        let downloaded = std::fs::read(&output_path).unwrap();
        assert_eq!(downloaded, tar_content);
    }

    #[tokio::test]
    async fn test_download_configuration_no_content() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/configuration-versions/cv-empty/download"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server.uri());
        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = temp_dir.path().join("config.tar.gz");

        let result = client
            .download_configuration("cv-empty", &output_path)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, TfeError::Api { status: 204, .. }));
    }

    #[tokio::test]
    async fn test_download_configuration_not_found() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/configuration-versions/cv-gone/download"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server.uri());
        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = temp_dir.path().join("config.tar.gz");

        let result = client.download_configuration("cv-gone", &output_path).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, TfeError::Api { status: 404, .. }));
    }

    #[tokio::test]
    async fn test_get_latest_configuration_version() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-abc123/configuration-versions"))
            .and(query_param("page[number]", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {
                        "id": "cv-newest-pending",
                        "type": "configuration-versions",
                        "attributes": {"status": "pending", "speculative": false, "provisional": false},
                        "links": {}
                    },
                    {
                        "id": "cv-older-uploaded",
                        "type": "configuration-versions",
                        "attributes": {"status": "uploaded", "speculative": false, "provisional": false},
                        "links": {"download": "/api/v2/configuration-versions/cv-older-uploaded/download"}
                    },
                    {
                        "id": "cv-oldest-uploaded",
                        "type": "configuration-versions",
                        "attributes": {"status": "uploaded", "speculative": false, "provisional": false},
                        "links": {"download": "/api/v2/configuration-versions/cv-oldest-uploaded/download"}
                    }
                ],
                "meta": {"pagination": {"current-page": 1, "total-pages": 1, "total-count": 3}}
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server.uri());
        let result = client
            .get_latest_configuration_version("ws-abc123")
            .await
            .unwrap();

        // Should skip cv-newest-pending and return cv-older-uploaded
        assert!(result.is_some());
        assert_eq!(result.unwrap().id, "cv-older-uploaded");
    }

    #[tokio::test]
    async fn test_get_latest_configuration_version_none_downloadable() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-abc123/configuration-versions"))
            .and(query_param("page[number]", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {
                        "id": "cv-pending",
                        "type": "configuration-versions",
                        "attributes": {"status": "pending", "speculative": false, "provisional": false},
                        "links": {}
                    }
                ],
                "meta": {"pagination": {"current-page": 1, "total-pages": 1, "total-count": 1}}
            })))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server.uri());
        let result = client
            .get_latest_configuration_version("ws-abc123")
            .await
            .unwrap();

        assert!(result.is_none());
    }
}
