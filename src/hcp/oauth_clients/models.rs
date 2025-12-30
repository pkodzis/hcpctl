//! OAuth Client data models

use serde::Deserialize;

use crate::hcp::traits::TfeResource;
use crate::hcp::PaginationMeta;

/// Response wrapper for oauth clients list
#[derive(Deserialize, Debug)]
pub struct OAuthClientsResponse {
    pub data: Vec<OAuthClient>,
    #[serde(default)]
    pub meta: Option<PaginationMeta>,
}

/// OAuth Client data from TFE API
#[derive(Deserialize, Debug, Clone)]
pub struct OAuthClient {
    pub id: String,
    #[serde(rename = "type")]
    pub client_type: Option<String>,
    pub attributes: OAuthClientAttributes,
    pub relationships: Option<OAuthClientRelationships>,
}

/// OAuth Client attributes from TFE API
#[derive(Deserialize, Debug, Clone)]
pub struct OAuthClientAttributes {
    #[serde(rename = "created-at")]
    pub created_at: Option<String>,
    #[serde(rename = "service-provider")]
    pub service_provider: Option<String>,
    #[serde(rename = "service-provider-display-name")]
    pub service_provider_display_name: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "http-url")]
    pub http_url: Option<String>,
    #[serde(rename = "api-url")]
    pub api_url: Option<String>,
    #[serde(rename = "callback-url")]
    pub callback_url: Option<String>,
    #[serde(rename = "organization-scoped")]
    pub organization_scoped: Option<bool>,
}

/// OAuth Client relationships from TFE API
#[derive(Deserialize, Debug, Clone)]
pub struct OAuthClientRelationships {
    pub organization: Option<RelationshipData>,
    #[serde(rename = "oauth-tokens")]
    pub oauth_tokens: Option<OAuthTokensRelationship>,
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

/// OAuth Tokens relationship (contains array of tokens)
#[derive(Deserialize, Debug, Clone)]
pub struct OAuthTokensRelationship {
    pub data: Option<Vec<RelationshipId>>,
}

impl TfeResource for OAuthClient {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        self.attributes
            .name
            .as_deref()
            .or(self.attributes.service_provider_display_name.as_deref())
            .unwrap_or(&self.id)
    }
}

impl OAuthClient {
    /// Get service provider (github, gitlab, etc.)
    pub fn service_provider(&self) -> &str {
        self.attributes
            .service_provider
            .as_deref()
            .unwrap_or("unknown")
    }

    /// Get service provider display name (GitHub, GitLab, etc.)
    pub fn service_provider_display_name(&self) -> &str {
        self.attributes
            .service_provider_display_name
            .as_deref()
            .unwrap_or("unknown")
    }

    /// Get HTTP URL of the VCS provider
    pub fn http_url(&self) -> &str {
        self.attributes.http_url.as_deref().unwrap_or("")
    }

    /// Get created at timestamp
    pub fn created_at(&self) -> &str {
        self.attributes.created_at.as_deref().unwrap_or("")
    }

    /// Check if organization scoped
    pub fn is_organization_scoped(&self) -> bool {
        self.attributes.organization_scoped.unwrap_or(true)
    }

    /// Get OAuth token IDs
    pub fn oauth_token_ids(&self) -> Vec<&str> {
        self.relationships
            .as_ref()
            .and_then(|r| r.oauth_tokens.as_ref())
            .and_then(|ot| ot.data.as_ref())
            .map(|tokens| tokens.iter().map(|t| t.id.as_str()).collect())
            .unwrap_or_default()
    }

    /// Get organization ID from relationships
    pub fn organization_id(&self) -> Option<&str> {
        self.relationships
            .as_ref()
            .and_then(|r| r.organization.as_ref())
            .and_then(|o| o.data.as_ref())
            .map(|d| d.id.as_str())
    }
}

// ============================================================================
// OAuth Token models (for fetching from organization's oauth-tokens endpoint)
// ============================================================================

/// Response wrapper for oauth tokens list
#[derive(Deserialize, Debug)]
pub struct OAuthTokensResponse {
    pub data: Vec<OAuthToken>,
}

/// OAuth Token data from TFE API
#[derive(Deserialize, Debug, Clone)]
pub struct OAuthToken {
    pub id: String,
    #[serde(rename = "type")]
    pub token_type: Option<String>,
    pub attributes: Option<OAuthTokenAttributes>,
}

