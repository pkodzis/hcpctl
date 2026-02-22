//! Organization membership data models

use serde::{Deserialize, Serialize};

/// Response wrapper for single organization membership
#[derive(Deserialize, Debug)]
pub struct OrganizationMembershipResponse {
    pub data: OrganizationMembership,
}

/// Organization membership data from TFE API
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct OrganizationMembership {
    pub id: String,
    #[serde(rename = "type")]
    pub membership_type: Option<String>,
    pub attributes: Option<OrganizationMembershipAttributes>,
    pub relationships: Option<OrganizationMembershipRelationships>,
}

/// Organization membership attributes from TFE API
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct OrganizationMembershipAttributes {
    pub email: Option<String>,
    pub status: Option<String>,
    #[serde(rename = "created-at")]
    pub created_at: Option<String>,
}

/// Organization membership relationships
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct OrganizationMembershipRelationships {
    pub user: Option<RelationshipData>,
    pub teams: Option<TeamsRelationship>,
}

/// Generic relationship data
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RelationshipData {
    pub data: Option<RelationshipItem>,
}

/// Relationship item with id and type
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RelationshipItem {
    pub id: String,
    #[serde(rename = "type")]
    pub item_type: String,
}

/// Teams relationship (array)
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TeamsRelationship {
    pub data: Vec<RelationshipItem>,
}

impl OrganizationMembership {
    /// Get email from attributes
    pub fn email(&self) -> &str {
        self.attributes
            .as_ref()
            .and_then(|a| a.email.as_deref())
            .unwrap_or("")
    }

    /// Get status from attributes
    pub fn status(&self) -> &str {
        self.attributes
            .as_ref()
            .and_then(|a| a.status.as_deref())
            .unwrap_or("unknown")
    }

    /// Get created_at from attributes
    pub fn created_at(&self) -> &str {
        self.attributes
            .as_ref()
            .and_then(|a| a.created_at.as_deref())
            .unwrap_or("")
    }

    /// Get user ID from relationships
    pub fn user_id(&self) -> Option<&str> {
        self.relationships
            .as_ref()
            .and_then(|r| r.user.as_ref())
            .and_then(|u| u.data.as_ref())
            .map(|d| d.id.as_str())
    }

    /// Get team IDs from relationships
    pub fn team_ids(&self) -> Vec<&str> {
        self.relationships
            .as_ref()
            .and_then(|r| r.teams.as_ref())
            .map(|t| t.data.iter().map(|d| d.id.as_str()).collect())
            .unwrap_or_default()
    }
}

/// Request payload for inviting a user to an organization
#[derive(Serialize, Debug)]
pub struct InviteUserRequest {
    pub data: InviteUserData,
}

/// Data part of invite request
#[derive(Serialize, Debug)]
pub struct InviteUserData {
    #[serde(rename = "type")]
    pub data_type: String,
    pub attributes: InviteUserAttributes,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationships: Option<InviteUserRelationships>,
}

/// Attributes for invite request
#[derive(Serialize, Debug)]
pub struct InviteUserAttributes {
    pub email: String,
}

/// Relationships for invite request (teams)
#[derive(Serialize, Debug)]
pub struct InviteUserRelationships {
    pub teams: InviteTeamsData,
}

/// Teams data for invite request
#[derive(Serialize, Debug)]
pub struct InviteTeamsData {
    pub data: Vec<TeamRef>,
}

/// Team reference in invite request
#[derive(Serialize, Debug)]
pub struct TeamRef {
    #[serde(rename = "type")]
    pub ref_type: String,
    pub id: String,
}

impl InviteUserRequest {
    /// Create a new invite request for an email
    pub fn new(email: &str) -> Self {
        Self {
            data: InviteUserData {
                data_type: "organization-memberships".to_string(),
                attributes: InviteUserAttributes {
                    email: email.to_string(),
                },
                relationships: None,
            },
        }
    }

    /// Create invite request with team assignments
    pub fn with_teams(email: &str, team_ids: Vec<String>) -> Self {
        let teams_data: Vec<TeamRef> = team_ids
            .into_iter()
            .map(|id| TeamRef {
                ref_type: "teams".to_string(),
                id,
            })
            .collect();

        Self {
            data: InviteUserData {
                data_type: "organization-memberships".to_string(),
                attributes: InviteUserAttributes {
                    email: email.to_string(),
                },
                relationships: if teams_data.is_empty() {
                    None
                } else {
                    Some(InviteUserRelationships {
                        teams: InviteTeamsData { data: teams_data },
                    })
                },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_organization_membership() {
        let json = serde_json::json!({
            "id": "ou-abc123",
            "type": "organization-memberships",
            "attributes": {
                "email": "user@example.com",
                "status": "active",
                "created-at": "2024-01-15T10:30:00Z"
            },
            "relationships": {
                "user": {
                    "data": {
                        "id": "user-xyz",
                        "type": "users"
                    }
                },
                "teams": {
                    "data": [
                        { "id": "team-1", "type": "teams" },
                        { "id": "team-2", "type": "teams" }
                    ]
                }
            }
        });

        let membership: OrganizationMembership = serde_json::from_value(json).unwrap();

        assert_eq!(membership.id, "ou-abc123");
        assert_eq!(membership.email(), "user@example.com");
        assert_eq!(membership.status(), "active");
        assert_eq!(membership.user_id(), Some("user-xyz"));
        assert_eq!(membership.team_ids(), vec!["team-1", "team-2"]);
    }

    #[test]
    fn test_invite_request_simple() {
        let request = InviteUserRequest::new("user@example.com");
        let json = serde_json::to_value(&request).unwrap();

        assert_eq!(json["data"]["type"], "organization-memberships");
        assert_eq!(json["data"]["attributes"]["email"], "user@example.com");
        assert!(json["data"]["relationships"].is_null());
    }

    #[test]
    fn test_invite_request_with_teams() {
        let request = InviteUserRequest::with_teams("user@example.com", vec!["team-1".to_string()]);
        let json = serde_json::to_value(&request).unwrap();

        assert_eq!(json["data"]["type"], "organization-memberships");
        assert_eq!(json["data"]["attributes"]["email"], "user@example.com");
        assert_eq!(
            json["data"]["relationships"]["teams"]["data"][0]["id"],
            "team-1"
        );
        assert_eq!(
            json["data"]["relationships"]["teams"]["data"][0]["type"],
            "teams"
        );
    }

    #[test]
    fn test_membership_defaults() {
        let membership: OrganizationMembership = serde_json::from_value(serde_json::json!({
            "id": "ou-minimal",
            "type": "organization-memberships"
        }))
        .unwrap();

        assert_eq!(membership.id, "ou-minimal");
        assert_eq!(membership.email(), "");
        assert_eq!(membership.status(), "unknown");
        assert_eq!(membership.user_id(), None);
        assert!(membership.team_ids().is_empty());
    }
}
