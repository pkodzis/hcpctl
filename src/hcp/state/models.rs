//! State version data models

use serde::{Deserialize, Serialize};

/// Response wrapper for current state version
#[derive(Deserialize, Debug)]
pub struct CurrentStateVersionResponse {
    pub data: CurrentStateVersion,
}

/// State version data from TFE API
#[derive(Deserialize, Debug)]
pub struct CurrentStateVersion {
    pub id: String,
    pub attributes: StateVersionAttributes,
}

/// State version attributes from TFE API
#[derive(Deserialize, Debug)]
pub struct StateVersionAttributes {
    pub serial: u64,

    #[serde(rename = "terraform-version")]
    pub terraform_version: Option<String>,

    #[serde(rename = "hosted-state-download-url")]
    pub hosted_state_download_url: Option<String>,

    #[serde(rename = "resources-processed")]
    pub resources_processed: Option<bool>,

    pub lineage: Option<String>,
}

/// Downloaded Terraform state file structure
#[derive(Deserialize, Debug)]
pub struct TerraformState {
    pub version: u32,
    pub terraform_version: String,
    pub serial: u64,
    pub lineage: String,
    #[serde(default)]
    pub outputs: serde_json::Value,
    #[serde(default)]
    pub resources: Vec<serde_json::Value>,
}

/// Empty state to upload
#[derive(Serialize, Debug)]
pub struct EmptyTerraformState {
    pub version: u32,
    pub terraform_version: String,
    pub serial: u64,
    pub lineage: String,
    pub outputs: serde_json::Value,
    pub resources: Vec<serde_json::Value>,
}

impl EmptyTerraformState {
    /// Create a new empty state based on the current state
    pub fn from_current(state: &TerraformState) -> Self {
        Self {
            version: 4, // Terraform state version
            terraform_version: state.terraform_version.clone(),
            serial: state.serial + 1,
            lineage: state.lineage.clone(),
            outputs: serde_json::json!({}),
            resources: vec![],
        }
    }
}

/// Request payload for creating a new state version
#[derive(Serialize, Debug)]
pub struct StateVersionRequest {
    pub data: StateVersionData,
}

/// State version data for upload
#[derive(Serialize, Debug)]
pub struct StateVersionData {
    #[serde(rename = "type")]
    pub data_type: String,
    pub attributes: StateVersionUpload,
}

/// State version upload attributes
#[derive(Serialize, Debug)]
pub struct StateVersionUpload {
    pub serial: u64,
    pub md5: String,
    pub lineage: String,
    pub state: String, // base64 encoded state
}

impl StateVersionRequest {
    /// Create a new state version request for uploading empty state
    pub fn new(serial: u64, md5: &str, lineage: &str, state_base64: &str) -> Self {
        Self {
            data: StateVersionData {
                data_type: "state-versions".to_string(),
                attributes: StateVersionUpload {
                    serial,
                    md5: md5.to_string(),
                    lineage: lineage.to_string(),
                    state: state_base64.to_string(),
                },
            },
        }
    }
}

/// Response wrapper for state versions list
#[derive(Deserialize, Debug)]
pub struct StateVersionListResponse {
    pub data: Vec<StateVersionListItem>,
    #[serde(default)]
    pub meta: Option<crate::hcp::PaginationMeta>,
}

impl crate::hcp::PaginatedResponse<StateVersionListItem> for StateVersionListResponse {
    fn into_data(self) -> Vec<StateVersionListItem> {
        self.data
    }

    fn meta(&self) -> Option<&crate::hcp::PaginationMeta> {
        self.meta.as_ref()
    }
}

/// A single state version item from the list endpoint
#[derive(Deserialize, Debug)]
pub struct StateVersionListItem {
    pub id: String,
    pub attributes: StateVersionListAttributes,
    pub relationships: Option<StateVersionRelationships>,
}

/// Attributes for a state version list item
#[derive(Deserialize, Debug)]
pub struct StateVersionListAttributes {
    pub serial: Option<u64>,

    #[serde(rename = "created-at")]
    pub created_at: Option<String>,

    pub size: Option<u64>,

    pub status: Option<String>,

    #[serde(rename = "terraform-version")]
    pub terraform_version: Option<String>,

    #[serde(rename = "vcs-commit-sha")]
    pub vcs_commit_sha: Option<String>,

    #[serde(rename = "resources-processed")]
    pub resources_processed: Option<bool>,

    pub resources: Option<Vec<StateResource>>,
}

/// A resource entry in the state version
#[derive(Deserialize, Debug)]
pub struct StateResource {
    pub count: Option<u64>,
}

/// Relationships for a state version
#[derive(Deserialize, Debug)]
pub struct StateVersionRelationships {
    pub run: Option<RelationshipItem>,
}

