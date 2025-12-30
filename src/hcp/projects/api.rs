//! Project API operations

use log::debug;
use std::collections::HashMap;

use crate::config::api;
use crate::error::{Result, TfeError};
use crate::hcp::traits::TfeResource;
use crate::hcp::TfeClient;

use super::models::{Project, ProjectsResponse};

impl TfeClient {
    /// Get all projects for an organization (with pagination)
    pub async fn get_projects(&self, org: &str) -> Result<Vec<Project>> {
        let mut all_projects = Vec::new();
        let mut page = 1;

        loop {
            let url = format!(
                "{}/{}/{}/{}?page[size]={}&page[number]={}",
                self.base_url(),
                api::ORGANIZATIONS,
                org,
                api::PROJECTS,
                api::DEFAULT_PAGE_SIZE,
                page
            );

            debug!("Fetching projects page {} from: {}", page, url);

            let response = self.get(&url).send().await?;

            if !response.status().is_success() {
                return Err(TfeError::Api {
                    status: response.status().as_u16(),
                    message: format!("Failed to fetch projects for org '{}'", org),
                });
            }

            let prj_response: ProjectsResponse = response.json().await?;
            let project_count = prj_response.data.len();
            all_projects.extend(prj_response.data);

            // Check if there are more pages
            if let Some(meta) = prj_response.meta {
                if let Some(pagination) = meta.pagination {
                    debug!(
                        "Page {}/{}, total projects: {}",
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

            if project_count == 0 {
                break;
            }
        }

        debug!(
            "Fetched {} total projects for org '{}'",
            all_projects.len(),
            org
        );
        Ok(all_projects)
    }

    /// Get a single project by ID (direct API call, no org needed)
    /// Returns both the typed model and raw JSON for flexible output
    pub async fn get_project_by_id(
        &self,
        project_id: &str,
    ) -> Result<Option<(Project, serde_json::Value)>> {
        let url = format!("{}/{}/{}", self.base_url(), api::PROJECTS, project_id);
        debug!("Fetching project directly by ID: {}", url);

        let response = self.get(&url).send().await?;

        match response.status().as_u16() {
            200 => {
                // First get raw JSON
                let raw: serde_json::Value = response.json().await?;
                // Then deserialize model from the same data
                let project: Project =
                    serde_json::from_value(raw["data"].clone()).map_err(|e| TfeError::Api {
                        status: 200,
                        message: format!("Failed to parse project: {}", e),
                    })?;
                Ok(Some((project, raw)))
            }
            404 => Ok(None),
            status => Err(TfeError::Api {
                status,
                message: format!("Failed to fetch project '{}'", project_id),
            }),
        }
    }

    /// Get a single project by name (requires org)
    /// Returns both the typed model and raw JSON for flexible output
    pub async fn get_project_by_name(
        &self,
        org: &str,
        name: &str,
    ) -> Result<Option<(Project, serde_json::Value)>> {
        debug!("Fetching project by name: {}", name);
        let projects = self.get_projects(org).await?;

        // Find the project by name
        if let Some(project) = projects.into_iter().find(|p| p.matches(name)) {
            // Now fetch it by ID to get the raw JSON
            self.get_project_by_id(&project.id).await
        } else {
            Ok(None)
        }
    }

    /// Count workspaces per project in an organization
    pub async fn count_workspaces_by_project(&self, org: &str) -> Result<HashMap<String, usize>> {
        let workspaces = self.get_workspaces(org).await?;

        let mut counts: HashMap<String, usize> = HashMap::new();

        for ws in workspaces {
            if let Some(project_id) = ws.project_id() {
                *counts.entry(project_id.to_string()).or_insert(0) += 1;
            }
        }

        debug!(
            "Workspace counts per project for org '{}': {:?}",
            org, counts
        );
        Ok(counts)
    }
}
