//! Run API operations

use log::debug;

use crate::config::api;
use crate::error::{Result, TfeError};
use crate::hcp::TfeClient;

use super::models::{Run, RunQuery, RunsResponse};

impl TfeClient {
    /// Get runs for a workspace with optional filters
    ///
    /// Uses API query parameters for efficient server-side filtering:
    /// - `filter[status_group]` for status group filtering (non_final, final, discardable)
    /// - `filter[status]` for specific status filtering
    ///
    /// Note: This endpoint has a rate limit of 30 requests per minute.
    pub async fn get_runs_for_workspace(
        &self,
        workspace_id: &str,
        query: RunQuery,
        max_results: Option<u32>,
    ) -> Result<Vec<Run>> {
        let mut all_runs = Vec::new();
        let mut page = query.page.unwrap_or(1);
        let page_size = query.page_size.unwrap_or(api::DEFAULT_PAGE_SIZE);

        loop {
            let mut url = format!(
                "{}/{}/{}/{}?page[size]={}&page[number]={}",
                self.base_url(),
                api::WORKSPACES,
                workspace_id,
                api::RUNS,
                page_size,
                page
            );

            // Add server-side filters
            self.append_run_query_params(&mut url, &query);

            debug!("Fetching runs page {} from: {}", page, url);

            let response = self.get(&url).send().await?;

            if !response.status().is_success() {
                return Err(TfeError::Api {
                    status: response.status().as_u16(),
                    message: format!("Failed to fetch runs for workspace '{}'", workspace_id),
                });
            }

            let runs_response: RunsResponse = response.json().await?;
            let run_count = runs_response.data.len();
            all_runs.extend(runs_response.data);

            // Check if we've reached max_results
            if let Some(max) = max_results {
                if all_runs.len() >= max as usize {
                    all_runs.truncate(max as usize);
                    break;
                }
            }

            // Check if there are more pages
            if let Some(meta) = runs_response.meta {
                if let Some(pagination) = meta.pagination {
                    debug!(
                        "Page {}/{:?}, fetched {} runs",
                        pagination.current_page, pagination.total_pages, run_count
                    );

                    if pagination.next_page.is_none() {
                        break;
                    }
                    page += 1;
                } else {
                    break;
                }
            } else {
                break;
            }

            if run_count == 0 {
                break;
            }
        }

        debug!(
            "Fetched {} runs for workspace '{}'",
            all_runs.len(),
            workspace_id
        );
        Ok(all_runs)
    }

    /// Get runs for an organization with optional filters
    ///
    /// Uses API query parameters for efficient server-side filtering:
    /// - `filter[status_group]` for status group filtering (non_final, final, discardable)
    /// - `filter[status]` for specific status filtering
    /// - `filter[workspace_names]` for filtering by workspace names
    ///
    /// Note: This endpoint has a rate limit of 30 requests per minute.
    /// Note: The org endpoint does not return total-count in pagination.
    pub async fn get_runs_for_organization(
        &self,
        org: &str,
        query: RunQuery,
        max_results: Option<u32>,
    ) -> Result<Vec<Run>> {
        let mut all_runs = Vec::new();
        let mut page = query.page.unwrap_or(1);
        let page_size = query.page_size.unwrap_or(api::DEFAULT_PAGE_SIZE);

        loop {
            let mut url = format!(
                "{}/{}/{}/{}?page[size]={}&page[number]={}",
                self.base_url(),
                api::ORGANIZATIONS,
                org,
                api::RUNS,
                page_size,
                page
            );

            // Add server-side filters
            self.append_run_query_params(&mut url, &query);

            // Add workspace names filter (org endpoint only)
            if let Some(ws_names) = &query.workspace_names {
                if !ws_names.is_empty() {
                    url.push_str(&format!(
                        "&filter[workspace_names]={}",
                        ws_names
                            .iter()
                            .map(|s| urlencoding::encode(s).into_owned())
                            .collect::<Vec<_>>()
                            .join(",")
                    ));
                }
            }

            debug!("Fetching runs page {} from: {}", page, url);

            let response = self.get(&url).send().await?;

            if !response.status().is_success() {
                return Err(TfeError::Api {
                    status: response.status().as_u16(),
                    message: format!("Failed to fetch runs for organization '{}'", org),
                });
            }

            let runs_response: RunsResponse = response.json().await?;
            let run_count = runs_response.data.len();
            all_runs.extend(runs_response.data);

            // Check if we've reached max_results
            if let Some(max) = max_results {
                if all_runs.len() >= max as usize {
                    all_runs.truncate(max as usize);
                    break;
                }
            }

            // Check if there are more pages (org endpoint doesn't have total_pages)
            if let Some(meta) = runs_response.meta {
                if let Some(pagination) = meta.pagination {
                    debug!(
                        "Page {}, fetched {} runs, next_page: {:?}",
                        pagination.current_page, run_count, pagination.next_page
                    );

                    if pagination.next_page.is_none() {
                        break;
                    }
                    page += 1;
                } else {
                    break;
                }
            } else {
                break;
            }

            if run_count == 0 {
                break;
            }
        }

        debug!("Fetched {} runs for organization '{}'", all_runs.len(), org);
        Ok(all_runs)
    }

