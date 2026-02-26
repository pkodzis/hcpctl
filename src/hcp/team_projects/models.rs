//! Team project access data models

use serde::{Deserialize, Serialize};

use crate::hcp::traits::TfeResource;

/// Team project access binding from TFE API
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TeamProjectAccess {
    pub id: String,
    #[serde(rename = "type")]
    pub access_type: Option<String>,
    pub attributes: TeamProjectAccessAttributes,
    pub relationships: Option<TeamProjectAccessRelationships>,
}

/// Team project access attributes
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TeamProjectAccessAttributes {
    pub access: Option<String>,
    #[serde(rename = "project-access")]
    pub project_access: Option<ProjectAccessPermissions>,
    #[serde(rename = "workspace-access")]
    pub workspace_access: Option<WorkspaceAccessPermissions>,
}

/// Custom project-level access permissions
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ProjectAccessPermissions {
    pub settings: Option<String>,
    pub teams: Option<String>,
}

/// Custom workspace-level access permissions
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct WorkspaceAccessPermissions {
    pub runs: Option<String>,
    pub variables: Option<String>,
    #[serde(rename = "state-versions")]
    pub state_versions: Option<String>,
    #[serde(rename = "sentinel-mocks")]
    pub sentinel_mocks: Option<String>,
    pub create: Option<bool>,
    #[serde(rename = "move")]
    pub move_workspace: Option<bool>,
    pub locking: Option<bool>,
    pub delete: Option<bool>,
    #[serde(rename = "run-tasks")]
    pub run_tasks: Option<bool>,
}

/// Relationships for team project access
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TeamProjectAccessRelationships {
    pub team: Option<RelationshipRef>,
    pub project: Option<RelationshipRef>,
}

/// A relationship reference
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RelationshipRef {
    pub data: Option<RelationshipData>,
}

/// Relationship data containing ID and type
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RelationshipData {
    pub id: String,
    #[serde(rename = "type")]
    pub data_type: Option<String>,
}

impl TeamProjectAccess {
    /// Get the access level
    pub fn access(&self) -> &str {
        self.attributes.access.as_deref().unwrap_or("")
    }

    /// Get the team ID from relationships
    pub fn team_id(&self) -> &str {
        self.relationships
            .as_ref()
            .and_then(|r| r.team.as_ref())
            .and_then(|t| t.data.as_ref())
            .map(|d| d.id.as_str())
            .unwrap_or("")
    }

    /// Get the project ID from relationships
    pub fn project_id(&self) -> &str {
        self.relationships
            .as_ref()
            .and_then(|r| r.project.as_ref())
            .and_then(|p| p.data.as_ref())
            .map(|d| d.id.as_str())
            .unwrap_or("")
    }
}

impl TfeResource for TeamProjectAccess {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        // Team-project access bindings don't have a human-readable name;
        // use ID for trait compliance (matches() will match by ID)
        &self.id
    }
}

/// Enriched team project access with resolved names for display
#[derive(Debug, Clone, Serialize)]
pub struct EnrichedTeamProjectAccess {
    pub id: String,
    pub team_id: String,
    pub team_name: String,
    pub project_id: String,
    pub project_name: String,
    pub access: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_team_project_access_json() -> serde_json::Value {
        serde_json::json!({
            "id": "tprj-abc123",
            "type": "team-projects",
            "attributes": {
                "access": "read",
                "project-access": {
                    "settings": "read",
                    "teams": "none"
                },
                "workspace-access": {
                    "runs": "read",
                    "variables": "read",
                    "state-versions": "read-outputs",
                    "sentinel-mocks": "none",
                    "create": false,
                    "move": false,
                    "locking": false,
                    "delete": false,
                    "run-tasks": false
                }
            },
            "relationships": {
                "team": {
                    "data": {
                        "id": "team-abc",
                        "type": "teams"
                    }
                },
                "project": {
                    "data": {
                        "id": "prj-def",
                        "type": "projects"
                    }
                }
            }
        })
    }

    #[test]
    fn test_deserialize_team_project_access() {
        let json = sample_team_project_access_json();
        let access: TeamProjectAccess = serde_json::from_value(json).unwrap();

        assert_eq!(access.id, "tprj-abc123");
        assert_eq!(access.access(), "read");
        assert_eq!(access.team_id(), "team-abc");
        assert_eq!(access.project_id(), "prj-def");
    }

    #[test]
    fn test_tfe_resource_trait() {
        let json = sample_team_project_access_json();
        let access: TeamProjectAccess = serde_json::from_value(json).unwrap();

        assert_eq!(TfeResource::id(&access), "tprj-abc123");
        assert_eq!(TfeResource::name(&access), "tprj-abc123");
        assert!(access.matches("tprj-abc123"));
        assert!(!access.matches("read"));
        assert!(!access.matches("other"));
    }

    #[test]
    fn test_deserialize_minimal() {
        let json = serde_json::json!({
            "id": "tprj-minimal",
            "type": "team-projects",
            "attributes": {}
        });

        let access: TeamProjectAccess = serde_json::from_value(json).unwrap();
        assert_eq!(access.id, "tprj-minimal");
        assert_eq!(access.access(), "");
        assert_eq!(access.team_id(), "");
        assert_eq!(access.project_id(), "");
    }

