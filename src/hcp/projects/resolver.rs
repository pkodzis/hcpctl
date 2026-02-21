//! Project resolution utilities
//!
//! Provides shared functionality for resolving projects by ID or name.

use log::debug;

use super::models::Project;
use crate::hcp::TfeClient;
use crate::ui::{create_spinner, finish_spinner};

/// Resolved project information
#[derive(Debug)]
pub struct ResolvedProject {
    /// The project model
    pub project: Project,
    /// Raw JSON response
    pub raw: serde_json::Value,
}

/// Resolve a project target (ID or name) to a project
///
/// # Arguments
/// * `client` - TFE API client
/// * `target` - Project ID (prj-xxx) or name
/// * `org` - Organization name (required for name resolution)
/// * `batch` - If true, no spinners
pub async fn resolve_project(
    client: &TfeClient,
    target: &str,
    org: &str,
    batch: bool,
) -> Result<ResolvedProject, Box<dyn std::error::Error>> {
    let spinner = create_spinner("Resolving project...", batch);

    let result = if target.starts_with("prj-") {
        debug!("Resolving project by ID: {}", target);
        client.get_project_by_id(target).await?
    } else {
        debug!("Resolving project by name '{}' in org '{}'", target, org);
        client.get_project_by_name(org, target).await?
    };

    finish_spinner(spinner);

    match result {
        Some((project, raw)) => Ok(ResolvedProject { project, raw }),
        None => Err(format!("Project '{}' not found in organization '{}'", target, org).into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hcp::traits::TfeResource;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn project_response(prj_id: &str, prj_name: &str) -> serde_json::Value {
        serde_json::json!({
            "data": {
                "id": prj_id,
                "type": "projects",
                "attributes": {
                    "name": prj_name,
                    "description": null
                }
            }
        })
    }

    fn projects_list_response(projects: Vec<(&str, &str)>) -> serde_json::Value {
        let data: Vec<serde_json::Value> = projects
            .iter()
            .map(|(id, name)| {
                serde_json::json!({
                    "id": id,
                    "type": "projects",
                    "attributes": {
                        "name": name,
                        "description": null
                    }
                })
            })
            .collect();

        serde_json::json!({
            "data": data,
            "meta": {
                "pagination": {
                    "current-page": 1,
                    "total-pages": 1,
                    "total-count": data.len()
                }
            }
        })
    }

    #[tokio::test]
    async fn test_resolve_project_by_id() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/projects/prj-abc123"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(project_response("prj-abc123", "my-project")),
            )
            .mount(&mock_server)
            .await;

        let result = resolve_project(&client, "prj-abc123", "my-org", true).await;

        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert_eq!(resolved.project.id, "prj-abc123");
        assert_eq!(resolved.project.name(), "my-project");
    }

    #[tokio::test]
    async fn test_resolve_project_by_name() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        // First call: list projects to find by name
        Mock::given(method("GET"))
            .and(path("/organizations/my-org/projects"))
            .and(query_param("page[number]", "1"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(projects_list_response(vec![
                    ("prj-abc123", "my-project"),
                    ("prj-def456", "other-project"),
                ])),
            )
            .mount(&mock_server)
            .await;

        // Second call: get_project_by_id for full details
        Mock::given(method("GET"))
            .and(path("/projects/prj-abc123"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(project_response("prj-abc123", "my-project")),
            )
            .mount(&mock_server)
            .await;

        let result = resolve_project(&client, "my-project", "my-org", true).await;

        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert_eq!(resolved.project.id, "prj-abc123");
        assert_eq!(resolved.project.name(), "my-project");
    }

    #[tokio::test]
    async fn test_resolve_project_by_id_not_found() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/projects/prj-notfound"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let result = resolve_project(&client, "prj-notfound", "my-org", true).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_resolve_project_by_name_not_found() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/projects"))
            .and(query_param("page[number]", "1"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(projects_list_response(vec![(
                    "prj-abc123",
                    "other-project",
                )])),
            )
            .mount(&mock_server)
            .await;

        let result = resolve_project(&client, "nonexistent", "my-org", true).await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("nonexistent"));
        assert!(err.contains("my-org"));
    }
}