/// OAuth Token attributes from TFE API
#[derive(Deserialize, Debug, Clone)]
pub struct OAuthTokenAttributes {
    #[serde(rename = "created-at")]
    pub created_at: Option<String>,
    #[serde(rename = "service-provider-user")]
    pub service_provider_user: Option<String>,
    #[serde(rename = "has-ssh-key")]
    pub has_ssh_key: Option<bool>,
}

impl OAuthToken {
    /// Get service provider user
    pub fn service_provider_user(&self) -> &str {
        self.attributes
            .as_ref()
            .and_then(|a| a.service_provider_user.as_deref())
            .unwrap_or("")
    }

    /// Get created at timestamp
    pub fn created_at(&self) -> &str {
        self.attributes
            .as_ref()
            .and_then(|a| a.created_at.as_deref())
            .unwrap_or("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_oauth_client() -> OAuthClient {
        OAuthClient {
            id: "oc-123".to_string(),
            client_type: Some("oauth-clients".to_string()),
            attributes: OAuthClientAttributes {
                created_at: Some("2025-01-01T00:00:00Z".to_string()),
                service_provider: Some("github".to_string()),
                service_provider_display_name: Some("GitHub".to_string()),
                name: Some("My GitHub Connection".to_string()),
                http_url: Some("https://github.com".to_string()),
                api_url: Some("https://api.github.com".to_string()),
                callback_url: Some("https://app.terraform.io/callback".to_string()),
                organization_scoped: Some(true),
            },
            relationships: Some(OAuthClientRelationships {
                organization: Some(RelationshipData {
                    data: Some(RelationshipId {
                        id: "my-org".to_string(),
                        rel_type: Some("organizations".to_string()),
                    }),
                }),
                oauth_tokens: Some(OAuthTokensRelationship {
                    data: Some(vec![
                        RelationshipId {
                            id: "ot-abc".to_string(),
                            rel_type: Some("oauth-tokens".to_string()),
                        },
                        RelationshipId {
                            id: "ot-def".to_string(),
                            rel_type: Some("oauth-tokens".to_string()),
                        },
                    ]),
                }),
            }),
        }
    }

    #[test]
    fn test_oauth_client_name() {
        let client = create_test_oauth_client();
        assert_eq!(client.name(), "My GitHub Connection");
    }

    #[test]
    fn test_oauth_client_name_fallback_to_display_name() {
        let mut client = create_test_oauth_client();
        client.attributes.name = None;
        assert_eq!(client.name(), "GitHub");
    }

    #[test]
    fn test_oauth_client_service_provider() {
        let client = create_test_oauth_client();
        assert_eq!(client.service_provider(), "github");
        assert_eq!(client.service_provider_display_name(), "GitHub");
    }

    #[test]
    fn test_oauth_client_http_url() {
        let client = create_test_oauth_client();
        assert_eq!(client.http_url(), "https://github.com");
    }

    #[test]
    fn test_oauth_client_created_at() {
        let client = create_test_oauth_client();
        assert_eq!(client.created_at(), "2025-01-01T00:00:00Z");
    }

    #[test]
    fn test_oauth_client_is_organization_scoped() {
        let client = create_test_oauth_client();
        assert!(client.is_organization_scoped());
    }

    #[test]
    fn test_oauth_client_oauth_token_ids() {
        let client = create_test_oauth_client();
        let token_ids = client.oauth_token_ids();
        assert_eq!(token_ids.len(), 2);
        assert_eq!(token_ids[0], "ot-abc");
        assert_eq!(token_ids[1], "ot-def");
    }

    #[test]
    fn test_oauth_client_organization_id() {
        let client = create_test_oauth_client();
        assert_eq!(client.organization_id(), Some("my-org"));
    }

    #[test]
    fn test_oauth_client_defaults() {
        let client = OAuthClient {
            id: "oc-123".to_string(),
            client_type: None,
            attributes: OAuthClientAttributes {
                created_at: None,
                service_provider: None,
                service_provider_display_name: None,
                name: None,
                http_url: None,
                api_url: None,
                callback_url: None,
                organization_scoped: None,
            },
            relationships: None,
        };
        assert_eq!(client.name(), "oc-123");
        assert_eq!(client.service_provider(), "unknown");
        assert_eq!(client.http_url(), "");
        assert_eq!(client.created_at(), "");
        assert!(client.is_organization_scoped());
        assert!(client.oauth_token_ids().is_empty());
        assert_eq!(client.organization_id(), None);
    }
}
