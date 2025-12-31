//! Organization data models

use serde::Deserialize;

use crate::hcp::traits::TfeResource;

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
    pub attributes: Option<OrganizationAttributes>,
    pub relationships: Option<OrganizationRelationships>,
}

/// Organization attributes from TFE API
#[derive(Deserialize, Debug, Clone)]
pub struct OrganizationAttributes {
    pub name: Option<String>,
    pub email: Option<String>,
    #[serde(rename = "external-id")]
    pub external_id: Option<String>,
    #[serde(rename = "created-at")]
    pub created_at: Option<String>,
    #[serde(rename = "saml-enabled")]
    pub saml_enabled: Option<bool>,
}

/// Organization relationships from TFE API
#[derive(Deserialize, Debug, Clone)]
pub struct OrganizationRelationships {
    #[serde(rename = "default-project")]
    pub default_project: Option<RelationshipData>,
    #[serde(rename = "oauth-tokens")]
    pub oauth_tokens: Option<RelationshipLink>,
}

/// Relationship with data containing id
#[derive(Deserialize, Debug, Clone)]
pub struct RelationshipData {
    pub data: Option<RelationshipDataInner>,
}

/// Inner relationship data with id and type
#[derive(Deserialize, Debug, Clone)]
pub struct RelationshipDataInner {
    pub id: String,
    #[serde(rename = "type")]
    pub data_type: Option<String>,
}

/// Relationship with links only
#[derive(Deserialize, Debug, Clone)]
pub struct RelationshipLink {
    pub links: Option<RelationshipLinks>,
}

/// Links in a relationship
#[derive(Deserialize, Debug, Clone)]
pub struct RelationshipLinks {
    pub related: Option<String>,
}

impl Organization {
    /// Get email from attributes
    pub fn email(&self) -> &str {
        self.attributes
            .as_ref()
            .and_then(|a| a.email.as_deref())
            .unwrap_or("")
    }

    /// Get external ID from attributes
    pub fn external_id(&self) -> &str {
        self.attributes
            .as_ref()
            .and_then(|a| a.external_id.as_deref())
            .unwrap_or("")
    }

    /// Get created_at from attributes
    pub fn created_at(&self) -> &str {
        self.attributes
            .as_ref()
            .and_then(|a| a.created_at.as_deref())
            .unwrap_or("")
    }

    /// Get saml_enabled from attributes
    pub fn saml_enabled(&self) -> bool {
        self.attributes
            .as_ref()
            .and_then(|a| a.saml_enabled)
            .unwrap_or(false)
    }

    /// Get default project ID from relationships
    pub fn default_project_id(&self) -> Option<&str> {
        self.relationships
            .as_ref()
            .and_then(|r| r.default_project.as_ref())
            .and_then(|dp| dp.data.as_ref())
            .map(|d| d.id.as_str())
    }

    /// Get oauth tokens link from relationships
    pub fn oauth_tokens_link(&self) -> Option<&str> {
        self.relationships
            .as_ref()
            .and_then(|r| r.oauth_tokens.as_ref())
            .and_then(|ot| ot.links.as_ref())
            .and_then(|l| l.related.as_deref())
    }
}

