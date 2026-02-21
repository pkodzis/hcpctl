//! Project data models

use serde::Deserialize;

use crate::hcp::traits::TfeResource;

/// Project data from TFE API
#[derive(Deserialize, Debug, Clone)]
pub struct Project {
    pub id: String,
    #[serde(rename = "type")]
    pub project_type: Option<String>,
    pub attributes: ProjectAttributes,
}

/// Project attributes from TFE API
#[derive(Deserialize, Debug, Clone)]
pub struct ProjectAttributes {
    pub name: String,
    pub description: Option<String>,
}

impl TfeResource for Project {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.attributes.name
    }
}

impl Project {
    /// Get the project description
    pub fn description(&self) -> &str {
        self.attributes.description.as_deref().unwrap_or("")
    }
}

/// Workspace info for a project
#[derive(Debug, Clone, Default)]
pub struct ProjectWorkspaces {
    /// Full workspace objects
    pub workspaces: Vec<crate::hcp::workspaces::Workspace>,
}

impl ProjectWorkspaces {
    pub fn new() -> Self {
        Self {
            workspaces: Vec::new(),
        }
    }

    pub fn from_workspaces(ws_list: Vec<crate::hcp::workspaces::Workspace>) -> Self {
        Self {
            workspaces: ws_list,
        }
    }

    pub fn count(&self) -> usize {
        self.workspaces.len()
    }

    pub fn names(&self) -> Vec<&str> {
        self.workspaces
            .iter()
            .map(|ws| ws.attributes.name.as_str())
            .collect()
    }

    pub fn ids(&self) -> Vec<&str> {
        self.workspaces.iter().map(|ws| ws.id.as_str()).collect()
    }

    /// Returns "name (id), ..." format
    pub fn name_id_pairs(&self) -> Vec<String> {
        self.workspaces
            .iter()
            .map(|ws| format!("{} ({})", ws.attributes.name, ws.id))
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.workspaces.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_project(id: &str, name: &str) -> Project {
        Project {
            id: id.to_string(),
            project_type: Some("projects".to_string()),
            attributes: ProjectAttributes {
                name: name.to_string(),
                description: None,
            },
        }
    }

    #[test]
    fn test_project_name() {
        let prj = create_test_project("prj-123", "my-project");
        assert_eq!(prj.name(), "my-project");
    }

    #[test]
    fn test_project_matches_by_id() {
        let prj = create_test_project("prj-123", "my-project");
        assert!(prj.matches("prj-123"));
    }

    #[test]
    fn test_project_matches_by_name() {
        let prj = create_test_project("prj-123", "my-project");
        assert!(prj.matches("my-project"));
    }

    #[test]
    fn test_project_no_match() {
        let prj = create_test_project("prj-123", "my-project");
        assert!(!prj.matches("other"));
    }

    #[test]
    fn test_project_description() {
        let mut prj = create_test_project("prj-123", "my-project");
        assert_eq!(prj.description(), "");

        prj.attributes.description = Some("A test project".to_string());
        assert_eq!(prj.description(), "A test project");
    }

    #[test]
    fn test_project_tfe_resource_trait() {
        let prj = create_test_project("prj-123", "my-project");
        assert_eq!(prj.id(), "prj-123");
        assert_eq!(prj.name(), "my-project");
    }

    #[test]
    fn test_project_deserialization() {
        let json = r#"{
            "id": "prj-abc123",
            "type": "projects",
            "attributes": {
                "name": "my-project",
                "description": "Test project"
            }
        }"#;

        let prj: Project = serde_json::from_str(json).unwrap();
        assert_eq!(prj.id, "prj-abc123");
        assert_eq!(prj.name(), "my-project");
        assert_eq!(prj.description(), "Test project");
        assert_eq!(prj.project_type, Some("projects".to_string()));
    }

    #[test]
    fn test_project_deserialization_minimal() {
        let json = r#"{
            "id": "prj-abc123",
            "attributes": {
                "name": "my-project"
            }
        }"#;

