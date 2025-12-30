//! Project data models

use serde::Deserialize;

use crate::hcp::traits::TfeResource;
use crate::hcp::PaginationMeta;

/// Response wrapper for projects list
#[derive(Deserialize, Debug)]
pub struct ProjectsResponse {
    pub data: Vec<Project>,
    #[serde(default)]
    pub meta: Option<PaginationMeta>,
}

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
}
