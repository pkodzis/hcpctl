//! Workspace data models

use serde::Deserialize;

use crate::hcp::traits::TfeResource;
use crate::hcp::PaginationMeta;

/// Query options for listing workspaces
#[derive(Default)]
pub struct WorkspaceQuery<'a> {
    /// Filter by workspace name (fuzzy server-side search)
    pub search: Option<&'a str>,
    /// Filter by project ID
    pub project_id: Option<&'a str>,
}

/// Response wrapper for workspaces list
#[derive(Deserialize, Debug)]
pub struct WorkspacesResponse {
    pub data: Vec<Workspace>,
    #[serde(default)]
    pub meta: Option<PaginationMeta>,
}

/// Workspace data from TFE API
#[derive(Deserialize, Debug, Clone)]
pub struct Workspace {
    pub id: String,
    pub attributes: WorkspaceAttributes,
    pub relationships: Option<WorkspaceRelationships>,
}

/// Workspace relationships from TFE API
#[derive(Deserialize, Debug, Clone)]
pub struct WorkspaceRelationships {
    pub project: Option<RelationshipData>,
    pub organization: Option<RelationshipData>,
}

/// Generic relationship data
#[derive(Deserialize, Debug, Clone)]
pub struct RelationshipData {
    pub data: Option<RelationshipId>,
}

/// Relationship ID reference
#[derive(Deserialize, Debug, Clone)]
pub struct RelationshipId {
    pub id: String,
    #[serde(rename = "type")]
    pub rel_type: Option<String>,
}

impl TfeResource for Workspace {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.attributes.name
    }
}

impl Workspace {
    /// Check if workspace name contains the given filter (substring match)
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

    /// Get project ID if available
    pub fn project_id(&self) -> Option<&str> {
        self.relationships
            .as_ref()
            .and_then(|r| r.project.as_ref())
            .and_then(|p| p.data.as_ref())
            .map(|d| d.id.as_str())
    }

    /// Get organization name if available (from relationships)
    pub fn organization_name(&self) -> Option<&str> {
        self.relationships
            .as_ref()
            .and_then(|r| r.organization.as_ref())
            .and_then(|o| o.data.as_ref())
            .map(|d| d.id.as_str())
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
            relationships: None,
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
            relationships: None,
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
    fn test_workspace_project_id() {
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
            relationships: Some(WorkspaceRelationships {
                project: Some(RelationshipData {
                    data: Some(RelationshipId {
                        id: "prj-456".to_string(),
                        rel_type: Some("projects".to_string()),
                    }),
                }),
                organization: None,
            }),
        };
        assert_eq!(ws.project_id(), Some("prj-456"));
    }

    #[test]
    fn test_workspace_project_id_none() {
        let ws = create_test_workspace("test", false);
        assert_eq!(ws.project_id(), None);
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

    #[test]
    fn test_workspace_deserialization_with_relationships() {
        let json = r#"{
            "id": "ws-abc123",
            "attributes": {
                "name": "my-workspace",
                "execution-mode": "remote",
                "resource-count": 50,
                "locked": true,
                "terraform-version": "1.6.0"
            },
            "relationships": {
                "project": {
                    "data": {
                        "id": "prj-xyz",
                        "type": "projects"
                    }
                }
            }
        }"#;

        let ws: Workspace = serde_json::from_str(json).unwrap();
        assert_eq!(ws.project_id(), Some("prj-xyz"));
    }

    // ===== WorkspaceQuery tests =====

    #[test]
    fn test_workspace_query_default() {
        let query = WorkspaceQuery::default();
        assert!(query.search.is_none());
        assert!(query.project_id.is_none());
    }

    #[test]
    fn test_workspace_query_with_search() {
        let query = WorkspaceQuery {
            search: Some("dev"),
            project_id: None,
        };
        assert_eq!(query.search, Some("dev"));
        assert!(query.project_id.is_none());
    }

    #[test]
    fn test_workspace_query_with_project_id() {
        let query = WorkspaceQuery {
            search: None,
            project_id: Some("prj-123"),
        };
        assert!(query.search.is_none());
        assert_eq!(query.project_id, Some("prj-123"));
    }

    #[test]
    fn test_workspace_query_with_both() {
        let query = WorkspaceQuery {
            search: Some("prod"),
            project_id: Some("prj-456"),
        };
        assert_eq!(query.search, Some("prod"));
        assert_eq!(query.project_id, Some("prj-456"));
    }

    // ===== Additional Workspace tests =====

    #[test]
    fn test_workspace_execution_mode() {
        let ws = create_test_workspace("test", false);
        assert_eq!(ws.execution_mode(), "remote");
    }

    #[test]
    fn test_workspace_execution_mode_default() {
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
            relationships: None,
        };
        assert_eq!(ws.execution_mode(), "unknown");
    }

    #[test]
    fn test_workspace_terraform_version() {
        let ws = create_test_workspace("test", false);
        assert_eq!(ws.terraform_version(), "1.5.0");
    }

    #[test]
    fn test_workspace_terraform_version_default() {
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
            relationships: None,
        };
        assert_eq!(ws.terraform_version(), "unknown");
    }

    #[test]
    fn test_workspace_updated_at() {
        let ws = Workspace {
            id: "ws-123".to_string(),
            attributes: WorkspaceAttributes {
                name: "test".to_string(),
                execution_mode: None,
                resource_count: None,
                locked: None,
                terraform_version: None,
                updated_at: Some("2025-01-01T00:00:00Z".to_string()),
            },
            relationships: None,
        };
        assert_eq!(ws.updated_at(), "2025-01-01T00:00:00Z");
    }

    #[test]
    fn test_workspace_updated_at_default() {
        let ws = create_test_workspace("test", false);
        assert_eq!(ws.updated_at(), "");
    }

    #[test]
    fn test_workspace_organization_name() {
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
            relationships: Some(WorkspaceRelationships {
                project: None,
                organization: Some(RelationshipData {
                    data: Some(RelationshipId {
                        id: "my-org".to_string(),
                        rel_type: Some("organizations".to_string()),
                    }),
                }),
            }),
        };
        assert_eq!(ws.organization_name(), Some("my-org"));
    }

    #[test]
    fn test_workspace_organization_name_none() {
        let ws = create_test_workspace("test", false);
        assert_eq!(ws.organization_name(), None);
    }

    #[test]
    fn test_workspace_is_locked_default() {
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
            relationships: None,
        };
        assert!(!ws.is_locked());
    }

    #[test]
    fn test_workspace_tfe_resource_trait() {
        let ws = create_test_workspace("my-workspace", false);
        assert_eq!(ws.id(), "ws-my-workspace");
        assert_eq!(ws.name(), "my-workspace");
        assert!(ws.matches("ws-my-workspace"));
        assert!(ws.matches("my-workspace"));
        assert!(!ws.matches("other"));
    }

    #[test]
    fn test_workspaces_response_deserialization() {
        let json = r#"{
            "data": [
                {
                    "id": "ws-1",
                    "attributes": {
                        "name": "workspace-1"
                    }
                },
                {
                    "id": "ws-2",
                    "attributes": {
                        "name": "workspace-2"
                    }
                }
            ]
        }"#;

        let response: WorkspacesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.data.len(), 2);
        assert_eq!(response.data[0].name(), "workspace-1");
        assert_eq!(response.data[1].name(), "workspace-2");
        assert!(response.meta.is_none());
    }
}
