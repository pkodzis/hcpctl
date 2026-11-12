//! Organization API operations

use log::debug;

use crate::config::api;
use crate::error::{Result, TfeError};
use crate::hcp::TfeClient;

use super::models::Organization;
use crate::hcp::traits::ApiListResponse;

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

        let response = self.send_get(&url).await?;

        let orgs_response: ApiListResponse<Organization> =
            self.parse_api_response(response, "organizations").await?;
        Ok(orgs_response.data)
    }

    /// Fetch every accessible organization page, failing rather than returning partial results.
    pub(crate) async fn get_all_organizations(&self) -> Result<Vec<String>> {
        let organizations = self
            .fetch_all_pages::<Organization, ApiListResponse<Organization>>(
                "/organizations",
                "organizations",
            )
            .await?;
        Ok(organizations
            .into_iter()
            .map(|organization| organization.id)
            .collect())
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

        let response = self.send_get(&url).await?;

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
            let response = self.send_get(&url).await?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hcp::RetryPolicy;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn bulk_organization_listing_paginates_and_retries_later_pages() {
        let server = MockServer::start().await;
        for page in 1..=2 {
            let attempts = AtomicUsize::new(0);
            Mock::given(method("GET"))
                .and(path("/organizations"))
                .and(query_param("page[number]", page.to_string()))
                .respond_with(move |_: &wiremock::Request| {
                    if page == 2 && attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                        return ResponseTemplate::new(503);
                    }
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({
                        "data":[org_json(&format!("org-{page}"), &format!("org-ext-{page}"))],
                        "meta":{"pagination":{"current-page":page,"total-pages":2,"total-count":2}}
                    }))
                })
                .expect(if page == 2 { 2 } else { 1 })
                .mount(&server)
                .await;
        }
        let client =
            TfeClient::test_client(&server.uri()).with_request_policy(RetryPolicy::instant());
        assert_eq!(
            client.get_all_organizations().await.unwrap(),
            vec!["org-1", "org-2"]
        );
    }

    #[tokio::test]
    async fn bulk_organization_listing_later_page_error_is_not_partial_success() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/organizations"))
            .and(query_param("page[number]", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data":[org_json("org-1", "org-ext-1")],
                "meta":{"pagination":{"current-page":1,"total-pages":2,"total-count":2}}
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/organizations"))
            .and(query_param("page[number]", "2"))
            .respond_with(ResponseTemplate::new(401))
            .expect(1)
            .mount(&server)
            .await;
        let error = TfeClient::test_client(&server.uri())
            .with_request_policy(RetryPolicy::instant())
            .get_all_organizations()
            .await
            .unwrap_err();
        assert!(matches!(error, TfeError::Api { status: 401, .. }));
    }

    fn org_json(id: &str, external_id: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "type": "organizations",
            "attributes": {
                "name": id,
                "email": "test@example.com",
                "external-id": external_id,
                "created-at": "2025-01-01T00:00:00Z"
            }
        })
    }

    #[tokio::test]
    async fn test_get_organizations_success() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        let response_body = serde_json::json!({
            "data": [
                org_json("org-1", "org-ext-1"),
                org_json("org-2", "org-ext-2")
            ]
        });

        Mock::given(method("GET"))
            .and(path("/organizations"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&mock_server)
            .await;

        let result = client.get_organizations().await;

        assert!(result.is_ok());
        let orgs = result.unwrap();
        assert_eq!(orgs.len(), 2);
        assert_eq!(orgs[0], "org-1");
        assert_eq!(orgs[1], "org-2");
    }

    #[tokio::test]
    async fn test_get_organizations_full_success() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        let response_body = serde_json::json!({
            "data": [org_json("my-org", "org-ABC123")]
        });

        Mock::given(method("GET"))
            .and(path("/organizations"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&mock_server)
            .await;

        let result = client.get_organizations_full().await;

        assert!(result.is_ok());
        let orgs = result.unwrap();
        assert_eq!(orgs.len(), 1);
        assert_eq!(orgs[0].id, "my-org");
        assert_eq!(orgs[0].external_id(), "org-ABC123");
    }

    #[tokio::test]
    async fn test_get_organizations_api_error() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/organizations"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&mock_server)
            .await;

        let result = client.get_organizations().await;

        assert!(result.is_err());
        match result.unwrap_err() {
            TfeError::Api { status, .. } => assert_eq!(status, 401),
            _ => panic!("Expected TfeError::Api"),
        }
    }

    #[tokio::test]
    async fn test_get_organization_by_name_success() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        let response_body = serde_json::json!({
            "data": org_json("my-org", "org-XYZ")
        });

        Mock::given(method("GET"))
            .and(path("/organizations/my-org"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&mock_server)
            .await;

        let result = client.get_organization("my-org").await;

        assert!(result.is_ok());
        let (org, _raw) = result.unwrap().unwrap();
        assert_eq!(org.id, "my-org");
    }

    #[tokio::test]
    async fn test_get_organization_not_found() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/organizations/nonexistent"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        // Name doesn't start with "org-" so no fallback
        let result = client.get_organization("nonexistent").await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_get_organization_by_external_id_fallback() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        // First call to /organizations/org-ABC123 returns 404
        Mock::given(method("GET"))
            .and(path("/organizations/org-ABC123"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        // Then falls back to listing all orgs
        let list_response = serde_json::json!({
            "data": [org_json("my-org", "org-ABC123")]
        });
        Mock::given(method("GET"))
            .and(path("/organizations"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&list_response))
            .mount(&mock_server)
            .await;

        // And fetches full details by name
        let detail_response = serde_json::json!({
            "data": org_json("my-org", "org-ABC123")
        });
        Mock::given(method("GET"))
            .and(path("/organizations/my-org"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&detail_response))
            .mount(&mock_server)
            .await;

        let result = client.get_organization("org-ABC123").await;

        assert!(result.is_ok());
        let (org, _raw) = result.unwrap().unwrap();
        assert_eq!(org.id, "my-org");
        assert_eq!(org.external_id(), "org-ABC123");
    }
}
