//! TFE API data models

use serde::Deserialize;

/// Response wrapper for organizations list
#[derive(Deserialize, Debug)]
pub struct OrganizationsResponse {
    pub data: Vec<Organization>,
}

/// Organization data from TFE API
#[derive(Deserialize, Debug, Clone)]
pub struct Organization {
    pub id: String,
    #[serde(rename = "type")]
    pub org_type: Option<String>,
}

impl Organization {
    /// Get the organization name (same as id in TFE)
    pub fn name(&self) -> &str {
        &self.id
    }
}

/// Response wrapper for workspaces list
#[derive(Deserialize, Debug)]
pub struct WorkspacesResponse {
    pub data: Vec<Workspace>,
    #[serde(default)]
    pub meta: Option<PaginationMeta>,
}

/// Pagination metadata from TFE API
#[derive(Deserialize, Debug, Default)]
pub struct PaginationMeta {
    pub pagination: Option<Pagination>,
}

/// Pagination details
#[derive(Deserialize, Debug)]
pub struct Pagination {
    #[serde(rename = "current-page")]
    pub current_page: u32,
    #[serde(rename = "total-pages")]
    pub total_pages: u32,
    #[serde(rename = "total-count")]
    pub total_count: u32,
}

/// Workspace data from TFE API
#[derive(Deserialize, Debug, Clone)]
pub struct Workspace {
    pub id: String,
    pub attributes: WorkspaceAttributes,
}

impl Workspace {
    /// Get the workspace name
    pub fn name(&self) -> &str {
        &self.attributes.name
    }

    /// Check if workspace name contains the given filter
    pub fn matches_filter(&self, filter: &str) -> bool {
        self.attributes.name.contains(filter)
    }

    /// Get resource count, defaulting to 0 if not available
    pub fn resource_count(&self) -> u32 {
        self.attributes.resource_count.unwrap_or(0)
    }

    /// Get execution mode, defaulting to "unknown" if not available
    pub fn execution_mode(&self) -> &str {
        self.attributes
            .execution_mode
            .as_deref()
            .unwrap_or("unknown")
    }

    /// Check if workspace is locked
    pub fn is_locked(&self) -> bool {
        self.attributes.locked.unwrap_or(false)
    }

    /// Get terraform version, defaulting to "unknown" if not available
    pub fn terraform_version(&self) -> &str {
        self.attributes
            .terraform_version
            .as_deref()
            .unwrap_or("unknown")
    }

    /// Get updated_at timestamp, defaulting to empty string if not available
    pub fn updated_at(&self) -> &str {
        self.attributes.updated_at.as_deref().unwrap_or("")
    }
}

/// Workspace attributes from TFE API
#[derive(Deserialize, Debug, Clone)]
pub struct WorkspaceAttributes {
    pub name: String,

    #[serde(rename = "execution-mode")]
    pub execution_mode: Option<String>,

    #[serde(rename = "resource-count")]
    pub resource_count: Option<u32>,

    pub locked: Option<bool>,

    #[serde(rename = "terraform-version")]
    pub terraform_version: Option<String>,

    #[serde(rename = "updated-at")]
    pub updated_at: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_workspace(name: &str, locked: bool) -> Workspace {
        Workspace {
            id: format!("ws-{}", name),
            attributes: WorkspaceAttributes {
                name: name.to_string(),
                execution_mode: Some("remote".to_string()),
                resource_count: Some(42),
                locked: Some(locked),
                terraform_version: Some("1.5.0".to_string()),
                updated_at: None,
            },
        }
    }

    #[test]
    fn test_workspace_name() {
        let ws = create_test_workspace("my-workspace", false);
        assert_eq!(ws.name(), "my-workspace");
    }

    #[test]
    fn test_workspace_matches_filter() {
        let ws = create_test_workspace("gcp-dev-app-1234", false);
        assert!(ws.matches_filter("dev"));
        assert!(ws.matches_filter("gcp"));
        assert!(!ws.matches_filter("prod"));
    }

    #[test]
    fn test_workspace_resource_count_default() {
        let ws = Workspace {
            id: "ws-123".to_string(),
            attributes: WorkspaceAttributes {
                name: "test".to_string(),
                execution_mode: None,
                resource_count: None,
                locked: None,
                terraform_version: None,
                updated_at: None,
            },
        };
        assert_eq!(ws.resource_count(), 0);
    }

    #[test]
    fn test_workspace_is_locked() {
        let locked_ws = create_test_workspace("locked", true);
        let unlocked_ws = create_test_workspace("unlocked", false);

        assert!(locked_ws.is_locked());
        assert!(!unlocked_ws.is_locked());
    }

    #[test]
    fn test_workspace_deserialization() {
        let json = r#"{
            "id": "ws-abc123",
            "attributes": {
                "name": "my-workspace",
                "execution-mode": "remote",
                "resource-count": 50,
                "locked": true,
                "terraform-version": "1.6.0"
            }
        }"#;

        let ws: Workspace = serde_json::from_str(json).unwrap();
        assert_eq!(ws.id, "ws-abc123");
        assert_eq!(ws.name(), "my-workspace");
        assert_eq!(ws.resource_count(), 50);
        assert!(ws.is_locked());
    }
}
