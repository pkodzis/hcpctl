//! Workspace API operations

use log::debug;

use crate::config::api;
use crate::error::{Result, TfeError};
use crate::hcp::TfeClient;

use super::models::{Workspace, WorkspacesResponse};

impl TfeClient {
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

    /// Get workspaces filtered by project (and optionally by name)
    pub async fn get_workspaces_by_project(
        &self,
        org: &str,
        project_id: &str,
        filter: Option<&str>,
    ) -> Result<Vec<Workspace>> {
        let workspaces = self.get_workspaces(org).await?;

        let filtered: Vec<Workspace> = workspaces
            .into_iter()
            .filter(|ws| ws.project_id() == Some(project_id))
            .filter(|ws| match filter {
                Some(f) => ws.matches_filter(f),
                None => true,
            })
            .collect();

        debug!(
            "After filtering by project '{}': {} workspaces",
            project_id,
            filtered.len()
        );
        Ok(filtered)
    }

    /// Get a single workspace by ID (direct API call, no org needed)
    pub async fn get_workspace_by_id(&self, workspace_id: &str) -> Result<Option<Workspace>> {
        use super::models::WorkspaceResponse;

        let url = format!("{}/{}/{}", self.base_url(), api::WORKSPACES, workspace_id);
        debug!("Fetching workspace directly by ID: {}", url);

        let response = self.get(&url).send().await?;

        match response.status().as_u16() {
            200 => {
                let ws_response: WorkspaceResponse = response.json().await?;
                Ok(Some(ws_response.data))
            }
            404 => Ok(None),
            status => Err(TfeError::Api {
                status,
                message: format!("Failed to fetch workspace '{}'", workspace_id),
            }),
        }
    }

    /// Get a single workspace by name (requires org)
    pub async fn get_workspace_by_name(&self, org: &str, name: &str) -> Result<Option<Workspace>> {
        use super::models::WorkspaceResponse;

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
                let ws_response: WorkspaceResponse = response.json().await?;
                Ok(Some(ws_response.data))
            }
            404 => Ok(None),
            status => Err(TfeError::Api {
                status,
                message: format!("Failed to fetch workspace '{}'", name),
            }),
        }
    }
}
