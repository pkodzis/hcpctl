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

#[cfg(test)]
mod tests {
    use super::*;

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
