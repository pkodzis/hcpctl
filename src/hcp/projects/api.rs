//! Project API operations

use log::debug;
use std::collections::HashMap;

use crate::config::api;
use crate::error::{Result, TfeError};
use crate::hcp::traits::TfeResource;
use crate::hcp::workspaces::WorkspaceQuery;
use crate::hcp::TfeClient;

use super::models::{Project, ProjectsResponse};

impl TfeClient {
    /// Get all projects for an organization (with pagination and optional server-side search)
    ///
    /// When `search` is provided, uses API's `q=` parameter for case-insensitive server-side filtering.
    /// This is more efficient than fetching all projects and filtering locally.
    pub async fn get_projects(&self, org: &str, search: Option<&str>) -> Result<Vec<Project>> {
        // Build path with optional query param
        let mut path = format!("/{}/{}/{}", api::ORGANIZATIONS, org, api::PROJECTS);

        if let Some(s) = search {
            path.push_str(&format!("?q={}", urlencoding::encode(s)));
        }

        let error_context = format!("projects for organization '{}' (search: {:?})", org, search);

        self.fetch_all_pages::<Project, ProjectsResponse>(&path, &error_context)
            .await
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
        let projects = self.get_projects(org, None).await?;

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
        let workspaces = self.get_workspaces(org, WorkspaceQuery::default()).await?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn create_test_client(base_url: &str) -> TfeClient {
        TfeClient::with_base_url(
            "test-token".to_string(),
            "mock.terraform.io".to_string(),
            base_url.to_string(),
        )
    }

    fn project_json(id: &str, name: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "type": "projects",
            "attributes": {
                "name": name
            }
        })
    }

    #[tokio::test]
    async fn test_get_projects_success() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(&mock_server.uri());

        let response_body = serde_json::json!({
            "data": [
                project_json("prj-1", "project-1"),
                project_json("prj-2", "project-2")
            ]
        });

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/projects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&mock_server)
            .await;

        let result = client.get_projects("my-org", None).await;

        assert!(result.is_ok());
        let projects = result.unwrap();
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].name(), "project-1");
        assert_eq!(projects[1].name(), "project-2");
    }

    #[tokio::test]
    async fn test_get_projects_with_search() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(&mock_server.uri());

        let response_body = serde_json::json!({
            "data": [project_json("prj-prod", "production")]
        });

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/projects"))
            .and(query_param("q", "prod"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&mock_server)
            .await;

        let result = client.get_projects("my-org", Some("prod")).await;

        assert!(result.is_ok());
        let projects = result.unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name(), "production");
    }

    #[tokio::test]
    async fn test_get_projects_empty() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(&mock_server.uri());

        let response_body = serde_json::json!({ "data": [] });

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/projects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&mock_server)
            .await;

        let result = client.get_projects("my-org", None).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_get_projects_api_error() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/projects"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let result = client.get_projects("my-org", None).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            TfeError::Api { status, .. } => assert_eq!(status, 500),
            _ => panic!("Expected TfeError::Api"),
        }
    }

    #[tokio::test]
    async fn test_get_project_by_id_success() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(&mock_server.uri());

        let response_body = serde_json::json!({
            "data": project_json("prj-abc123", "my-project")
        });

        Mock::given(method("GET"))
            .and(path("/projects/prj-abc123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&mock_server)
            .await;

        let result = client.get_project_by_id("prj-abc123").await;

        assert!(result.is_ok());
        let (project, _raw) = result.unwrap().unwrap();
        assert_eq!(project.id, "prj-abc123");
        assert_eq!(project.name(), "my-project");
    }

    #[tokio::test]
    async fn test_get_project_by_id_not_found() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/projects/prj-nonexistent"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let result = client.get_project_by_id("prj-nonexistent").await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
}
