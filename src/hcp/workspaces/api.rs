//! Workspace API operations

use log::debug;

use crate::config::api;
use crate::error::{Result, TfeError};
use crate::hcp::TfeClient;

use super::models::{Workspace, WorkspaceQuery, WorkspacesResponse};

impl TfeClient {
    /// Get workspaces for an organization with optional filters
    ///
    /// Uses API query parameters for efficient server-side filtering:
    /// - `search[name]` for fuzzy name search
    /// - `filter[project][id]` for project filtering
    pub async fn get_workspaces(
        &self,
        org: &str,
        query: WorkspaceQuery<'_>,
    ) -> Result<Vec<Workspace>> {
        let mut all_workspaces = Vec::new();
        let mut page = 1;

        loop {
            let mut url = format!(
                "{}/{}/{}/{}?page[size]={}&page[number]={}",
                self.base_url(),
                api::ORGANIZATIONS,
                org,
                api::WORKSPACES,
                api::DEFAULT_PAGE_SIZE,
                page
            );

            // Add server-side filters
            if let Some(s) = query.search {
                url.push_str(&format!("&search[name]={}", urlencoding::encode(s)));
            }
            if let Some(prj) = query.project_id {
                url.push_str(&format!(
                    "&filter[project][id]={}",
                    urlencoding::encode(prj)
                ));
            }

            debug!("Fetching workspaces page {} from: {}", page, url);

            let response = self.get(&url).send().await?;

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
                break;
            }

            if workspace_count == 0 {
                break;
            }
        }

        debug!(
            "Fetched {} workspaces for org '{}' (search: {:?}, project: {:?})",
            all_workspaces.len(),
            org,
            query.search,
            query.project_id
        );
        Ok(all_workspaces)
    }

    /// Get a single workspace by ID (direct API call, no org needed)
    /// Returns both the typed model and raw JSON for flexible output
    pub async fn get_workspace_by_id(
        &self,
        workspace_id: &str,
    ) -> Result<Option<(Workspace, serde_json::Value)>> {
        let url = format!("{}/{}/{}", self.base_url(), api::WORKSPACES, workspace_id);
        debug!("Fetching workspace directly by ID: {}", url);

        let response = self.get(&url).send().await?;

        match response.status().as_u16() {
            200 => {
                // First get raw JSON
                let raw: serde_json::Value = response.json().await?;
                // Then deserialize model from the same data
                let workspace: Workspace =
                    serde_json::from_value(raw["data"].clone()).map_err(|e| TfeError::Api {
                        status: 200,
                        message: format!("Failed to parse workspace: {}", e),
                    })?;
                Ok(Some((workspace, raw)))
            }
            404 => Ok(None),
            status => Err(TfeError::Api {
                status,
                message: format!("Failed to fetch workspace '{}'", workspace_id),
            }),
        }
    }

    /// Get a single workspace by name (requires org)
    /// Returns both the typed model and raw JSON for flexible output
    pub async fn get_workspace_by_name(
        &self,
        org: &str,
        name: &str,
    ) -> Result<Option<(Workspace, serde_json::Value)>> {
        let url = format!(
            "{}/{}/{}/{}/{}",
            self.base_url(),
            api::ORGANIZATIONS,
            org,
            api::WORKSPACES,
            name
        );

        debug!("Fetching workspace by name: {}", url);

        let response = self.get(&url).send().await?;

        match response.status().as_u16() {
            200 => {
                // First get raw JSON
                let raw: serde_json::Value = response.json().await?;
                // Then deserialize model from the same data
                let workspace: Workspace =
                    serde_json::from_value(raw["data"].clone()).map_err(|e| TfeError::Api {
                        status: 200,
                        message: format!("Failed to parse workspace: {}", e),
                    })?;
                Ok(Some((workspace, raw)))
            }
            404 => Ok(None),
            status => Err(TfeError::Api {
                status,
                message: format!("Failed to fetch workspace '{}'", name),
            }),
        }
    }

    /// Fetch a subresource by its API URL
    /// Used to fetch related resources like current-run, current-state-version, etc.
    pub async fn get_subresource(&self, url: &str) -> Result<serde_json::Value> {
        let full_url = format!("https://{}{}", self.host(), url);
        debug!("Fetching subresource: {}", full_url);

        let response = self.get(&full_url).send().await?;

        match response.status().as_u16() {
            200 => {
                let raw: serde_json::Value = response.json().await?;
                Ok(raw)
            }
            404 => Err(TfeError::Api {
                status: 404,
                message: format!("Subresource not found at '{}'", url),
            }),
            status => Err(TfeError::Api {
                status,
                message: format!("Failed to fetch subresource from '{}'", url),
            }),
        }
    }
}
