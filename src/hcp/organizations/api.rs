//! Organization API operations

use log::debug;

use crate::config::api;
use crate::error::{Result, TfeError};
use crate::hcp::TfeClient;

use super::models::{Organization, OrganizationsResponse};

impl TfeClient {
    /// Get all organizations accessible to the token (names only)
    pub async fn get_organizations(&self) -> Result<Vec<String>> {
        let orgs = self.get_organizations_full().await?;
        Ok(orgs.into_iter().map(|org| org.id).collect())
    }

    /// Get all organizations with full details
    pub async fn get_organizations_full(&self) -> Result<Vec<Organization>> {
        let url = format!("{}/{}", self.base_url(), api::ORGANIZATIONS);
        debug!("Fetching organizations from: {}", url);

        let response = self.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(TfeError::Api {
                status: response.status().as_u16(),
                message: "Failed to fetch organizations".to_string(),
            });
        }

        let orgs_response: OrganizationsResponse = response.json().await?;
        Ok(orgs_response.data)
    }

    /// Get a single organization by name or external ID
    ///
    /// HCP API has inconsistent naming:
    /// - `id` field = organization name (e.g., "my-org")
    /// - `external-id` = actual ID (e.g., "org-ABC123")
    ///
    /// This method accepts either and finds the organization.
    pub async fn get_organization(
        &self,
        name_or_id: &str,
    ) -> Result<Option<(Organization, serde_json::Value)>> {
        // First try direct lookup by name (most common case)
        let url = format!("{}/{}/{}", self.base_url(), api::ORGANIZATIONS, name_or_id);
        debug!("Fetching organization by name: {}", url);

        let response = self.get(&url).send().await?;

        match response.status().as_u16() {
            200 => {
                let raw: serde_json::Value = response.json().await?;
                let org: Organization =
                    serde_json::from_value(raw["data"].clone()).map_err(|e| TfeError::Api {
                        status: 200,
                        message: format!("Failed to parse organization: {}", e),
                    })?;
                Ok(Some((org, raw)))
            }
            404 => {
                // Not found by name - try external ID if it looks like one
                if name_or_id.starts_with("org-") {
                    debug!(
                        "Not found by name, searching by external-id: {}",
                        name_or_id
                    );
                    self.get_organization_by_external_id(name_or_id).await
                } else {
                    Ok(None)
                }
            }
            status => Err(TfeError::Api {
                status,
                message: format!("Failed to fetch organization '{}'", name_or_id),
            }),
        }
    }

    /// Get organization by external ID (searches all orgs)
    async fn get_organization_by_external_id(
        &self,
        external_id: &str,
    ) -> Result<Option<(Organization, serde_json::Value)>> {
        let orgs = self.get_organizations_full().await?;

        // Find org matching external_id
        if let Some(org) = orgs.into_iter().find(|o| o.external_id() == external_id) {
            // Fetch full details by name to get raw JSON
            let url = format!("{}/{}/{}", self.base_url(), api::ORGANIZATIONS, org.id);
            let response = self.get(&url).send().await?;

            if response.status().is_success() {
                let raw: serde_json::Value = response.json().await?;
                let org: Organization =
                    serde_json::from_value(raw["data"].clone()).map_err(|e| TfeError::Api {
                        status: 200,
                        message: format!("Failed to parse organization: {}", e),
                    })?;
                return Ok(Some((org, raw)));
            }
        }

        Ok(None)
    }
}
