//! Configuration version data models

use serde::Deserialize;

use crate::hcp::traits::{PaginatedResponse, TfeResource};
use crate::hcp::PaginationMeta;

/// Response wrapper for configuration versions list
#[derive(Deserialize, Debug)]
pub struct ConfigurationVersionsResponse {
    pub data: Vec<ConfigurationVersion>,
    #[serde(default)]
    pub meta: Option<PaginationMeta>,
}

impl PaginatedResponse<ConfigurationVersion> for ConfigurationVersionsResponse {
    fn into_data(self) -> Vec<ConfigurationVersion> {
        self.data
    }

    fn meta(&self) -> Option<&PaginationMeta> {
        self.meta.as_ref()
    }
}

/// Single configuration version response
#[derive(Deserialize, Debug)]
pub struct ConfigurationVersionResponse {
    pub data: ConfigurationVersion,
}

/// Configuration version data from TFE API
#[derive(Deserialize, Debug, Clone)]
pub struct ConfigurationVersion {
    pub id: String,
    pub attributes: ConfigurationVersionAttributes,
    pub links: Option<ConfigurationVersionLinks>,
}

/// Configuration version attributes
#[derive(Deserialize, Debug, Clone)]
pub struct ConfigurationVersionAttributes {
    /// Source of the configuration (e.g., "tfe-api", "gitlab", "github")
    pub source: Option<String>,
    /// Status: pending, fetching, uploaded, archived, errored
    pub status: String,
    /// Whether this is a speculative configuration
    #[serde(default)]
    pub speculative: bool,
    /// Whether this is a provisional configuration
    #[serde(default)]
    pub provisional: bool,
    /// Error message if status is "errored"
    #[serde(rename = "error-message")]
    pub error_message: Option<String>,
}

/// Configuration version links
#[derive(Deserialize, Debug, Clone)]
pub struct ConfigurationVersionLinks {
    /// Self link
    #[serde(rename = "self")]
    pub self_link: Option<String>,
    /// Download link for configuration files
    pub download: Option<String>,
}

impl TfeResource for ConfigurationVersion {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        // Configuration versions don't have names, use ID
        &self.id
    }
}

impl ConfigurationVersion {
    /// Check if configuration version is downloadable
    pub fn is_downloadable(&self) -> bool {
        self.attributes.status == "uploaded"
    }

    /// Get download path if available
    pub fn download_path(&self) -> Option<&str> {
        self.links.as_ref().and_then(|l| l.download.as_deref())
    }

    /// Get source description
    pub fn source(&self) -> &str {
        self.attributes.source.as_deref().unwrap_or("unknown")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_cv(status: &str, download: Option<&str>) -> ConfigurationVersion {
        ConfigurationVersion {
            id: "cv-abc123".to_string(),
            attributes: ConfigurationVersionAttributes {
                source: Some("tfe-api".to_string()),
                status: status.to_string(),
                speculative: false,
                provisional: false,
                error_message: None,
            },
            links: Some(ConfigurationVersionLinks {
                self_link: Some("/api/v2/configuration-versions/cv-abc123".to_string()),
                download: download.map(|s| s.to_string()),
            }),
        }
    }

    #[test]
    fn test_is_downloadable_uploaded() {
        let cv = create_test_cv(
            "uploaded",
            Some("/api/v2/configuration-versions/cv-abc123/download"),
        );
        assert!(cv.is_downloadable());
    }

    #[test]
    fn test_is_downloadable_pending() {
        let cv = create_test_cv("pending", None);
        assert!(!cv.is_downloadable());
    }

    #[test]
    fn test_is_downloadable_archived() {
        let cv = create_test_cv("archived", None);
        assert!(!cv.is_downloadable());
    }

    #[test]
    fn test_download_path() {
        let cv = create_test_cv(
            "uploaded",
            Some("/api/v2/configuration-versions/cv-abc123/download"),
        );
        assert_eq!(
            cv.download_path(),
            Some("/api/v2/configuration-versions/cv-abc123/download")
        );
    }

    #[test]
    fn test_download_path_none() {
        let cv = create_test_cv("pending", None);
        assert_eq!(cv.download_path(), None);
    }

    #[test]
    fn test_source() {
        let cv = create_test_cv("uploaded", None);
        assert_eq!(cv.source(), "tfe-api");
    }

    #[test]
    fn test_tfe_resource_trait() {
        let cv = create_test_cv("uploaded", None);
        assert_eq!(cv.id(), "cv-abc123");
        assert_eq!(cv.name(), "cv-abc123");
    }

    #[test]
    fn test_deserialize_configuration_version() {
        let json = r#"{
            "id": "cv-ntv3HbhJqvFzamy7",
            "type": "configuration-versions",
            "attributes": {
                "source": "gitlab",
                "speculative": false,
                "status": "uploaded",
                "provisional": false
            },
            "links": {
                "self": "/api/v2/configuration-versions/cv-ntv3HbhJqvFzamy7",
                "download": "/api/v2/configuration-versions/cv-ntv3HbhJqvFzamy7/download"
            }
        }"#;

        let cv: ConfigurationVersion = serde_json::from_str(json).unwrap();
        assert_eq!(cv.id, "cv-ntv3HbhJqvFzamy7");
        assert_eq!(cv.attributes.status, "uploaded");
        assert_eq!(cv.source(), "gitlab");
        assert!(cv.is_downloadable());
        assert_eq!(
            cv.download_path(),
            Some("/api/v2/configuration-versions/cv-ntv3HbhJqvFzamy7/download")
        );
    }

    #[test]
    fn test_deserialize_configuration_versions_response() {
        let json = r#"{
            "data": [
                {
                    "id": "cv-abc123",
                    "type": "configuration-versions",
                    "attributes": {
                        "source": "tfe-api",
                        "speculative": false,
                        "status": "uploaded",
                        "provisional": false
                    },
                    "links": {
                        "self": "/api/v2/configuration-versions/cv-abc123",
                        "download": "/api/v2/configuration-versions/cv-abc123/download"
                    }
                }
            ],
            "meta": {
                "pagination": {
                    "current-page": 1,
                    "total-pages": 1,
                    "total-count": 1
                }
            }
        }"#;

        let resp: ConfigurationVersionsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.len(), 1);
        assert_eq!(resp.data[0].id, "cv-abc123");
    }
}
