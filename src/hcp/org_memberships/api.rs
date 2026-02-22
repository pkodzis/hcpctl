//! Organization membership API operations

use log::debug;

use crate::config::api;
use crate::error::{Result, TfeError};
use crate::hcp::TfeClient;

use super::models::{InviteUserRequest, OrganizationMembership, OrganizationMembershipResponse};
use crate::hcp::traits::ApiListResponse;

impl TfeClient {
    /// Get all organization memberships for an organization (with pagination)
    pub async fn get_org_memberships(&self, org: &str) -> Result<Vec<OrganizationMembership>> {
        let path = format!("/{}/{}/organization-memberships", api::ORGANIZATIONS, org);
        let error_context = format!("organization memberships for '{}'", org);

        self.fetch_all_pages::<OrganizationMembership, ApiListResponse<OrganizationMembership>>(
            &path,
            &error_context,
        )
        .await
    }

    /// Get organization membership by email (filtered query - efficient)
    pub async fn get_org_membership_by_email(
        &self,
        org: &str,
        email: &str,
    ) -> Result<Option<OrganizationMembership>> {
        let url = format!(
            "{}/{}/{}/organization-memberships?filter[email]={}",
            self.base_url(),
            api::ORGANIZATIONS,
            org,
            urlencoding::encode(email)
        );

        debug!("Looking up membership for {} in {}", email, org);

        let response = self.get(&url).send().await?;

        match response.status().as_u16() {
            200 => {
                let memberships: ApiListResponse<OrganizationMembership> = response.json().await?;
                Ok(memberships.data.into_iter().next())
            }
            404 => Ok(None),
            status => Err(TfeError::Api {
                status,
                message: format!("Failed to lookup membership for '{}' in '{}'", email, org),
            }),
        }
    }

    /// Invite a user to an organization
    ///
    /// # Arguments
    /// * `org` - Organization name
    /// * `email` - Email of user to invite
    /// * `team_ids` - Optional list of team IDs to add the user to
    ///
    /// Returns error if user already has a membership (invited or active)
    pub async fn invite_user(
        &self,
        org: &str,
        email: &str,
        team_ids: Option<Vec<String>>,
    ) -> Result<OrganizationMembership> {
        // Check if user already has a membership (filtered query - efficient)
        if let Some(membership) = self.get_org_membership_by_email(org, email).await? {
            let status = membership.status();
            return Err(TfeError::Api {
                status: 409,
                message: format!(
                    "User '{}' already has membership in '{}' (status: {}, id: {})",
                    email, org, status, membership.id
                ),
            });
        }

        let url = format!(
            "{}/{}/{}/organization-memberships",
            self.base_url(),
            api::ORGANIZATIONS,
            org
        );

        debug!("Inviting user {} to organization {}", email, org);

        let request = match team_ids {
            Some(ids) if !ids.is_empty() => InviteUserRequest::with_teams(email, ids),
            _ => InviteUserRequest::new(email),
        };

        let response = self.post(&url).json(&request).send().await?;

        match response.status().as_u16() {
            200 | 201 => {
                let membership_response: OrganizationMembershipResponse = response.json().await?;
                debug!(
                    "Successfully invited user {} to {} (membership ID: {})",
                    email, org, membership_response.data.id
                );
                Ok(membership_response.data)
            }
            404 => Err(TfeError::Api {
                status: 404,
                message: format!("Organization '{}' not found", org),
            }),
            422 => {
                // Try to parse error message from response
                let error_body: serde_json::Value =
                    response.json().await.unwrap_or(serde_json::json!({}));
                let error_msg = error_body["errors"][0]["detail"]
                    .as_str()
                    .unwrap_or("Validation error");
                Err(TfeError::Api {
                    status: 422,
                    message: format!("Cannot invite '{}': {}", email, error_msg),
                })
            }
            status => {
                let error_body = response.text().await.unwrap_or_default();
                Err(TfeError::Api {
                    status,
                    message: format!(
                        "Failed to invite user '{}' to '{}': {}",
                        email, org, error_body
                    ),
                })
            }
        }
    }

