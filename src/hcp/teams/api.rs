//! Team API operations

use log::debug;

use crate::config::api;
use crate::error::{Result, TfeError};
use crate::hcp::TfeClient;

use super::models::Team;
use crate::hcp::traits::ApiListResponse;

impl TfeClient {
    /// Get all teams for an organization (with pagination)
    pub async fn get_teams(&self, org: &str) -> Result<Vec<Team>> {
        let path = format!("/{}/{}/{}", api::ORGANIZATIONS, org, api::TEAMS);
        let error_context = format!("teams for organization '{}'", org);

        self.fetch_all_pages::<Team, ApiListResponse<Team>>(&path, &error_context)
            .await
    }

    /// Get a team by ID
    pub async fn get_team(&self, team_id: &str) -> Result<Option<(Team, serde_json::Value)>> {
        let path = format!("/{}/{}", api::TEAMS, team_id);
        self.fetch_resource_by_path::<Team>(&path, &format!("team '{}'", team_id))
            .await
    }

    /// Get a team by name within an organization
    pub async fn get_team_by_name(
        &self,
        org: &str,
        name: &str,
    ) -> Result<Option<(Team, serde_json::Value)>> {
        // Use filter[names] for exact match
        let url = format!(
            "{}/{}/{}/{}?filter[names]={}",
            self.base_url(),
            api::ORGANIZATIONS,
            org,
            api::TEAMS,
            urlencoding::encode(name)
        );
        debug!("Fetching team by name: {}", url);

        let response = self.get(&url).send().await?;

        match response.status().as_u16() {
            200 => {
                let raw: serde_json::Value = response.json().await?;
                let teams_response: ApiListResponse<Team> = serde_json::from_value(raw.clone())
                    .map_err(|e| TfeError::Api {
                        status: 200,
                        message: format!("Failed to parse teams: {}", e),
                    })?;

                // Find exact match (API might return partial matches)
                if let Some(team) = teams_response
                    .data
                    .into_iter()
                    .find(|t| t.name().eq_ignore_ascii_case(name))
                {
                    // Wrap in proper response format
                    let team_raw = serde_json::json!({
                        "data": team
                    });
                    Ok(Some((team.clone(), team_raw)))
                } else {
                    Ok(None)
                }
            }
            404 => Err(TfeError::Api {
                status: 404,
                message: format!("Organization '{}' not found", org),
            }),
            status => Err(TfeError::Api {
                status,
                message: format!("Failed to fetch team '{}' in organization '{}'", name, org),
            }),
        }
    }

    /// Find team ID by name (convenience method for invite flow)
    pub async fn resolve_team_id(&self, org: &str, name: &str) -> Result<Option<String>> {
        if name.starts_with("team-") {
            // Already an ID
            Ok(Some(name.to_string()))
        } else {
            // Look up by name
            match self.get_team_by_name(org, name).await? {
                Some((team, _)) => Ok(Some(team.id)),
                None => Ok(None),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_get_teams() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/teams"))
            .and(query_param("page[number]", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {
                        "id": "team-abc",
                        "type": "teams",
                        "attributes": {
                            "name": "owners",
                            "users-count": 2,
                            "visibility": "organization"
                        }
                    },
                    {
                        "id": "team-def",
                        "type": "teams",
                        "attributes": {
                            "name": "developers",
                            "users-count": 5,
                            "visibility": "secret"
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
        let teams = client.get_teams("my-org").await.unwrap();

        assert_eq!(teams.len(), 2);
        assert_eq!(teams[0].id, "team-abc");
        assert_eq!(teams[0].name(), "owners");
        assert_eq!(teams[1].id, "team-def");
        assert_eq!(teams[1].name(), "developers");
    }

    #[tokio::test]
    async fn test_get_teams_org_not_found() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/organizations/unknown-org/teams"))
            .and(query_param("page[number]", "1"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let client = TfeClient::test_client(&mock_server.uri());
        let result = client.get_teams("unknown-org").await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Failed to fetch"));
    }

    #[tokio::test]
    async fn test_get_teams_pagination() {
        let mock_server = MockServer::start().await;

        // Page 1
        Mock::given(method("GET"))
            .and(path("/organizations/my-org/teams"))
            .and(query_param("page[number]", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {
                        "id": "team-1",
                        "type": "teams",
                        "attributes": {
                            "name": "team-one",
                            "users-count": 1,
                            "visibility": "organization"
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
            .and(path("/organizations/my-org/teams"))
            .and(query_param("page[number]", "2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {
                        "id": "team-2",
                        "type": "teams",
                        "attributes": {
                            "name": "team-two",
                            "users-count": 2,
                            "visibility": "secret"
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
        let teams = client.get_teams("my-org").await.unwrap();

        assert_eq!(teams.len(), 2);
        assert_eq!(teams[0].id, "team-1");
        assert_eq!(teams[0].name(), "team-one");
        assert_eq!(teams[1].id, "team-2");
        assert_eq!(teams[1].name(), "team-two");
    }

    #[tokio::test]
    async fn test_get_team_by_id() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/teams/team-abc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "id": "team-abc",
                    "type": "teams",
                    "attributes": {
                        "name": "owners",
                        "users-count": 3,
                        "visibility": "organization"
                    }
                }
            })))
            .mount(&mock_server)
            .await;

        let client = TfeClient::test_client(&mock_server.uri());
        let result = client.get_team("team-abc").await.unwrap();

        assert!(result.is_some());
        let (team, _) = result.unwrap();
        assert_eq!(team.id, "team-abc");
        assert_eq!(team.name(), "owners");
    }

    #[tokio::test]
    async fn test_get_team_by_id_not_found() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/teams/team-unknown"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let client = TfeClient::test_client(&mock_server.uri());
        let result = client.get_team("team-unknown").await.unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_team_by_name() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/teams"))
            .and(query_param("filter[names]", "owners"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {
                        "id": "team-abc",
                        "type": "teams",
                        "attributes": {
                            "name": "owners",
                            "users-count": 3
                        }
                    }
                ]
            })))
            .mount(&mock_server)
            .await;

        let client = TfeClient::test_client(&mock_server.uri());
        let result = client.get_team_by_name("my-org", "owners").await.unwrap();

        assert!(result.is_some());
        let (team, _) = result.unwrap();
        assert_eq!(team.id, "team-abc");
        assert_eq!(team.name(), "owners");
    }

    #[tokio::test]
    async fn test_get_team_by_name_not_found() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/teams"))
            .and(query_param("filter[names]", "unknown"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": []
            })))
            .mount(&mock_server)
            .await;

        let client = TfeClient::test_client(&mock_server.uri());
        let result = client.get_team_by_name("my-org", "unknown").await.unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_resolve_team_id_with_id() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        // When given an ID, should return it directly without API call
        let result = client.resolve_team_id("my-org", "team-abc").await.unwrap();
        assert_eq!(result, Some("team-abc".to_string()));
    }

    #[tokio::test]
    async fn test_resolve_team_id_with_name() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/teams"))
            .and(query_param("filter[names]", "owners"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {
                        "id": "team-xyz",
                        "type": "teams",
                        "attributes": {
                            "name": "owners"
                        }
                    }
                ]
            })))
            .mount(&mock_server)
            .await;

        let client = TfeClient::test_client(&mock_server.uri());
        let result = client.resolve_team_id("my-org", "owners").await.unwrap();

        assert_eq!(result, Some("team-xyz".to_string()));
    }
}
