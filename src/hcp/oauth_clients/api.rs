//! OAuth Client API operations

use log::debug;

use crate::config::api;
use crate::error::{Result, TfeError};
use crate::hcp::TfeClient;

use super::models::{OAuthClient, OAuthClientsResponse};

impl TfeClient {
    /// Get all OAuth clients for an organization (with pagination)
    pub async fn get_oauth_clients(&self, org: &str) -> Result<Vec<OAuthClient>> {
        let mut all_clients = Vec::new();
        let mut page = 1;

        loop {
            let url = format!(
                "{}/{}/{}/oauth-clients?page[size]={}&page[number]={}",
                self.base_url(),
                api::ORGANIZATIONS,
                org,
                api::DEFAULT_PAGE_SIZE,
                page
            );

            debug!("Fetching oauth clients page {} from: {}", page, url);

            let response = self.get(&url).send().await?;

            if !response.status().is_success() {
                return Err(TfeError::Api {
                    status: response.status().as_u16(),
                    message: format!("Failed to fetch OAuth clients for org '{}'", org),
                });
            }

            let clients_response: OAuthClientsResponse = response.json().await?;
            let client_count = clients_response.data.len();
            all_clients.extend(clients_response.data);

            // Check if there are more pages
            if let Some(meta) = clients_response.meta {
                if let Some(pagination) = meta.pagination {
                    debug!(
                        "Page {}/{}, total OAuth clients: {}",
                        pagination.current_page, pagination.total_pages, pagination.total_count
                    );

                    if page >= pagination.total_pages {
                        break;
                    }
                    page += 1;
                } else {
                    break;
                }
            } else {
                // No pagination info means single page
                break;
            }

            // Safety check: if no clients returned, stop
            if client_count == 0 {
                break;
            }
        }

        debug!(
            "Fetched {} total OAuth clients for org '{}'",
            all_clients.len(),
            org
        );
        Ok(all_clients)
    }

    /// Get a single OAuth client by ID
    pub async fn get_oauth_client(&self, client_id: &str) -> Result<OAuthClient> {
        let url = format!("{}/oauth-clients/{}", self.base_url(), client_id);

        debug!("Fetching OAuth client from: {}", url);

        let response = self.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(TfeError::Api {
                status: response.status().as_u16(),
                message: format!("Failed to fetch OAuth client '{}'", client_id),
            });
        }

        let client_response: super::models::OAuthClientResponse = response.json().await?;
        Ok(client_response.data)
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
