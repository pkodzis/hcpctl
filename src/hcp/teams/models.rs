//! Team data models

use serde::{Deserialize, Serialize};

use crate::hcp::traits::TfeResource;

/// Team data from TFE API
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Team {
    pub id: String,
    #[serde(rename = "type")]
    pub team_type: Option<String>,
    pub attributes: Option<TeamAttributes>,
    pub relationships: Option<TeamRelationships>,
}

/// Team attributes from TFE API
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TeamAttributes {
    pub name: Option<String>,
    #[serde(rename = "sso-team-id")]
    pub sso_team_id: Option<String>,
    #[serde(rename = "users-count")]
    pub users_count: Option<u32>,
    pub visibility: Option<String>,
    #[serde(rename = "allow-member-token-management")]
    pub allow_member_token_management: Option<bool>,
    pub permissions: Option<TeamPermissions>,
    #[serde(rename = "organization-access")]
    pub organization_access: Option<OrganizationAccess>,
}

/// Team permissions
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TeamPermissions {
    #[serde(rename = "can-update-membership")]
    pub can_update_membership: Option<bool>,
    #[serde(rename = "can-destroy")]
    pub can_destroy: Option<bool>,
    #[serde(rename = "can-update-organization-access")]
    pub can_update_organization_access: Option<bool>,
    #[serde(rename = "can-update-api-token")]
    pub can_update_api_token: Option<bool>,
    #[serde(rename = "can-update-visibility")]
    pub can_update_visibility: Option<bool>,
}

/// Organization-level access permissions for the team
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct OrganizationAccess {
    #[serde(rename = "manage-policies")]
    pub manage_policies: Option<bool>,
    #[serde(rename = "manage-policy-overrides")]
    pub manage_policy_overrides: Option<bool>,
    #[serde(rename = "manage-run-tasks")]
    pub manage_run_tasks: Option<bool>,
    #[serde(rename = "manage-workspaces")]
    pub manage_workspaces: Option<bool>,
    #[serde(rename = "manage-vcs-settings")]
    pub manage_vcs_settings: Option<bool>,
    #[serde(rename = "manage-agent-pools")]
    pub manage_agent_pools: Option<bool>,
    #[serde(rename = "manage-projects")]
    pub manage_projects: Option<bool>,
    #[serde(rename = "read-projects")]
    pub read_projects: Option<bool>,
    #[serde(rename = "read-workspaces")]
    pub read_workspaces: Option<bool>,
    #[serde(rename = "manage-membership")]
    pub manage_membership: Option<bool>,
    #[serde(rename = "manage-teams")]
    pub manage_teams: Option<bool>,
    #[serde(rename = "manage-organization-access")]
    pub manage_organization_access: Option<bool>,
}

/// Team relationships from TFE API
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TeamRelationships {
    pub users: Option<TeamUsersRelationship>,
    #[serde(rename = "authentication-token")]
    pub authentication_token: Option<serde_json::Value>,
}

/// Users relationship
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TeamUsersRelationship {
    pub data: Option<Vec<TeamUserRef>>,
}

/// User reference in team
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TeamUserRef {
    pub id: String,
    #[serde(rename = "type")]
    pub ref_type: Option<String>,
}

impl Team {
    /// Get team name from attributes
    pub fn name(&self) -> &str {
        self.attributes
            .as_ref()
            .and_then(|a| a.name.as_deref())
            .unwrap_or("")
    }

    /// Get users count from attributes
    pub fn users_count(&self) -> u32 {
        self.attributes
            .as_ref()
            .and_then(|a| a.users_count)
            .unwrap_or(0)
    }

    /// Get visibility from attributes
    pub fn visibility(&self) -> &str {
        self.attributes
            .as_ref()
            .and_then(|a| a.visibility.as_deref())
            .unwrap_or("secret")
    }

    /// Get SSO team ID from attributes
    pub fn sso_team_id(&self) -> &str {
        self.attributes
            .as_ref()
            .and_then(|a| a.sso_team_id.as_deref())
            .unwrap_or("")
    }

    /// Check if team has manage-workspaces permission
    pub fn can_manage_workspaces(&self) -> bool {
        self.attributes
            .as_ref()
            .and_then(|a| a.organization_access.as_ref())
            .and_then(|oa| oa.manage_workspaces)
            .unwrap_or(false)
    }
}

impl TfeResource for Team {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        self.name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_team() {
        let json = r#"{
            "id": "team-6p5jTwJQXwqZBncC",
            "type": "teams",
            "attributes": {
                "name": "owners",
                "sso-team-id": null,
                "users-count": 3,
                "visibility": "organization",
                "allow-member-token-management": true,
                "permissions": {
                    "can-update-membership": true,
                    "can-destroy": false,
                    "can-update-organization-access": true,
                    "can-update-api-token": true,
                    "can-update-visibility": true
                },
                "organization-access": {
                    "manage-policies": true,
                    "manage-policy-overrides": true,
                    "manage-run-tasks": true,
                    "manage-workspaces": true,
                    "manage-vcs-settings": true,
                    "manage-agent-pools": true,
                    "manage-projects": true,
                    "read-projects": true,
                    "read-workspaces": true,
                    "manage-membership": true,
                    "manage-teams": true,
                    "manage-organization-access": true
                }
            },
            "relationships": {
                "users": {
                    "data": [
                        {"id": "user-abc", "type": "users"},
                        {"id": "user-def", "type": "users"}
                    ]
                },
                "authentication-token": {
                    "meta": {}
                }
            }
        }"#;

        let team: Team = serde_json::from_str(json).unwrap();
        assert_eq!(team.id, "team-6p5jTwJQXwqZBncC");
        assert_eq!(team.name(), "owners");
        assert_eq!(team.users_count(), 3);
        assert_eq!(team.visibility(), "organization");
        assert!(team.can_manage_workspaces());
    }

    #[test]
    fn test_deserialize_teams_response() {
        let json = r#"{
            "data": [
                {
                    "id": "team-abc",
                    "type": "teams",
                    "attributes": {
                        "name": "owners",
                        "users-count": 2,
                        "visibility": "organization"
                    }
                },
                {
                    "id": "team-def",
                    "type": "teams",
                    "attributes": {
                        "name": "developers",
                        "users-count": 5,
                        "visibility": "secret"
                    }
                }
            ]
        }"#;

        let response: crate::hcp::traits::ApiListResponse<Team> =
            serde_json::from_str(json).unwrap();
        assert_eq!(response.data.len(), 2);
        assert_eq!(response.data[0].name(), "owners");
        assert_eq!(response.data[1].name(), "developers");
    }

    #[test]
    fn test_team_defaults() {
        let json = r#"{
            "id": "team-minimal",
            "type": "teams"
        }"#;

        let team: Team = serde_json::from_str(json).unwrap();
        assert_eq!(team.id, "team-minimal");
        assert_eq!(team.name(), "");
        assert_eq!(team.users_count(), 0);
        assert_eq!(team.visibility(), "secret");
        assert!(!team.can_manage_workspaces());
    }

    #[test]
    fn test_tfe_resource_trait() {
        let json = r#"{
            "id": "team-xyz",
            "type": "teams",
            "attributes": {"name": "test-team"}
        }"#;

        let team: Team = serde_json::from_str(json).unwrap();
        assert_eq!(TfeResource::id(&team), "team-xyz");
        assert_eq!(TfeResource::name(&team), "test-team");
        // Test matches() default impl
        assert!(team.matches("team-xyz"));
        assert!(team.matches("test-team"));
        assert!(!team.matches("other"));
    }
}