/// A relationship item with data
#[derive(Deserialize, Debug)]
pub struct RelationshipItem {
    pub data: Option<RelationshipData>,
}

/// Relationship data containing an ID
#[derive(Deserialize, Debug)]
pub struct RelationshipData {
    pub id: String,
}

impl StateVersionListItem {
    /// Sum of resource counts, or None if not processed
    pub fn resource_count(&self) -> Option<u64> {
        if self.attributes.resources_processed != Some(true) {
            return None;
        }
        self.attributes
            .resources
            .as_ref()
            .map(|resources| resources.iter().filter_map(|r| r.count).sum())
    }

    /// Extract run ID from relationships, or "-"
    pub fn run_id(&self) -> &str {
        self.relationships
            .as_ref()
            .and_then(|r| r.run.as_ref())
            .and_then(|r| r.data.as_ref())
            .map(|d| d.id.as_str())
            .unwrap_or("-")
    }

    /// First 8 chars of VCS commit SHA, or "-"
    pub fn vcs_sha_short(&self) -> &str {
        self.attributes
            .vcs_commit_sha
            .as_deref()
            .map(|s| if s.len() > 8 { &s[..8] } else { s })
            .unwrap_or("-")
    }

    /// Format size in human-readable form
    pub fn size_human(&self) -> String {
        match self.attributes.size {
            Some(bytes) if bytes >= 1_048_576 => format!("{:.1} MB", bytes as f64 / 1_048_576.0),
            Some(bytes) if bytes >= 1024 => format!("{:.1} KB", bytes as f64 / 1024.0),
            Some(bytes) => format!("{} B", bytes),
            None => "-".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_version_list_item_resource_count() {
        let item = StateVersionListItem {
            id: "sv-1".to_string(),
            attributes: StateVersionListAttributes {
                serial: Some(1),
                created_at: None,
                size: None,
                status: None,
                terraform_version: None,
                vcs_commit_sha: None,
                resources_processed: Some(true),
                resources: Some(vec![
                    StateResource { count: Some(5) },
                    StateResource { count: Some(3) },
                ]),
            },
            relationships: None,
        };
        assert_eq!(item.resource_count(), Some(8));
    }

    #[test]
    fn test_state_version_list_item_resource_count_not_processed() {
        let item = StateVersionListItem {
            id: "sv-1".to_string(),
            attributes: StateVersionListAttributes {
                serial: Some(1),
                created_at: None,
                size: None,
                status: None,
                terraform_version: None,
                vcs_commit_sha: None,
                resources_processed: Some(false),
                resources: None,
            },
            relationships: None,
        };
        assert_eq!(item.resource_count(), None);
    }

    #[test]
    fn test_state_version_list_item_run_id() {
        let item = StateVersionListItem {
            id: "sv-1".to_string(),
            attributes: StateVersionListAttributes {
                serial: Some(1),
                created_at: None,
                size: None,
                status: None,
                terraform_version: None,
                vcs_commit_sha: None,
                resources_processed: None,
                resources: None,
            },
            relationships: Some(StateVersionRelationships {
                run: Some(RelationshipItem {
                    data: Some(RelationshipData {
                        id: "run-abc123".to_string(),
                    }),
                }),
            }),
        };
        assert_eq!(item.run_id(), "run-abc123");
    }

    #[test]
    fn test_state_version_list_item_run_id_missing() {
        let item = StateVersionListItem {
            id: "sv-1".to_string(),
            attributes: StateVersionListAttributes {
                serial: Some(1),
                created_at: None,
                size: None,
                status: None,
                terraform_version: None,
                vcs_commit_sha: None,
                resources_processed: None,
                resources: None,
            },
            relationships: None,
        };
        assert_eq!(item.run_id(), "-");
    }

    #[test]
    fn test_state_version_list_item_vcs_sha_short() {
        let item = StateVersionListItem {
            id: "sv-1".to_string(),
            attributes: StateVersionListAttributes {
                serial: Some(1),
                created_at: None,
                size: None,
                status: None,
                terraform_version: None,
                vcs_commit_sha: Some("abcdef1234567890".to_string()),
                resources_processed: None,
                resources: None,
            },
            relationships: None,
        };
        assert_eq!(item.vcs_sha_short(), "abcdef12");
    }

    #[test]
    fn test_state_version_list_item_size_human() {
        let make = |size: Option<u64>| StateVersionListItem {
            id: "sv-1".to_string(),
            attributes: StateVersionListAttributes {
                serial: Some(1),
                created_at: None,
                size,
                status: None,
                terraform_version: None,
                vcs_commit_sha: None,
                resources_processed: None,
                resources: None,
            },
            relationships: None,
        };
        assert_eq!(make(Some(500)).size_human(), "500 B");
        assert_eq!(make(Some(2048)).size_human(), "2.0 KB");
        assert_eq!(make(Some(2_097_152)).size_human(), "2.0 MB");
        assert_eq!(make(None).size_human(), "-");
    }

    #[test]
    fn test_state_version_list_response_deserialization() {
        let json = serde_json::json!({
            "data": [{
                "id": "sv-123",
                "attributes": {
                    "serial": 42,
                    "created-at": "2024-01-01T00:00:00Z",
                    "size": 1024,
                    "status": "finalized",
                    "terraform-version": "1.6.0",
                    "vcs-commit-sha": "abc123def456",
                    "resources-processed": true,
                    "resources": [{"count": 10}]
                },
                "relationships": {
                    "run": {
                        "data": {
                            "id": "run-abc",
                            "type": "runs"
                        }
                    }
                }
            }]
        });

        let response: StateVersionListResponse = serde_json::from_value(json).unwrap();
        assert_eq!(response.data.len(), 1);
        assert_eq!(response.data[0].id, "sv-123");
        assert_eq!(response.data[0].attributes.serial, Some(42));
        assert_eq!(response.data[0].resource_count(), Some(10));
        assert_eq!(response.data[0].run_id(), "run-abc");
    }

    #[test]
    fn test_empty_state_from_current() {
        let current = TerraformState {
            version: 4,
            terraform_version: "1.5.0".to_string(),
            serial: 10,
            lineage: "test-lineage-123".to_string(),
            outputs: serde_json::json!({"key": "value"}),
            resources: vec![serde_json::json!({"type": "aws_instance"})],
        };

        let empty = EmptyTerraformState::from_current(&current);

        assert_eq!(empty.version, 4);
        assert_eq!(empty.terraform_version, "1.5.0");
        assert_eq!(empty.serial, 11); // Incremented
        assert_eq!(empty.lineage, "test-lineage-123");
        assert_eq!(empty.outputs, serde_json::json!({}));
        assert!(empty.resources.is_empty());
    }

    #[test]
    fn test_state_version_request_serialization() {
        let request = StateVersionRequest::new(5, "abc123", "lineage-456", "base64data==");

        let json = serde_json::to_string(&request).unwrap();

        assert!(json.contains("\"serial\":5"));
        assert!(json.contains("\"md5\":\"abc123\""));
        assert!(json.contains("\"lineage\":\"lineage-456\""));
        assert!(json.contains("\"state\":\"base64data==\""));
        assert!(json.contains("\"type\":\"state-versions\""));
    }

    #[test]
    fn test_current_state_version_deserialization() {
        let json = serde_json::json!({
            "data": {
                "id": "sv-123",
                "attributes": {
                    "serial": 42,
                    "terraform-version": "1.6.0",
                    "hosted-state-download-url": "https://example.com/state",
                    "resources-processed": true,
                    "lineage": "abc-def-123"
                }
            }
        });

        let response: CurrentStateVersionResponse = serde_json::from_value(json).unwrap();

        assert_eq!(response.data.id, "sv-123");
        assert_eq!(response.data.attributes.serial, 42);
        assert_eq!(
            response.data.attributes.terraform_version,
            Some("1.6.0".to_string())
        );
        assert_eq!(
            response.data.attributes.hosted_state_download_url,
            Some("https://example.com/state".to_string())
        );
        assert_eq!(response.data.attributes.resources_processed, Some(true));
        assert_eq!(
            response.data.attributes.lineage,
            Some("abc-def-123".to_string())
        );
    }

    #[test]
    fn test_terraform_state_deserialization() {
        let json = serde_json::json!({
            "version": 4,
            "terraform_version": "1.5.0",
            "serial": 10,
            "lineage": "test-lineage",
            "outputs": {},
            "resources": [
                {"type": "aws_instance", "name": "test"}
            ]
        });

        let state: TerraformState = serde_json::from_value(json).unwrap();

        assert_eq!(state.version, 4);
        assert_eq!(state.terraform_version, "1.5.0");
        assert_eq!(state.serial, 10);
        assert_eq!(state.lineage, "test-lineage");
        assert_eq!(state.resources.len(), 1);
    }

    #[test]
    fn test_terraform_state_deserialization_minimal() {
        // State with missing optional fields
        let json = serde_json::json!({
            "version": 4,
            "terraform_version": "1.0.0",
            "serial": 1,
            "lineage": "minimal"
        });

        let state: TerraformState = serde_json::from_value(json).unwrap();

        assert_eq!(state.version, 4);
        assert_eq!(state.lineage, "minimal");
        assert!(state.resources.is_empty());
    }
}