    /// Delete an organization membership (remove user from org)
    pub async fn delete_org_membership(&self, membership_id: &str) -> Result<()> {
        let url = format!(
            "{}/organization-memberships/{}",
            self.base_url(),
            membership_id
        );

        debug!("Deleting organization membership: {}", membership_id);

        let response = self.delete(&url).send().await?;

        match response.status().as_u16() {
            200 | 204 => {
                debug!("Successfully deleted membership {}", membership_id);
                Ok(())
            }
            404 => Err(TfeError::Api {
                status: 404,
                message: format!("Membership '{}' not found", membership_id),
            }),
            status => Err(TfeError::Api {
                status,
                message: format!("Failed to delete membership '{}'", membership_id),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_get_org_memberships() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/organization-memberships"))
            .and(query_param("page[number]", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {
                        "id": "ou-abc",
                        "type": "organization-memberships",
                        "attributes": {
                            "email": "user1@example.com",
                            "status": "active"
                        }
                    },
                    {
                        "id": "ou-def",
                        "type": "organization-memberships",
                        "attributes": {
                            "email": "user2@example.com",
                            "status": "invited"
                        }
                    }
                ],
                "meta": {
                    "pagination": {
                        "current-page": 1,
                        "total-pages": 1,
                        "total-count": 2
                    }
                }
            })))
            .mount(&mock_server)
            .await;

        let client = TfeClient::test_client(&mock_server.uri());
        let memberships = client.get_org_memberships("my-org").await.unwrap();

        assert_eq!(memberships.len(), 2);
        assert_eq!(memberships[0].email(), "user1@example.com");
        assert_eq!(memberships[0].status(), "active");
        assert_eq!(memberships[1].email(), "user2@example.com");
        assert_eq!(memberships[1].status(), "invited");
    }

