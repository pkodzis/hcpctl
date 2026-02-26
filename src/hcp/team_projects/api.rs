//! Team project access API operations

use log::debug;

use crate::config::api;
use crate::error::Result;
use crate::hcp::TfeClient;

use super::models::TeamProjectAccess;
use crate::hcp::traits::ApiListResponse;

impl TfeClient {
    /// Get all team-project access bindings for a project (with pagination)
    pub async fn get_team_project_access(
        &self,
        project_id: &str,
    ) -> Result<Vec<TeamProjectAccess>> {
        let path = format!("/{}?filter[project][id]={}", api::TEAM_PROJECTS, project_id);
        let error_context = format!("team-project access for project '{}'", project_id);

        debug!("Fetching team-project access: {}", path);

        self.fetch_all_pages::<TeamProjectAccess, ApiListResponse<TeamProjectAccess>>(
            &path,
            &error_context,
        )
        .await
    }

    /// Get a single team-project access binding by ID
    pub async fn get_team_project_access_by_id(
        &self,
        tprj_id: &str,
    ) -> Result<Option<(TeamProjectAccess, serde_json::Value)>> {
        let path = format!("/{}/{}", api::TEAM_PROJECTS, tprj_id);
        self.fetch_resource_by_path::<TeamProjectAccess>(
            &path,
            &format!("team-project access '{}'", tprj_id),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_get_team_project_access() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/team-projects"))
            .and(query_param("filter[project][id]", "prj-abc"))
            .and(query_param("page[number]", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {
                        "id": "tprj-1",
                        "type": "team-projects",
                        "attributes": {
                            "access": "admin"
                        },
                        "relationships": {
                            "team": { "data": { "id": "team-a", "type": "teams" } },
                            "project": { "data": { "id": "prj-abc", "type": "projects" } }
                        }
                    },
                    {
                        "id": "tprj-2",
                        "type": "team-projects",
                        "attributes": {
                            "access": "read"
                        },
                        "relationships": {
                            "team": { "data": { "id": "team-b", "type": "teams" } },
                            "project": { "data": { "id": "prj-abc", "type": "projects" } }
                        }
                    }
                ],
                "meta": {
                    "pagination": {
                        "current-page": 1,
                        "page-size": 20,
                        "prev-page": null,
                        "next-page": null,
                        "total-pages": 1,
                        "total-count": 2
                    }
                }
            })))
            .mount(&mock_server)
            .await;

        let client = TfeClient::test_client(&mock_server.uri());
        let bindings = client.get_team_project_access("prj-abc").await.unwrap();

        assert_eq!(bindings.len(), 2);
        assert_eq!(bindings[0].id, "tprj-1");
        assert_eq!(bindings[0].access(), "admin");
        assert_eq!(bindings[0].team_id(), "team-a");
        assert_eq!(bindings[0].project_id(), "prj-abc");
        assert_eq!(bindings[1].id, "tprj-2");
        assert_eq!(bindings[1].access(), "read");
    }

    #[tokio::test]
    async fn test_get_team_project_access_empty() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/team-projects"))
            .and(query_param("filter[project][id]", "prj-empty"))
            .and(query_param("page[number]", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [],
                "meta": {
                    "pagination": {
                        "current-page": 1,
                        "page-size": 20,
                        "prev-page": null,
                        "next-page": null,
                        "total-pages": 1,
                        "total-count": 0
                    }
                }
            })))
            .mount(&mock_server)
            .await;

        let client = TfeClient::test_client(&mock_server.uri());
        let bindings = client.get_team_project_access("prj-empty").await.unwrap();

        assert!(bindings.is_empty());
    }

    #[tokio::test]
    async fn test_get_team_project_access_by_id() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/team-projects/tprj-abc123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "id": "tprj-abc123",
                    "type": "team-projects",
                    "attributes": {
                        "access": "write"
                    },
                    "relationships": {
                        "team": { "data": { "id": "team-x", "type": "teams" } },
                        "project": { "data": { "id": "prj-y", "type": "projects" } }
                    }
                }
            })))
            .mount(&mock_server)
            .await;

        let client = TfeClient::test_client(&mock_server.uri());
        let result = client
            .get_team_project_access_by_id("tprj-abc123")
            .await
            .unwrap();

        assert!(result.is_some());
        let (access, _raw) = result.unwrap();
        assert_eq!(access.id, "tprj-abc123");
        assert_eq!(access.access(), "write");
        assert_eq!(access.team_id(), "team-x");
        assert_eq!(access.project_id(), "prj-y");
    }

    #[tokio::test]
    async fn test_get_team_project_access_by_id_not_found() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/team-projects/tprj-unknown"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let client = TfeClient::test_client(&mock_server.uri());
        let result = client
            .get_team_project_access_by_id("tprj-unknown")
            .await
            .unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_team_project_access_pagination() {
        let mock_server = MockServer::start().await;

        // Page 1
        Mock::given(method("GET"))
            .and(path("/team-projects"))
            .and(query_param("filter[project][id]", "prj-big"))
            .and(query_param("page[number]", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {
                        "id": "tprj-p1",
                        "type": "team-projects",
                        "attributes": { "access": "admin" },
                        "relationships": {
                            "team": { "data": { "id": "team-1", "type": "teams" } },
                            "project": { "data": { "id": "prj-big", "type": "projects" } }
                        }
                    }
                ],
                "meta": {
                    "pagination": {
                        "current-page": 1,
                        "page-size": 1,
                        "prev-page": null,
                        "next-page": 2,
                        "total-pages": 2,
                        "total-count": 2
                    }
                }
            })))
            .mount(&mock_server)
            .await;

        // Page 2
        Mock::given(method("GET"))
            .and(path("/team-projects"))
            .and(query_param("filter[project][id]", "prj-big"))
            .and(query_param("page[number]", "2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {
                        "id": "tprj-p2",
                        "type": "team-projects",
                        "attributes": { "access": "read" },
                        "relationships": {
                            "team": { "data": { "id": "team-2", "type": "teams" } },
                            "project": { "data": { "id": "prj-big", "type": "projects" } }
                        }
                    }
                ],
                "meta": {
                    "pagination": {
                        "current-page": 2,
                        "page-size": 1,
                        "prev-page": 1,
                        "next-page": null,
                        "total-pages": 2,
                        "total-count": 2
                    }
                }
            })))
            .mount(&mock_server)
            .await;

        let client = TfeClient::test_client(&mock_server.uri());
        let bindings = client.get_team_project_access("prj-big").await.unwrap();

        assert_eq!(bindings.len(), 2);
        assert_eq!(bindings[0].id, "tprj-p1");
        assert_eq!(bindings[1].id, "tprj-p2");
    }

    #[tokio::test]
    async fn test_get_team_project_access_server_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/team-projects"))
            .and(query_param("filter[project][id]", "prj-err"))
            .and(query_param("page[number]", "1"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let client = TfeClient::test_client(&mock_server.uri());
        let result = client.get_team_project_access("prj-err").await;

        assert!(result.is_err());
    }
}