        let prj: Project = serde_json::from_str(json).unwrap();
        assert_eq!(prj.id, "prj-abc123");
        assert_eq!(prj.name(), "my-project");
        assert_eq!(prj.description(), "");
        assert!(prj.project_type.is_none());
    }

    #[test]
    fn test_projects_response_deserialization() {
        let json = r#"{
            "data": [
                {
                    "id": "prj-1",
                    "attributes": {
                        "name": "project-1"
                    }
                },
                {
                    "id": "prj-2",
                    "attributes": {
                        "name": "project-2"
                    }
                }
            ]
        }"#;

        let response: crate::hcp::traits::ApiListResponse<Project> =
            serde_json::from_str(json).unwrap();
        assert_eq!(response.data.len(), 2);
        assert_eq!(response.data[0].name(), "project-1");
        assert_eq!(response.data[1].name(), "project-2");
        assert!(response.meta.is_none());
    }

    // ===== ProjectWorkspaces tests =====

    #[test]
    fn test_project_workspaces_new() {
        let pw = ProjectWorkspaces::new();
        assert!(pw.is_empty());
        assert_eq!(pw.count(), 0);
    }

    #[test]
    fn test_project_workspaces_from_workspaces() {
        use crate::hcp::workspaces::{Workspace, WorkspaceAttributes};

        let workspaces = vec![
            Workspace {
                id: "ws-1".to_string(),
                attributes: WorkspaceAttributes {
                    name: "workspace-1".to_string(),
                    execution_mode: None,
                    resource_count: None,
                    locked: None,
                    terraform_version: None,
                    updated_at: None,
                },
                relationships: None,
            },
            Workspace {
                id: "ws-2".to_string(),
                attributes: WorkspaceAttributes {
                    name: "workspace-2".to_string(),
                    execution_mode: None,
                    resource_count: None,
                    locked: None,
                    terraform_version: None,
                    updated_at: None,
                },
                relationships: None,
            },
        ];

        let pw = ProjectWorkspaces::from_workspaces(workspaces);
        assert!(!pw.is_empty());
        assert_eq!(pw.count(), 2);
    }

    #[test]
    fn test_project_workspaces_names() {
        use crate::hcp::workspaces::{Workspace, WorkspaceAttributes};

        let workspaces = vec![
            Workspace {
                id: "ws-1".to_string(),
                attributes: WorkspaceAttributes {
                    name: "alpha".to_string(),
                    execution_mode: None,
                    resource_count: None,
                    locked: None,
                    terraform_version: None,
                    updated_at: None,
                },
                relationships: None,
            },
            Workspace {
                id: "ws-2".to_string(),
                attributes: WorkspaceAttributes {
                    name: "beta".to_string(),
                    execution_mode: None,
                    resource_count: None,
                    locked: None,
                    terraform_version: None,
                    updated_at: None,
                },
                relationships: None,
            },
        ];

        let pw = ProjectWorkspaces::from_workspaces(workspaces);
        let names = pw.names();
        assert_eq!(names, vec!["alpha", "beta"]);
    }

    #[test]
    fn test_project_workspaces_ids() {
        use crate::hcp::workspaces::{Workspace, WorkspaceAttributes};

        let workspaces = vec![
            Workspace {
                id: "ws-abc".to_string(),
                attributes: WorkspaceAttributes {
                    name: "alpha".to_string(),
                    execution_mode: None,
                    resource_count: None,
                    locked: None,
                    terraform_version: None,
                    updated_at: None,
                },
                relationships: None,
            },
            Workspace {
                id: "ws-xyz".to_string(),
                attributes: WorkspaceAttributes {
                    name: "beta".to_string(),
                    execution_mode: None,
                    resource_count: None,
                    locked: None,
                    terraform_version: None,
                    updated_at: None,
                },
                relationships: None,
            },
        ];

        let pw = ProjectWorkspaces::from_workspaces(workspaces);
        let ids = pw.ids();
        assert_eq!(ids, vec!["ws-abc", "ws-xyz"]);
    }

    #[test]
    fn test_project_workspaces_name_id_pairs() {
        use crate::hcp::workspaces::{Workspace, WorkspaceAttributes};

        let workspaces = vec![Workspace {
            id: "ws-123".to_string(),
            attributes: WorkspaceAttributes {
                name: "my-workspace".to_string(),
                execution_mode: None,
                resource_count: None,
                locked: None,
                terraform_version: None,
                updated_at: None,
            },
            relationships: None,
        }];

        let pw = ProjectWorkspaces::from_workspaces(workspaces);
        let pairs = pw.name_id_pairs();
        assert_eq!(pairs, vec!["my-workspace (ws-123)"]);
    }
}