    /// Get a single run by ID
    pub async fn get_run_by_id(&self, run_id: &str) -> Result<Option<(Run, serde_json::Value)>> {
        let url = format!("{}/{}/{}", self.base_url(), api::RUNS, run_id);

        debug!("Fetching run by ID: {}", url);

        let response = self.get(&url).send().await?;

        if response.status().as_u16() == 404 {
            return Ok(None);
        }

        if !response.status().is_success() {
            return Err(TfeError::Api {
                status: response.status().as_u16(),
                message: format!("Failed to fetch run '{}'", run_id),
            });
        }

        let raw: serde_json::Value = response.json().await?;
        let run: Run = serde_json::from_value(raw["data"].clone())?;
        Ok(Some((run, raw)))
    }

    /// Helper to append run query parameters to URL
    fn append_run_query_params(&self, url: &mut String, query: &RunQuery) {
        // Add status group filter
        if let Some(status_group) = &query.status_group {
            url.push_str(&format!("&filter[status_group]={}", status_group));
        }

        // Add specific status filter
        if let Some(statuses) = &query.statuses {
            if !statuses.is_empty() {
                url.push_str(&format!(
                    "&filter[status]={}",
                    statuses
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                ));
            }
        }
    }

    /// Get the plan for a run
    pub async fn get_run_plan(&self, run_id: &str) -> Result<super::models::Plan> {
        let url = format!("{}/{}/{}/plan", self.base_url(), api::RUNS, run_id);

        debug!("Fetching plan for run: {}", url);

        let response = self.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(TfeError::Api {
                status: response.status().as_u16(),
                message: format!("Failed to fetch plan for run '{}'", run_id),
            });
        }

