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

    /// Get a single organization by name (direct API call)
    /// Returns both the typed model and raw JSON for flexible output
    pub async fn get_organization(
        &self,
        name: &str,
    ) -> Result<Option<(Organization, serde_json::Value)>> {
        let url = format!("{}/{}/{}", self.base_url(), api::ORGANIZATIONS, name);
        debug!("Fetching organization directly: {}", url);

        let response = self.get(&url).send().await?;

        match response.status().as_u16() {
            200 => {
                // First get raw JSON
                let raw: serde_json::Value = response.json().await?;
                // Then deserialize model from the same data
                let org: Organization =
                    serde_json::from_value(raw["data"].clone()).map_err(|e| TfeError::Api {
                        status: 200,
                        message: format!("Failed to parse organization: {}", e),
                    })?;
                Ok(Some((org, raw)))
            }
            404 => Ok(None),
            status => Err(TfeError::Api {
                status,
                message: format!("Failed to fetch organization '{}'", name),
            }),
        }
    }
}
