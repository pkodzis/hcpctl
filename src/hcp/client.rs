//! TFE HTTP client for API interactions

use log::debug;
use reqwest::Client;

use super::models::{OrganizationsResponse, Workspace, WorkspacesResponse};
use crate::config::api;
use crate::error::{Result, TfeError};

/// TFE API client
pub struct TfeClient {
    client: Client,
    token: String,
    host: String,
}

impl TfeClient {
    /// Create a new TFE client
    pub fn new(token: String, host: String) -> Self {
        Self {
            client: Client::new(),
            token,
            host,
        }
    }

    /// Build the base URL for API requests
    fn base_url(&self) -> String {
        format!(
            "https://{}/{}",
            self.host,
            api::BASE_PATH.trim_start_matches('/')
        )
    }

    /// Create a request builder with standard headers
    fn request(&self, url: &str) -> reqwest::RequestBuilder {
        self.client
            .get(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Content-Type", "application/vnd.api+json")
    }

    /// Get all organizations accessible to the token
    pub async fn get_organizations(&self) -> Result<Vec<String>> {
        let url = format!("{}/{}", self.base_url(), api::ORGANIZATIONS);
        debug!("Fetching organizations from: {}", url);

        let response = self.request(&url).send().await?;

        if !response.status().is_success() {
            return Err(TfeError::Api {
                status: response.status().as_u16(),
                message: format!("Failed to fetch organizations"),
            });
        }

        let orgs_response: OrganizationsResponse = response.json().await?;
        Ok(orgs_response.data.into_iter().map(|org| org.id).collect())
    }

    /// Get all workspaces for an organization (with pagination)
    pub async fn get_workspaces(&self, org: &str) -> Result<Vec<Workspace>> {
        let mut all_workspaces = Vec::new();
        let mut page = 1;

        loop {
            let url = format!(
                "{}/{}/{}/{}?page[size]={}&page[number]={}",
                self.base_url(),
                api::ORGANIZATIONS,
                org,
                api::WORKSPACES,
                api::DEFAULT_PAGE_SIZE,
                page
            );

            debug!("Fetching workspaces page {} from: {}", page, url);

            let response = self.request(&url).send().await?;

            if !response.status().is_success() {
                return Err(TfeError::Api {
                    status: response.status().as_u16(),
                    message: format!("Failed to fetch workspaces for org '{}'", org),
                });
            }

            let ws_response: WorkspacesResponse = response.json().await?;
            let workspace_count = ws_response.data.len();
            all_workspaces.extend(ws_response.data);

            // Check if there are more pages
            if let Some(meta) = ws_response.meta {
                if let Some(pagination) = meta.pagination {
                    debug!(
                        "Page {}/{}, total workspaces: {}",
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

            // Safety check: if no workspaces returned, stop
            if workspace_count == 0 {
                break;
            }
        }

        debug!(
            "Fetched {} total workspaces for org '{}'",
            all_workspaces.len(),
            org
        );
        Ok(all_workspaces)
    }

    /// Get workspaces with optional filter
    pub async fn get_workspaces_filtered(
        &self,
        org: &str,
        filter: Option<&str>,
    ) -> Result<Vec<Workspace>> {
        let workspaces = self.get_workspaces(org).await?;

        let filtered = match filter {
            Some(f) => workspaces
                .into_iter()
                .filter(|ws| ws.matches_filter(f))
                .collect(),
            None => workspaces,
        };

        debug!(
            "After filtering: {} workspaces for org '{}'",
            filtered.len(),
            org
        );
        Ok(filtered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_url() {
        let client = TfeClient::new("token".to_string(), "example.com".to_string());
        assert_eq!(client.base_url(), "https://example.com/api/v2");
    }

    #[test]
    fn test_client_creation() {
        let client = TfeClient::new("my-token".to_string(), "tfe.example.com".to_string());
        assert_eq!(client.host, "tfe.example.com");
        assert_eq!(client.token, "my-token");
    }
}