        let plan_response: super::models::PlanResponse = response.json().await?;
        Ok(plan_response.data)
    }

    /// Get the apply for a run
    pub async fn get_run_apply(&self, run_id: &str) -> Result<super::models::Apply> {
        let url = format!("{}/{}/{}/apply", self.base_url(), api::RUNS, run_id);

        debug!("Fetching apply for run: {}", url);

        let response = self.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(TfeError::Api {
                status: response.status().as_u16(),
                message: format!("Failed to fetch apply for run '{}'", run_id),
            });
        }

        let apply_response: super::models::ApplyResponse = response.json().await?;
        Ok(apply_response.data)
    }

    /// Get log content from a log-read-url
    ///
    /// The log-read-url is a temporary authenticated URL that expires in 1 minute.
    /// This method fetches the raw log content from the archivist.
    pub async fn get_log_content(&self, log_read_url: &str) -> Result<String> {
        debug!("Fetching log content from: {}", log_read_url);

        // Note: log-read-url is a pre-authenticated URL, so we use a raw client
        let response = reqwest::get(log_read_url).await?;

        if !response.status().is_success() {
            return Err(TfeError::Api {
                status: response.status().as_u16(),
                message: "Failed to fetch log content".to_string(),
            });
        }

        let content = response.text().await?;
        Ok(content)
    }

    /// Cancel a run that is actively executing (planning or applying)
    ///
    /// Sends POST /runs/:run_id/actions/cancel
    /// The run must have is-cancelable: true in its actions.
    pub async fn cancel_run(&self, run_id: &str) -> Result<()> {
        let url = format!(
            "{}/{}/{}/actions/cancel",
            self.base_url(),
            api::RUNS,
            run_id
        );

        debug!("Canceling run: {}", run_id);

        let response = self.post(&url).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(TfeError::Api {
                status,
                message: format!("Failed to cancel run '{}': {}", run_id, body),
            });
        }

        Ok(())
    }

    /// Discard a run that is waiting for confirmation or priority
    ///
    /// Sends POST /runs/:run_id/actions/discard
    /// The run must have is-discardable: true in its actions.
    pub async fn discard_run(&self, run_id: &str) -> Result<()> {
        let url = format!(
            "{}/{}/{}/actions/discard",
            self.base_url(),
            api::RUNS,
            run_id
        );

        debug!("Discarding run: {}", run_id);

        let response = self.post(&url).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(TfeError::Api {
                status,
                message: format!("Failed to discard run '{}': {}", run_id, body),
            });
        }

        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn create_test_client(base_url: &str) -> TfeClient {
        TfeClient::with_base_url(
            "test-token".to_string(),
            "mock.terraform.io".to_string(),
            base_url.to_string(),
        )
    }

    fn sample_runs_response() -> serde_json::Value {
        serde_json::json!({
            "data": [
                {
                    "id": "run-abc123",
                    "type": "runs",
                    "attributes": {
                        "status": "planning",
                        "message": "Triggered via API",
                        "source": "tfe-api",
                        "created-at": "2025-01-01T10:00:00.000Z",
                        "has-changes": true,
                        "is-destroy": false,
                        "plan-only": false,
                        "trigger-reason": "manual",
                        "actions": {
                            "is-cancelable": true,
                            "is-confirmable": false,
                            "is-discardable": false,
                            "is-force-cancelable": false
                        }
                    },
                    "relationships": {
                        "workspace": {
                            "data": {
                                "id": "ws-xyz789",
                                "type": "workspaces"
                            }
                        }
                    }
                }
            ],
            "meta": {
                "pagination": {
                    "current-page": 1,
                    "page-size": 20,
                    "next-page": null,
                    "prev-page": null
                }
            }
        })
    }

    #[tokio::test]
    async fn test_get_runs_for_workspace_success() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-test123/runs"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_runs_response()))
            .mount(&mock_server)
            .await;

        let query = RunQuery::non_final();
        let result = client
            .get_runs_for_workspace("ws-test123", query, None)
            .await;

        assert!(result.is_ok());
        let runs = result.unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].id, "run-abc123");
        assert_eq!(runs[0].status(), "planning");
    }

    #[tokio::test]
    async fn test_get_runs_for_organization_success() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/runs"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_runs_response()))
            .mount(&mock_server)
            .await;

        let query = RunQuery::non_final();
        let result = client
            .get_runs_for_organization("my-org", query, None)
            .await;

        assert!(result.is_ok());
        let runs = result.unwrap();
        assert_eq!(runs.len(), 1);
    }

    #[tokio::test]
    async fn test_get_runs_with_workspace_names_filter() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/runs"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_runs_response()))
            .mount(&mock_server)
            .await;

        let query = RunQuery {
            workspace_names: Some(vec!["ws1".to_string(), "ws2".to_string()]),
            ..Default::default()
        };
        let result = client
            .get_runs_for_organization("my-org", query, None)
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_run_by_id_success() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(&mock_server.uri());

        let run_response = serde_json::json!({
            "data": {
                "id": "run-abc123",
                "type": "runs",
                "attributes": {
                    "status": "applied",
                    "message": "Apply complete",
                    "source": "tfe-ui",
                    "created-at": "2025-01-01T10:00:00.000Z"
                }
            }
        });

        Mock::given(method("GET"))
            .and(path("/runs/run-abc123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(run_response))
            .mount(&mock_server)
            .await;

        let result = client.get_run_by_id("run-abc123").await;

        assert!(result.is_ok());
        let (run, _raw) = result.unwrap().unwrap();
        assert_eq!(run.id, "run-abc123");
        assert_eq!(run.status(), "applied");
    }

    #[tokio::test]
    async fn test_get_run_by_id_not_found() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/runs/run-notfound"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let result = client.get_run_by_id("run-notfound").await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_get_runs_api_error() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-test123/runs"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let query = RunQuery::non_final();
        let result = client
            .get_runs_for_workspace("ws-test123", query, None)
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_runs_max_results() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(&mock_server.uri());

        // Response with multiple runs
        let response = serde_json::json!({
            "data": [
                {"id": "run-1", "type": "runs", "attributes": {"status": "planning"}},
                {"id": "run-2", "type": "runs", "attributes": {"status": "planning"}},
                {"id": "run-3", "type": "runs", "attributes": {"status": "planning"}}
            ],
            "meta": {
                "pagination": {
                    "current-page": 1,
                    "page-size": 20,
                    "next-page": null,
                    "prev-page": null
                }
            }
        });

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-test123/runs"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response))
            .mount(&mock_server)
            .await;

        let query = RunQuery::non_final();
        let result = client
            .get_runs_for_workspace("ws-test123", query, Some(2))
            .await;

        assert!(result.is_ok());
        let runs = result.unwrap();
        assert_eq!(runs.len(), 2); // Limited to max_results
    }

    #[tokio::test]
    async fn test_get_run_plan_success() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(&mock_server.uri());

        let plan_response = serde_json::json!({
            "data": {
                "id": "plan-abc123",
                "type": "plans",
                "attributes": {
                    "status": "finished",
                    "has-changes": true,
                    "resource-additions": 5,
                    "resource-changes": 2,
                    "resource-destructions": 1,
                    "resource-imports": 0,
                    "log-read-url": "https://archivist.terraform.io/v1/object/abc123"
                }
            }
        });

        Mock::given(method("GET"))
            .and(path("/runs/run-test123/plan"))
            .respond_with(ResponseTemplate::new(200).set_body_json(plan_response))
            .mount(&mock_server)
            .await;

        let result = client.get_run_plan("run-test123").await;

        assert!(result.is_ok());
        let plan = result.unwrap();
        assert_eq!(plan.id, "plan-abc123");
        assert_eq!(plan.status(), "finished");
        assert!(plan.has_changes());
        assert_eq!(plan.resource_additions(), 5);
        assert_eq!(plan.resource_changes(), 2);
        assert_eq!(plan.resource_destructions(), 1);
        assert!(plan.is_final());
    }

    #[tokio::test]
    async fn test_get_run_plan_error() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/runs/run-notfound/plan"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let result = client.get_run_plan("run-notfound").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_run_apply_success() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(&mock_server.uri());

        let apply_response = serde_json::json!({
            "data": {
                "id": "apply-xyz789",
                "type": "applies",
                "attributes": {
                    "status": "finished",
                    "resource-additions": 3,
                    "resource-changes": 1,
                    "resource-destructions": 0,
                    "resource-imports": 2,
                    "log-read-url": "https://archivist.terraform.io/v1/object/xyz789"
                }
            }
        });

        Mock::given(method("GET"))
            .and(path("/runs/run-test456/apply"))
            .respond_with(ResponseTemplate::new(200).set_body_json(apply_response))
            .mount(&mock_server)
            .await;

        let result = client.get_run_apply("run-test456").await;

        assert!(result.is_ok());
        let apply = result.unwrap();
        assert_eq!(apply.id, "apply-xyz789");
        assert_eq!(apply.status(), "finished");
        assert_eq!(apply.resource_additions(), 3);
        assert_eq!(apply.resource_changes(), 1);
        assert!(apply.is_final());
    }

    #[tokio::test]
    async fn test_get_run_apply_error() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/runs/run-notfound/apply"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let result = client.get_run_apply("run-notfound").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_log_content_success() {
        let mock_server = MockServer::start().await;

        let log_content = "Terraform v1.5.0\nInitializing...\nApply complete!";

        Mock::given(method("GET"))
            .and(path("/v1/object/testlog"))
            .respond_with(ResponseTemplate::new(200).set_body_string(log_content))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server.uri());
        let log_url = format!("{}/v1/object/testlog", mock_server.uri());

        let result = client.get_log_content(&log_url).await;

        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(content.contains("Terraform v1.5.0"));
        assert!(content.contains("Apply complete!"));
    }

    #[tokio::test]
    async fn test_cancel_run_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/runs/run-abc123/actions/cancel"))
            .respond_with(ResponseTemplate::new(202))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server.uri());
        let result = client.cancel_run("run-abc123").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_cancel_run_not_cancelable() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/runs/run-abc123/actions/cancel"))
            .respond_with(
                ResponseTemplate::new(409)
                    .set_body_string(r#"{"errors":[{"status":"409","title":"conflict"}]}"#),
            )
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server.uri());
        let result = client.cancel_run("run-abc123").await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_discard_run_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/runs/run-xyz789/actions/discard"))
            .respond_with(ResponseTemplate::new(202))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server.uri());
        let result = client.discard_run("run-xyz789").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_discard_run_not_discardable() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/runs/run-xyz789/actions/discard"))
            .respond_with(
                ResponseTemplate::new(409)
                    .set_body_string(r#"{"errors":[{"status":"409","title":"conflict"}]}"#),
            )
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server.uri());
        let result = client.discard_run("run-xyz789").await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cancel_run_not_found() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/runs/run-notfound/actions/cancel"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server.uri());
        let result = client.cancel_run("run-notfound").await;

        assert!(result.is_err());
    }
}