    #[test]
    fn test_deserialize_with_custom_access() {
        let json = serde_json::json!({
            "id": "tprj-custom",
            "type": "team-projects",
            "attributes": {
                "access": "custom",
                "project-access": {
                    "settings": "update",
                    "teams": "manage"
                },
                "workspace-access": {
                    "runs": "apply",
                    "variables": "write",
                    "state-versions": "read-outputs",
                    "sentinel-mocks": "read",
                    "create": true,
                    "move": true,
                    "locking": true,
                    "delete": false,
                    "run-tasks": true
                }
            },
            "relationships": {
                "team": {
                    "data": {
                        "id": "team-xyz",
                        "type": "teams"
                    }
                },
                "project": {
                    "data": {
                        "id": "prj-789",
                        "type": "projects"
                    }
                }
            }
        });

        let access: TeamProjectAccess = serde_json::from_value(json).unwrap();
        assert_eq!(access.access(), "custom");

        let pa = access.attributes.project_access.as_ref().unwrap();
        assert_eq!(pa.settings.as_deref(), Some("update"));
        assert_eq!(pa.teams.as_deref(), Some("manage"));

        let wa = access.attributes.workspace_access.as_ref().unwrap();
        assert_eq!(wa.runs.as_deref(), Some("apply"));
        assert_eq!(wa.variables.as_deref(), Some("write"));
        assert_eq!(wa.create, Some(true));
        assert_eq!(wa.delete, Some(false));
    }

    #[test]
    fn test_deserialize_list_response() {
        let json = serde_json::json!({
            "data": [
                {
                    "id": "tprj-1",
                    "type": "team-projects",
                    "attributes": {
                        "access": "admin"
                    },
                    "relationships": {
                        "team": { "data": { "id": "team-a", "type": "teams" } },
                        "project": { "data": { "id": "prj-1", "type": "projects" } }
                    }
                },
                {
                    "id": "tprj-2",
                    "type": "team-projects",
                    "attributes": {
                        "access": "read"
                    },
                    "relationships": {
                        "team": { "data": { "id": "team-b", "type": "teams" } },
                        "project": { "data": { "id": "prj-1", "type": "projects" } }
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
        });

        let response: crate::hcp::traits::ApiListResponse<TeamProjectAccess> =
            serde_json::from_value(json).unwrap();
        assert_eq!(response.data.len(), 2);
        assert_eq!(response.data[0].access(), "admin");
        assert_eq!(response.data[1].access(), "read");
    }

    #[test]
    fn test_enriched_team_project_access_serialization() {
        let enriched = EnrichedTeamProjectAccess {
            id: "tprj-abc".to_string(),
            team_id: "team-1".to_string(),
            team_name: "developers".to_string(),
            project_id: "prj-1".to_string(),
            project_name: "my-project".to_string(),
            access: "write".to_string(),
        };

        let json = serde_json::to_string(&enriched).unwrap();
        assert!(json.contains("\"team_name\":\"developers\""));
        assert!(json.contains("\"project_name\":\"my-project\""));
        assert!(json.contains("\"access\":\"write\""));
    }

    #[test]
    fn test_team_id_with_no_project_relationship() {
        let json = serde_json::json!({
            "id": "tprj-partial",
            "type": "team-projects",
            "attributes": { "access": "read" },
            "relationships": {
                "team": { "data": { "id": "team-abc", "type": "teams" } }
            }
        });

        let access: TeamProjectAccess = serde_json::from_value(json).unwrap();
        assert_eq!(access.team_id(), "team-abc");
        assert_eq!(access.project_id(), "");
    }

    #[test]
    fn test_project_id_with_no_team_relationship() {
        let json = serde_json::json!({
            "id": "tprj-partial",
            "type": "team-projects",
            "attributes": { "access": "write" },
            "relationships": {
                "project": { "data": { "id": "prj-xyz", "type": "projects" } }
            }
        });

        let access: TeamProjectAccess = serde_json::from_value(json).unwrap();
        assert_eq!(access.team_id(), "");
        assert_eq!(access.project_id(), "prj-xyz");
    }

    #[test]
    fn test_relationship_with_null_data() {
        let json = serde_json::json!({
            "id": "tprj-null",
            "type": "team-projects",
            "attributes": { "access": "admin" },
            "relationships": {
                "team": { "data": null },
                "project": { "data": null }
            }
        });

        let access: TeamProjectAccess = serde_json::from_value(json).unwrap();
        assert_eq!(access.team_id(), "");
        assert_eq!(access.project_id(), "");
    }

    #[test]
    fn test_access_returns_empty_for_none() {
        let json = serde_json::json!({
            "id": "tprj-no-access",
            "type": "team-projects",
            "attributes": {}
        });

        let access: TeamProjectAccess = serde_json::from_value(json).unwrap();
        assert_eq!(access.access(), "");
    }

    #[test]
    fn test_matches_by_id() {
        let json = sample_team_project_access_json();
        let access: TeamProjectAccess = serde_json::from_value(json).unwrap();
        assert!(access.matches("tprj-abc123"));
    }

    #[test]
    fn test_does_not_match_by_access_level() {
        let json = sample_team_project_access_json();
        let access: TeamProjectAccess = serde_json::from_value(json).unwrap();
        assert!(!access.matches("read"));
    }

    #[test]
    fn test_does_not_match_unrelated() {
        let json = sample_team_project_access_json();
        let access: TeamProjectAccess = serde_json::from_value(json).unwrap();
        assert!(!access.matches("nonexistent"));
    }
}