    #[tokio::test]
    async fn test_invite_user_simple() {
        let mock_server = MockServer::start().await;

        // Mock the email check (no existing membership)
        Mock::given(method("GET"))
            .and(path("/organizations/my-org/organization-memberships"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": []
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/organizations/my-org/organization-memberships"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "data": {
                    "id": "ou-new123",
                    "type": "organization-memberships",
                    "attributes": {
                        "email": "newuser@example.com",
                        "status": "invited"
                    }
                }
            })))
            .mount(&mock_server)
            .await;

        let client = TfeClient::test_client(&mock_server.uri());
        let membership = client
            .invite_user("my-org", "newuser@example.com", None)
            .await
            .unwrap();

        assert_eq!(membership.id, "ou-new123");
        assert_eq!(membership.email(), "newuser@example.com");
        assert_eq!(membership.status(), "invited");
    }

    #[tokio::test]
    async fn test_invite_user_with_teams() {
        let mock_server = MockServer::start().await;

        // Mock the email check (no existing membership)
        Mock::given(method("GET"))
            .and(path("/organizations/my-org/organization-memberships"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": []
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/organizations/my-org/organization-memberships"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "data": {
                    "id": "ou-withteams",
                    "type": "organization-memberships",
                    "attributes": {
                        "email": "teamuser@example.com",
                        "status": "invited"
                    },
                    "relationships": {
                        "teams": {
                            "data": [
                                { "id": "team-1", "type": "teams" }
                            ]
                        }
                    }
                }
            })))
            .mount(&mock_server)
            .await;

        let client = TfeClient::test_client(&mock_server.uri());
        let membership = client
            .invite_user(
                "my-org",
                "teamuser@example.com",
                Some(vec!["team-1".to_string()]),
            )
            .await
            .unwrap();

        assert_eq!(membership.id, "ou-withteams");
        assert_eq!(membership.team_ids(), vec!["team-1"]);
    }

    #[tokio::test]
    async fn test_invite_user_already_exists() {
        let mock_server = MockServer::start().await;

        // Mock the email check - user already has membership
        Mock::given(method("GET"))
            .and(path("/organizations/my-org/organization-memberships"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [{
                    "id": "ou-existing",
                    "type": "organization-memberships",
                    "attributes": {
                        "email": "existing@example.com",
                        "status": "invited"
                    }
                }]
            })))
            .mount(&mock_server)
            .await;

        let client = TfeClient::test_client(&mock_server.uri());
        let result = client
            .invite_user("my-org", "existing@example.com", None)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("already has membership"));
        assert!(err.to_string().contains("invited"));
    }

    #[tokio::test]
    async fn test_delete_org_membership() {
        let mock_server = MockServer::start().await;

        Mock::given(method("DELETE"))
            .and(path("/organization-memberships/ou-todelete"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&mock_server)
            .await;

        let client = TfeClient::test_client(&mock_server.uri());
        let result = client.delete_org_membership("ou-todelete").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_delete_org_membership_not_found() {
        let mock_server = MockServer::start().await;

        Mock::given(method("DELETE"))
            .and(path("/organization-memberships/ou-unknown"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let client = TfeClient::test_client(&mock_server.uri());
        let result = client.delete_org_membership("ou-unknown").await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_get_org_membership_by_email_found() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/organization-memberships"))
            .and(query_param("filter[email]", "user@example.com"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [{
                    "id": "ou-found123",
                    "type": "organization-memberships",
                    "attributes": {
                        "email": "user@example.com",
                        "status": "active"
                    }
                }]
            })))
            .mount(&mock_server)
            .await;

        let client = TfeClient::test_client(&mock_server.uri());
        let result = client
            .get_org_membership_by_email("my-org", "user@example.com")
            .await
            .unwrap();

        assert!(result.is_some());
        let membership = result.unwrap();
        assert_eq!(membership.id, "ou-found123");
        assert_eq!(membership.email(), "user@example.com");
        assert_eq!(membership.status(), "active");
    }

    #[tokio::test]
    async fn test_get_org_membership_by_email_not_found() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/organization-memberships"))
            .and(query_param("filter[email]", "nobody@example.com"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": []
            })))
            .mount(&mock_server)
            .await;

        let client = TfeClient::test_client(&mock_server.uri());
        let result = client
            .get_org_membership_by_email("my-org", "nobody@example.com")
            .await
            .unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_org_membership_by_email_org_not_found() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/organizations/unknown-org/organization-memberships"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let client = TfeClient::test_client(&mock_server.uri());
        let result = client
            .get_org_membership_by_email("unknown-org", "user@example.com")
            .await
            .unwrap();

        // 404 returns None, not error (org might not exist or we have no access)
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_org_memberships_pagination() {
        let mock_server = MockServer::start().await;

        // Page 1
        Mock::given(method("GET"))
            .and(path("/organizations/my-org/organization-memberships"))
            .and(query_param("page[number]", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [{
                    "id": "ou-page1",
                    "type": "organization-memberships",
                    "attributes": {
                        "email": "user1@example.com",
                        "status": "active"
                    }
                }],
                "meta": {
                    "pagination": {
                        "current-page": 1,
                        "total-pages": 2,
                        "total-count": 2
                    }
                }
            })))
            .mount(&mock_server)
            .await;

        // Page 2
        Mock::given(method("GET"))
            .and(path("/organizations/my-org/organization-memberships"))
            .and(query_param("page[number]", "2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [{
                    "id": "ou-page2",
                    "type": "organization-memberships",
                    "attributes": {
                        "email": "user2@example.com",
                        "status": "invited"
                    }
                }],
                "meta": {
                    "pagination": {
                        "current-page": 2,
                        "total-pages": 2,
                        "total-count": 2
                    }
                }
            })))
            .mount(&mock_server)
            .await;

        let client = TfeClient::test_client(&mock_server.uri());
        let memberships = client.get_org_memberships("my-org").await.unwrap();

        assert_eq!(memberships.len(), 2);
        assert_eq!(memberships[0].id, "ou-page1");
        assert_eq!(memberships[1].id, "ou-page2");
    }
}
