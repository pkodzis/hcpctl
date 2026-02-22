//! OAuth Client API operations

use log::debug;

use crate::config::api;
use crate::error::{Result, TfeError};
use crate::hcp::TfeClient;

use super::models::OAuthClient;
use crate::hcp::traits::ApiListResponse;

impl TfeClient {
    /// Get all OAuth clients for an organization (with pagination)
    pub async fn get_oauth_clients(&self, org: &str) -> Result<Vec<OAuthClient>> {
        let path = format!("/{}/{}/oauth-clients", api::ORGANIZATIONS, org);
        let error_context = format!("OAuth clients for organization '{}'", org);

        self.fetch_all_pages::<OAuthClient, ApiListResponse<OAuthClient>>(&path, &error_context)
            .await
    }

    /// Get a single OAuth client by ID
    /// Returns both the typed model and raw JSON for flexible output
    pub async fn get_oauth_client(
        &self,
        client_id: &str,
    ) -> Result<(OAuthClient, serde_json::Value)> {
        let path = format!("/oauth-clients/{}", client_id);
        self.fetch_resource_by_path::<OAuthClient>(&path, &format!("OAuth client '{}'", client_id))
            .await?
            .ok_or_else(|| TfeError::Api {
                status: 404,
                message: format!("OAuth client '{}' not found", client_id),
            })
    }

    /// Get OAuth tokens for an organization (from the oauth-tokens link)
    pub async fn get_oauth_tokens_for_org(
        &self,
        org: &str,
    ) -> Result<Vec<super::models::OAuthToken>> {
        let url = format!("{}/organizations/{}/oauth-tokens", self.base_url(), org);

        debug!("Fetching OAuth tokens from: {}", url);

        let response = self.get(&url).send().await?;

        if !response.status().is_success() {
            // Return empty vec instead of error for orgs without oauth tokens
            debug!(
                "No OAuth tokens found for org '{}' (status: {})",
                org,
                response.status()
            );
            return Ok(vec![]);
        }

        let tokens_response: super::models::OAuthTokensResponse = response.json().await?;
        debug!(
            "Fetched {} OAuth tokens for org '{}'",
            tokens_response.data.len(),
            org
        );
        Ok(tokens_response.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hcp::traits::TfeResource;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn oauth_client_json(id: &str, name: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "type": "oauth-clients",
            "attributes": {
                "name": name,
                "service-provider": "github",
                "http-url": "https://github.com"
            }
        })
    }

    #[tokio::test]
    async fn test_get_oauth_clients_success() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        let response_body = serde_json::json!({
            "data": [
                oauth_client_json("oc-1", "GitHub Prod"),
                oauth_client_json("oc-2", "GitLab Dev")
            ]
        });

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/oauth-clients"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&mock_server)
            .await;

        let result = client.get_oauth_clients("my-org").await;

        assert!(result.is_ok());
        let clients = result.unwrap();
        assert_eq!(clients.len(), 2);
        assert_eq!(clients[0].name(), "GitHub Prod");
    }

    #[tokio::test]
    async fn test_get_oauth_clients_empty() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        let response_body = serde_json::json!({ "data": [] });

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/oauth-clients"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&mock_server)
            .await;

        let result = client.get_oauth_clients("my-org").await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_get_oauth_clients_api_error() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/oauth-clients"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&mock_server)
            .await;

        let result = client.get_oauth_clients("my-org").await;

        assert!(result.is_err());
        match result.unwrap_err() {
            TfeError::Api { status, .. } => assert_eq!(status, 403),
            _ => panic!("Expected TfeError::Api"),
        }
    }

    #[tokio::test]
    async fn test_get_oauth_client_success() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        let response_body = serde_json::json!({
            "data": oauth_client_json("oc-abc123", "My GitHub")
        });

        Mock::given(method("GET"))
            .and(path("/oauth-clients/oc-abc123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&mock_server)
            .await;

        let result = client.get_oauth_client("oc-abc123").await;

        assert!(result.is_ok());
        let (oauth_client, _raw) = result.unwrap();
        assert_eq!(oauth_client.id, "oc-abc123");
        assert_eq!(oauth_client.name(), "My GitHub");
    }

    #[tokio::test]
    async fn test_get_oauth_tokens_for_org_success() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        let response_body = serde_json::json!({
            "data": [
                {
                    "id": "ot-1",
                    "type": "oauth-tokens",
                    "attributes": {}
                }
            ]
        });

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/oauth-tokens"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&mock_server)
            .await;

        let result = client.get_oauth_tokens_for_org("my-org").await;

        assert!(result.is_ok());
        let tokens = result.unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].id, "ot-1");
    }

    #[tokio::test]
    async fn test_get_oauth_tokens_not_found_returns_empty() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/oauth-tokens"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let result = client.get_oauth_tokens_for_org("my-org").await;

        // Should return empty vec, not error
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