impl TfeResource for Organization {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        // For orgs, id and name are the same
        &self.id
    }

    /// Override matches to also check external_id
    ///
    /// HCP API has inconsistent naming:
    /// - `id` = organization name (e.g., "my-org")
    /// - `external-id` = actual ID (e.g., "org-ABC123")
    ///
    /// This allows users to look up orgs by either.
    fn matches(&self, input: &str) -> bool {
        self.id() == input || self.name() == input || self.external_id() == input
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_org() -> Organization {
        Organization {
            id: "my-org".to_string(),
            org_type: Some("organizations".to_string()),
            attributes: Some(OrganizationAttributes {
                name: Some("my-org".to_string()),
                email: Some("test@example.com".to_string()),
                external_id: Some("org-123".to_string()),
                created_at: Some("2025-01-01T00:00:00Z".to_string()),
                saml_enabled: Some(true),
            }),
            relationships: Some(OrganizationRelationships {
                default_project: Some(RelationshipData {
                    data: Some(RelationshipDataInner {
                        id: "prj-123".to_string(),
                        data_type: Some("projects".to_string()),
                    }),
                }),
                oauth_tokens: Some(RelationshipLink {
                    links: Some(RelationshipLinks {
                        related: Some("/api/v2/organizations/my-org/oauth-tokens".to_string()),
                    }),
                }),
            }),
        }
    }

    #[test]
    fn test_organization_name() {
        let org = create_test_org();
        assert_eq!(org.name(), "my-org");
    }

    #[test]
    fn test_organization_email() {
        let org = create_test_org();
        assert_eq!(org.email(), "test@example.com");
    }

    #[test]
    fn test_organization_external_id() {
        let org = create_test_org();
        assert_eq!(org.external_id(), "org-123");
    }

    #[test]
    fn test_organization_created_at() {
        let org = create_test_org();
        assert_eq!(org.created_at(), "2025-01-01T00:00:00Z");
    }

    #[test]
    fn test_organization_saml_enabled() {
        let org = create_test_org();
        assert!(org.saml_enabled());
    }

    #[test]
    fn test_organization_default_project_id() {
        let org = create_test_org();
        assert_eq!(org.default_project_id(), Some("prj-123"));
    }

    #[test]
    fn test_organization_oauth_tokens_link() {
        let org = create_test_org();
        assert_eq!(
            org.oauth_tokens_link(),
            Some("/api/v2/organizations/my-org/oauth-tokens")
        );
    }

    #[test]
    fn test_organization_matches() {
        let org = Organization {
            id: "my-org".to_string(),
            org_type: None,
            attributes: None,
            relationships: None,
        };
        assert!(org.matches("my-org"));
        assert!(!org.matches("other"));
    }

    #[test]
    fn test_organization_matches_by_external_id() {
        let org = create_test_org();
        // Should match by name
        assert!(org.matches("my-org"));
        // Should match by external_id (org-123)
        assert!(org.matches("org-123"));
        // Should not match random string
        assert!(!org.matches("other"));
        assert!(!org.matches("org-999"));
    }

    #[test]
    fn test_organization_defaults() {
        let org = Organization {
            id: "my-org".to_string(),
            org_type: None,
            attributes: None,
            relationships: None,
        };
        assert_eq!(org.email(), "");
        assert_eq!(org.external_id(), "");
        assert_eq!(org.created_at(), "");
        assert!(!org.saml_enabled());
        assert_eq!(org.default_project_id(), None);
        assert_eq!(org.oauth_tokens_link(), None);
    }

    #[test]
    fn test_organization_deserialization() {
        let json = r#"{
            "id": "my-org",
            "type": "organizations",
            "attributes": {
                "name": "my-org",
                "email": "admin@example.com",
                "external-id": "org-ABC123",
                "created-at": "2025-01-01T00:00:00Z",
                "saml-enabled": true
            }
        }"#;

        let org: Organization = serde_json::from_str(json).unwrap();
        assert_eq!(org.id, "my-org");
        assert_eq!(org.email(), "admin@example.com");
        assert_eq!(org.external_id(), "org-ABC123");
        assert!(org.saml_enabled());
    }

    #[test]
    fn test_organization_deserialization_minimal() {
        let json = r#"{"id": "minimal-org"}"#;

        let org: Organization = serde_json::from_str(json).unwrap();
        assert_eq!(org.id, "minimal-org");
        assert_eq!(org.email(), "");
        assert!(org.org_type.is_none());
    }

    #[test]
    fn test_organizations_response_deserialization() {
        let json = r#"{
            "data": [
                {"id": "org-1"},
                {"id": "org-2"}
            ]
        }"#;

        let response: OrganizationsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.data.len(), 2);
        assert_eq!(response.data[0].id, "org-1");
        assert_eq!(response.data[1].id, "org-2");
    }

    #[test]
    fn test_organization_with_relationships() {
        let json = r#"{
            "id": "my-org",
            "relationships": {
                "default-project": {
                    "data": {
                        "id": "prj-default",
                        "type": "projects"
                    }
                },
                "oauth-tokens": {
                    "links": {
                        "related": "/api/v2/organizations/my-org/oauth-tokens"
                    }
                }
            }
        }"#;

        let org: Organization = serde_json::from_str(json).unwrap();
        assert_eq!(org.default_project_id(), Some("prj-default"));
        assert_eq!(
            org.oauth_tokens_link(),
            Some("/api/v2/organizations/my-org/oauth-tokens")
        );
    }
}
