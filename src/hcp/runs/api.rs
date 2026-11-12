//! Run API operations

use log::debug;

use std::collections::HashSet;
use std::time::Duration;
use tokio::time::Instant;

use crate::config::api;
use crate::error::{Result, TfeError};
use crate::hcp::client::{send_coordinated, send_with_retry};
use crate::hcp::traits::ApiListResponse;
use crate::hcp::{RetryPolicy, TfeClient};

use super::models::{select_newest_runs, Apply, ApplyResponse, Plan, PlanResponse};
use super::models::{Run, RunQuery, RunsResponse};

/// Operations requested when listing history for bulk log download
///
/// Includes `action_only` so that no run type is silently dropped.
pub(crate) const BULK_RUN_OPERATIONS: &str =
    "plan_and_apply,plan_only,save_plan,refresh_only,destroy,empty_apply,action_only";

/// Operations understood by older TFE versions (no `action_only`)
const LEGACY_RUN_OPERATIONS: &str =
    "plan_and_apply,plan_only,save_plan,refresh_only,destroy,empty_apply";

/// True when the leading `runs` hold at least `count` distinct runs and are
/// verifiably ordered newest first, so later pages cannot contain newer runs.
fn enough_newest_runs(runs: &[Run], count: usize) -> bool {
    let mut seen = HashSet::new();
    let mut previous = None;
    for run in runs {
        let Some(created) = run
            .attributes
            .created_at
            .as_deref()
            .and_then(|c| chrono::DateTime::parse_from_rfc3339(c).ok())
        else {
            return false;
        };
        if previous.is_some_and(|p| created > p) {
            return false;
        }
        previous = Some(created);
        seen.insert(run.id.as_str());
    }
    seen.len() >= count
}

impl TfeClient {
    /// Get the newest `count` runs of a workspace by creation time, oldest first
    ///
    /// Reads the newest-first history only as far as needed (all operation
    /// types, any status), sorts locally by `(created-at, id)` and keeps the
    /// last `count`.
    pub async fn get_newest_runs_for_workspace(
        &self,
        workspace_id: &str,
        count: usize,
        policy: &RetryPolicy,
    ) -> Result<Vec<Run>> {
        Ok(self
            .get_bulk_run_history(workspace_id, count, "plan,apply", policy)
            .await?
            .0)
    }

    pub(crate) async fn get_bulk_run_history(
        &self,
        workspace_id: &str,
        count: usize,
        phases: &str,
        policy: &RetryPolicy,
    ) -> Result<(Vec<Run>, Vec<serde_json::Value>, Instant)> {
        // The API lists runs newest first, so only the leading pages are read.
        // Reading continues while the order is not verifiably newest-first.
        let enough = |runs: &[Run]| enough_newest_runs(runs, count);
        // Request only as many runs per page as needed (API maximum is 100)
        let page_size = count.clamp(1, 100);
        let acquired = Instant::now();
        let mut operations = BULK_RUN_OPERATIONS;
        let mut include = true;
        let (mut runs, included) = loop {
            let mut path = format!(
                "/workspaces/{}/runs?filter[operation]={}",
                workspace_id, operations
            );
            if include {
                path.push_str(&format!("&include={}", phases));
            }
            match self
                .fetch_all_pages_bulk::<Run, ApiListResponse<Run>>(
                    &path, page_size, policy, &enough,
                )
                .await
            {
                Err(TfeError::Api {
                    status: status @ (400 | 422),
                    message,
                }) => {
                    // Older TFE versions reject newer operation values; the
                    // error names the filter. Otherwise the include is at fault.
                    // Never drop any other operation silently.
                    if operations == BULK_RUN_OPERATIONS && message.contains("action_only") {
                        operations = LEGACY_RUN_OPERATIONS;
                        policy.diagnostic(
                            "Server does not support the action_only run operation; omitting it."
                                .into(),
                        );
                    } else if include
                        && !message.contains("filter param")
                        && !message.contains("action_only")
                    {
                        include = false;
                        policy.diagnostic(
                            "Run history includes rejected; using phase metadata fallback.".into(),
                        );
                    } else {
                        return Err(TfeError::Api { status, message });
                    }
                }
                result => break result?,
            }
        };
        // Pages may overlap when runs are created during the read.
        let mut seen = HashSet::new();
        runs.retain(|run| seen.insert(run.id.clone()));
        let selected = select_newest_runs(runs, count).map_err(|message| TfeError::Api {
            status: 200,
            message,
        })?;
        Ok((selected, included, acquired))
    }

    /// Fetch a bulk phase independently; absent/empty blobs are errors only here.
    pub(crate) async fn fetch_bulk_phase(
        &self,
        run: &Run,
        is_apply: bool,
        included: &[serde_json::Value],
        acquired: Instant,
        policy: &RetryPolicy,
    ) -> Result<String> {
        let phase = if is_apply { "apply" } else { "plan" };
        // A shared gate may have held a worker for longer than a URL's useful
        // age. Check freshness only after that wait, before resolving metadata.
        policy.wait_for_gate().await?;
        let relationship = run.relationships.as_ref().and_then(|r| {
            if is_apply {
                r.apply.as_ref()
            } else {
                r.plan.as_ref()
            }
        });
        let id = relationship
            .and_then(|r| r.data.as_ref())
            .map(|r| r.id.as_str());
        let info = included
            .iter()
            .find(|item| {
                item["type"] == if is_apply { "applies" } else { "plans" }
                    && id.is_some()
                    && item["id"].as_str() == id
            })
            .and_then(|item| item["attributes"].get("log-read-url"));
        let missing = || TfeError::Api {
            status: 404,
            message: format!("No {} log available", phase),
        };
        let mut url = match info {
            Some(value) if acquired.elapsed() <= Duration::from_secs(30) => {
                value.as_str().map(String::from)
            }
            _ => None,
        };
        if url.is_none() {
            url = self.get_phase_log_url(&run.id, is_apply, policy).await?;
        }
        let mut url = url.ok_or_else(missing)?;
        let mut refreshed_403 = false;
        let mut rates = 0;
        let mut waited = Duration::ZERO;
        let blob_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;
        loop {
            let response = send_coordinated(policy, || blob_client.get(&url), false, true)
                .await
                .map_err(|error| match error {
                    TfeError::Http(error) => TfeError::Http(error.without_url()),
                    error => error,
                })?;
            match response.status().as_u16() {
                200..=299 => {
                    let content = response
                        .text()
                        .await
                        .map_err(|error| TfeError::Http(error.without_url()))?;
                    return if content.is_empty() {
                        Err(missing())
                    } else {
                        Ok(content)
                    };
                }
                404 => return Err(missing()),
                403 if !refreshed_403 => refreshed_403 = true,
                429 => {
                    let wait = policy.rate_wait(response.headers());
                    if rates >= policy.max_rate_retries
                        || wait > policy.max_total_rate_wait.saturating_sub(waited)
                    {
                        return Err(TfeError::Api {
                            status: 429,
                            message: "rate limited: blob retry budget exhausted".into(),
                        });
                    }
                    policy.diagnostic(format!(
                        "Rate limited retrieving run {} {}, waiting {}s",
                        run.id,
                        phase,
                        wait.as_secs()
                    ));
                    policy.pause(wait).await?;
                    waited += wait;
                    rates += 1;
                }
                status => {
                    return Err(TfeError::Api {
                        status,
                        message: "Failed to fetch log content".into(),
                    })
                }
            }
            url = self
                .get_phase_log_url(&run.id, is_apply, policy)
                .await?
                .ok_or_else(missing)?;
        }
    }

    /// Get the `log-read-url` of a run's plan or apply, with retry
    ///
    /// Returns `None` when the phase has no log URL or the phase does not exist (404).
    pub(crate) async fn get_phase_log_url(
        &self,
        run_id: &str,
        is_apply: bool,
        policy: &RetryPolicy,
    ) -> Result<Option<String>> {
        let phase = if is_apply { "apply" } else { "plan" };
        let url = format!("{}/{}/{}/{}", self.base_url(), api::RUNS, run_id, phase);
        debug!("Fetching {} for run: {}", phase, url);

        let response = send_with_retry(policy, || self.get(&url)).await?;
        if response.status().as_u16() == 404 {
            return Ok(None);
        }
        let context = format!("{} for run '{}'", phase, run_id);
        if is_apply {
            let parsed: ApplyResponse = self.parse_api_response(response, &context).await?;
            let apply: Apply = parsed.data;
            Ok(apply.attributes.log_read_url)
        } else {
            let parsed: PlanResponse = self.parse_api_response(response, &context).await?;
            let plan: Plan = parsed.data;
            Ok(plan.attributes.log_read_url)
        }
    }

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

            let runs_response: RunsResponse = self
                .parse_api_response(response, &format!("runs for workspace '{}'", workspace_id))
                .await?;
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

            let runs_response: RunsResponse = self
                .parse_api_response(response, &format!("runs for organization '{}'", org))
                .await?;
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
        let path = format!("/{}/{}", api::RUNS, run_id);
        self.fetch_resource_by_path::<Run>(&path, &format!("run '{}'", run_id))
            .await
    }

    /// Helper to append run query parameters to URL
    fn append_run_query_params(&self, url: &mut String, query: &RunQuery) {
        // Include all operation types (API excludes plan_only by default)
        if query.include_all_operations {
            url.push_str("&filter[operation]=plan_and_apply,plan_only,save_plan,refresh_only,destroy,empty_apply");
        }

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

        let plan_response: super::models::PlanResponse = self
            .parse_api_response(response, &format!("plan for run '{}'", run_id))
            .await?;
        Ok(plan_response.data)
    }

    /// Get the apply for a run
    pub async fn get_run_apply(&self, run_id: &str) -> Result<super::models::Apply> {
        let url = format!("{}/{}/{}/apply", self.base_url(), api::RUNS, run_id);

        debug!("Fetching apply for run: {}", url);

        let response = self.get(&url).send().await?;

        let apply_response: super::models::ApplyResponse = self
            .parse_api_response(response, &format!("apply for run '{}'", run_id))
            .await?;
        Ok(apply_response.data)
    }

    /// Get log content from a log-read-url
    ///
    /// The log-read-url is a temporary pre-authenticated blob URL.
    /// This method fetches the raw log content from the archivist.
    pub async fn get_log_content(&self, log_read_url: &str) -> Result<String> {
        debug!("Fetching log content from archivist");

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
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn verification_stale_null_include_refreshes_available_plan() {
        null_include_refreshes_available_phase(false, Duration::from_secs(31)).await;
    }

    #[tokio::test]
    async fn verification_stale_null_include_refreshes_available_apply() {
        null_include_refreshes_available_phase(true, Duration::from_secs(31)).await;
    }

    #[tokio::test]
    async fn verification_round_one_fresh_null_refreshes_available_plan() {
        null_include_refreshes_available_phase(false, Duration::from_secs(1)).await;
    }

    #[tokio::test]
    async fn verification_round_one_fresh_null_refreshes_available_apply() {
        null_include_refreshes_available_phase(true, Duration::from_secs(1)).await;
    }

    async fn null_include_refreshes_available_phase(apply: bool, age: Duration) {
        let server = MockServer::start().await;
        let phase = if apply { "apply" } else { "plan" };
        let run: Run = serde_json::from_value(serde_json::json!({
            "id":"run-abc", "attributes":{"status":if apply {"applying"} else {"planning"}},
            "relationships":{phase:{"data":{"id":format!("{phase}-abc")}}}
        }))
        .unwrap();
        let included = vec![serde_json::json!({
            "type":if apply {"applies"} else {"plans"},
            "id":format!("{phase}-abc"), "attributes":{"log-read-url":null}
        })];
        Mock::given(method("GET"))
            .and(path(format!("/runs/run-abc/{phase}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data":{"id":format!("{phase}-abc"),"attributes":{
                    "status":"finished","log-read-url":format!("{}/blob",server.uri())
                }}
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/blob"))
            .respond_with(ResponseTemplate::new(200).set_body_string("now available"))
            .expect(1)
            .mount(&server)
            .await;
        let result = TfeClient::test_client(&server.uri())
            .fetch_bulk_phase(
                &run,
                apply,
                &included,
                Instant::now() - age,
                &RetryPolicy::instant(),
            )
            .await;
        assert_eq!(result.unwrap(), "now available");
        assert_eq!(server.received_requests().await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn bulk_history_omits_action_only_on_older_servers() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(query_param("filter[operation]", BULK_RUN_OPERATIONS))
            .respond_with(ResponseTemplate::new(400).set_body_string(
                "Bad value [action_only] given for query filter param[operation].",
            ))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(query_param("filter[operation]", LEGACY_RUN_OPERATIONS))
            .and(query_param("include", "plan,apply"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data":[{"id":"run-a","attributes":{"status":"applied","created-at":"2026-01-01T00:00:00Z"}}],
                "meta":{"pagination":{"current-page":1,"total-pages":1,"total-count":1}}
            })))
            .expect(1)
            .mount(&server)
            .await;
        let runs = TfeClient::test_client(&server.uri())
            .get_newest_runs_for_workspace("ws-abc", 1, &RetryPolicy::instant())
            .await
            .unwrap();
        assert_eq!(runs.len(), 1);
    }

    #[tokio::test]
    async fn bulk_history_reads_only_needed_newest_pages() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/workspaces/ws-abc/runs"))
            .and(query_param("page[number]", "1"))
            .and(query_param("include", "plan,apply"))
            .and(query_param("filter[operation]", BULK_RUN_OPERATIONS))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {"id":"run-new","attributes":{"status":"errored","created-at":"2026-01-01T00:00:00Z"}},
                    {"id":"run-old","attributes":{"status":"canceled","created-at":"2025-01-01T00:00:00Z"}}
                ],
                "included":[{"type":"plans","id":"plan-new"}],
                "meta":{"pagination":{"current-page":1,"total-pages":5,"total-count":450}}
            })))
            .expect(1)
            .mount(&server).await;
        let (runs, included, _) = TfeClient::test_client(&server.uri())
            .get_bulk_run_history("ws-abc", 1, "plan,apply", &RetryPolicy::instant())
            .await
            .unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].id, "run-new");
        assert_eq!(included.len(), 1);
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn bulk_history_ignores_changing_pagination_totals() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(query_param("page[number]", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data":[{"id":"run-b","attributes":{"status":"applied","created-at":"2026-01-02T00:00:00Z"}}],
                "meta":{"pagination":{"current-page":1,"total-pages":2,"total-count":2}}
            }))).expect(1).mount(&server).await;
        Mock::given(method("GET"))
            .and(query_param("page[number]", "2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data":[{"id":"run-a","attributes":{"status":"applied","created-at":"2026-01-01T00:00:00Z"}}],
                "meta":{"pagination":{"current-page":2,"total-pages":2,"total-count":4999}}
            }))).expect(1).mount(&server).await;
        let runs = TfeClient::test_client(&server.uri())
            .get_newest_runs_for_workspace("ws-abc", 5, &RetryPolicy::instant())
            .await
            .unwrap();
        assert_eq!(runs.len(), 2);
    }

    #[tokio::test]
    async fn bulk_history_http_and_json_errors_do_not_trigger_include_fallback() {
        for status in [401, 404, 500, 200] {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/workspaces/ws-abc/runs"))
                .and(query_param("include", "plan,apply"))
                .respond_with(ResponseTemplate::new(status).set_body_string("not JSON"))
                .expect(1)
                .mount(&server)
                .await;
            let error = TfeClient::test_client(&server.uri())
                .get_newest_runs_for_workspace("ws-abc", 1, &RetryPolicy::instant())
                .await
                .unwrap_err();
            if status != 200 {
                assert!(matches!(error, TfeError::Api { status: actual, .. } if actual == status));
            } else {
                assert!(matches!(error, TfeError::Http(_)));
            }
            assert_eq!(server.received_requests().await.unwrap().len(), 1);
        }
    }

    #[tokio::test]
    async fn bulk_phase_metadata_missing_url_and_errors_for_both_phases() {
        for apply in [false, true] {
            for (status, attributes) in [
                (404, serde_json::json!({})),
                (200, serde_json::json!({"status":"finished"})),
                (
                    200,
                    serde_json::json!({"status":"finished","log-read-url":null}),
                ),
                (401, serde_json::json!({})),
                (500, serde_json::json!({})),
                (
                    200,
                    serde_json::json!({"status":"finished","log-read-url":42}),
                ),
            ] {
                let server = MockServer::start().await;
                let phase = if apply { "apply" } else { "plan" };
                Mock::given(method("GET"))
                    .and(path(format!("/runs/run-abc/{phase}")))
                    .respond_with(
                        ResponseTemplate::new(status).set_body_json(serde_json::json!({
                            "data":{"id":format!("{phase}-abc"),"attributes":attributes}
                        })),
                    )
                    .expect(1)
                    .mount(&server)
                    .await;
                let result = TfeClient::test_client(&server.uri())
                    .get_phase_log_url("run-abc", apply, &RetryPolicy::instant())
                    .await;
                if status == 404 || (status == 200 && !attributes["log-read-url"].is_number()) {
                    assert!(result.unwrap().is_none());
                } else {
                    assert!(result.is_err());
                }
                assert_eq!(server.received_requests().await.unwrap().len(), 1);
            }
        }
    }

    #[tokio::test]
    async fn bulk_unrelated_or_invalid_includes_use_phase_metadata() {
        for related in [
            serde_json::json!({"type":"applies","id":"plan-abc","attributes":{"log-read-url":null}}),
            serde_json::json!({"type":"plans","id":"plan-other","attributes":{"log-read-url":null}}),
            serde_json::json!({"type":"plans","id":"plan-abc","attributes":{}}),
            serde_json::json!({"type":"plans","id":"plan-abc","attributes":{"log-read-url":42}}),
        ] {
            let server = MockServer::start().await;
            let run: Run = serde_json::from_value(serde_json::json!({
                "id":"run-abc","attributes":{"status":"applied"},
                "relationships":{"plan":{"data":{"id":"plan-abc"}}}
            }))
            .unwrap();
            Mock::given(method("GET")).and(path("/runs/run-abc/plan"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data":{"id":"plan-abc","attributes":{"status":"finished","log-read-url":format!("{}/blob",server.uri())}}
                }))).expect(1).mount(&server).await;
            Mock::given(method("GET"))
                .and(path("/blob"))
                .respond_with(ResponseTemplate::new(200).set_body_string("content"))
                .expect(1)
                .mount(&server)
                .await;
            assert_eq!(
                TfeClient::test_client(&server.uri())
                    .fetch_bulk_phase(
                        &run,
                        false,
                        &[related],
                        Instant::now(),
                        &RetryPolicy::instant()
                    )
                    .await
                    .unwrap(),
                "content"
            );
        }
    }

    #[tokio::test]
    async fn bulk_blob_rate_wait_budget_fails_without_metadata_refresh() {
        let server = MockServer::start().await;
        let run: Run = serde_json::from_value(serde_json::json!({
            "id":"run-abc","attributes":{"status":"applied"},
            "relationships":{"apply":{"data":{"id":"apply-abc"}}}
        }))
        .unwrap();
        let included = vec![serde_json::json!({"type":"applies","id":"apply-abc",
            "attributes":{"log-read-url":format!("{}/blob?secret=do-not-report",server.uri())}})];
        Mock::given(method("GET"))
            .and(path("/blob"))
            .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "601"))
            .expect(1)
            .mount(&server)
            .await;
        let error = TfeClient::test_client(&server.uri())
            .fetch_bulk_phase(
                &run,
                true,
                &included,
                Instant::now(),
                &RetryPolicy::instant(),
            )
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("budget exhausted"));
        assert!(!error.contains("do-not-report"));
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn bulk_blob_transport_error_redacts_signed_url() {
        let server = MockServer::start().await;
        let unavailable = "http://[invalid]";
        let run: Run = serde_json::from_value(serde_json::json!({
            "id":"run-abc","attributes":{"status":"applied"},
            "relationships":{"plan":{"data":{"id":"plan-abc"}}}
        }))
        .unwrap();
        let included = vec![serde_json::json!({"type":"plans","id":"plan-abc",
            "attributes":{"log-read-url":format!("{unavailable}/blob?secret=do-not-report")}})];
        let error = TfeClient::test_client(&server.uri())
            .fetch_bulk_phase(
                &run,
                false,
                &included,
                Instant::now(),
                &RetryPolicy::instant(),
            )
            .await
            .unwrap_err()
            .to_string();
        assert!(!error.contains("do-not-report"));
        assert!(!error.contains(unavailable));
    }

    #[tokio::test]
    async fn bulk_include_probe_preserves_operations_and_unstable_history_fails() {
        for status in [400, 422] {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(query_param("include", "plan,apply"))
                .respond_with(ResponseTemplate::new(status))
                .expect(1)
                .mount(&server)
                .await;
            Mock::given(method("GET")).and(query_param("filter[operation]",BULK_RUN_OPERATIONS))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data":[],"meta":{"pagination":{"current-page":1,"total-pages":1,"total-count":0}}
                }))).expect(1).mount(&server).await;
            let client = TfeClient::test_client(&server.uri());
            assert!(client
                .get_newest_runs_for_workspace("ws-abc", 3, &RetryPolicy::instant())
                .await
                .unwrap()
                .is_empty());
            server.reset().await;
            Mock::given(method("GET"))
                .respond_with(
                    ResponseTemplate::new(status).set_body_string("unsupported action_only"),
                )
                .expect(2)
                .mount(&server)
                .await;
            assert!(client
                .get_newest_runs_for_workspace("ws-abc", 3, &RetryPolicy::instant())
                .await
                .unwrap_err()
                .to_string()
                .contains("unsupported action_only"));
        }
    }

    #[tokio::test]
    async fn bulk_null_missing_includes_and_expired_url_refresh() {
        for first_status in [403, 429] {
            let server = MockServer::start().await;
            let run:Run=serde_json::from_value(serde_json::json!({"id":"run-abc","attributes":{"status":"errored","created-at":"2026-01-01T00:00:00Z"},
                "relationships":{"plan":{"data":{"id":"plan-abc"}}}})).unwrap();
            let included = vec![
                serde_json::json!({"id":"plan-abc","type":"plans","attributes":{"log-read-url":format!("{}/old",server.uri())}}),
            ];
            Mock::given(method("GET"))
                .and(path("/old"))
                .respond_with(ResponseTemplate::new(first_status))
                .expect(1)
                .mount(&server)
                .await;
            Mock::given(method("GET")).and(path("/runs/run-abc/plan"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"data":{"id":"plan-abc","attributes":{"status":"finished","log-read-url":format!("{}/new",server.uri())}}})))
                .expect(1).mount(&server).await;
            Mock::given(method("GET"))
                .and(path("/new"))
                .respond_with(ResponseTemplate::new(200).set_body_string("content"))
                .expect(1)
                .mount(&server)
                .await;
            let client = TfeClient::test_client(&server.uri());
            assert_eq!(
                client
                    .fetch_bulk_phase(
                        &run,
                        false,
                        &included,
                        Instant::now(),
                        &RetryPolicy::instant()
                    )
                    .await
                    .unwrap(),
                "content"
            );
            server.reset().await;
            let null = vec![
                serde_json::json!({"id":"plan-abc","type":"plans","attributes":{"log-read-url":null}}),
            ];
            Mock::given(method("GET"))
                .and(path("/runs/run-abc/plan"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data":{"id":"plan-abc","attributes":{"status":"pending","log-read-url":null}}
                })))
                .expect(1)
                .mount(&server)
                .await;
            assert!(client
                .fetch_bulk_phase(&run, false, &null, Instant::now(), &RetryPolicy::instant())
                .await
                .unwrap_err()
                .to_string()
                .contains("No plan log"));
            assert_eq!(server.received_requests().await.unwrap().len(), 1);
            server.reset().await;
            Mock::given(method("GET"))
                .and(path("/runs/run-abc/plan"))
                .respond_with(ResponseTemplate::new(404))
                .expect(1)
                .mount(&server)
                .await;
            assert!(client
                .fetch_bulk_phase(&run, false, &[], Instant::now(), &RetryPolicy::instant())
                .await
                .is_err());
        }
    }

    #[tokio::test]
    async fn bulk_stale_include_refreshes_without_reading_old_blob() {
        let server = MockServer::start().await;
        let run:Run=serde_json::from_value(serde_json::json!({"id":"run-abc","attributes":{"status":"applied"},"relationships":{"apply":{"data":{"id":"apply-abc"}}}})).unwrap();
        let included = vec![
            serde_json::json!({"id":"apply-abc","type":"applies","attributes":{"log-read-url":format!("{}/old",server.uri())}}),
        ];
        Mock::given(method("GET"))
            .and(path("/old"))
            .respond_with(ResponseTemplate::new(500))
            .expect(0)
            .mount(&server)
            .await;
        Mock::given(method("GET")).and(path("/runs/run-abc/apply"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"data":{"id":"apply-abc","attributes":{"status":"finished","log-read-url":format!("{}/new",server.uri())}}})))
            .expect(1).mount(&server).await;
        Mock::given(method("GET"))
            .and(path("/new"))
            .respond_with(ResponseTemplate::new(200).set_body_string("apply"))
            .expect(1)
            .mount(&server)
            .await;
        assert_eq!(
            TfeClient::test_client(&server.uri())
                .fetch_bulk_phase(
                    &run,
                    true,
                    &included,
                    Instant::now() - Duration::from_secs(31),
                    &RetryPolicy::instant()
                )
                .await
                .unwrap(),
            "apply"
        );
    }

    #[tokio::test]
    async fn single_log_empty_body_stays_success_and_status_errors_stay_errors() {
        let server = MockServer::start().await;
        let client = TfeClient::test_client(&server.uri());
        for status in [200, 404, 500] {
            server.reset().await;
            Mock::given(method("GET"))
                .respond_with(ResponseTemplate::new(status))
                .expect(1)
                .mount(&server)
                .await;
            let result = client.get_log_content(&server.uri()).await;
            if status == 200 {
                assert_eq!(result.unwrap(), "");
            } else {
                assert!(result.is_err());
            }
        }
    }

    #[tokio::test]
    async fn bulk_persistent_blob_rate_limits_and_second_forbidden_are_bounded() {
        for status in [429, 403] {
            let server = MockServer::start().await;
            let run: Run = serde_json::from_value(
                serde_json::json!({"id":"run-abc","attributes":{"status":"applied"}}),
            )
            .unwrap();
            let attempts = if status == 429 { 21 } else { 2 };
            Mock::given(method("GET")).and(path("/runs/run-abc/plan"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"data":{"id":"plan-abc","attributes":{"status":"finished","log-read-url":format!("{}/blob",server.uri())}}})))
                .expect(attempts).mount(&server).await;
            Mock::given(method("GET"))
                .and(path("/blob"))
                .respond_with(ResponseTemplate::new(status))
                .expect(attempts)
                .mount(&server)
                .await;
            let error = TfeClient::test_client(&server.uri())
                .fetch_bulk_phase(&run, false, &[], Instant::now(), &RetryPolicy::instant())
                .await
                .unwrap_err();
            assert!(error.to_string().contains(if status == 429 {
                "budget exhausted"
            } else {
                "403"
            }));
        }
    }

    #[tokio::test]
    async fn bulk_duplicate_history_is_deduplicated() {
        let server = MockServer::start().await;
        let value = serde_json::json!({"id":"run-abc","attributes":{"status":"canceled","created-at":"2026-01-01T00:00:00Z"}});
        Mock::given(method("GET")).respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data":[value.clone(),value],"meta":{"pagination":{"current-page":1,"total-pages":1,"total-count":2}}
        }))).expect(1).mount(&server).await;
        let runs = TfeClient::test_client(&server.uri())
            .get_newest_runs_for_workspace("ws-abc", 5, &RetryPolicy::instant())
            .await
            .unwrap();
        assert_eq!(runs.len(), 1);
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
        let client = TfeClient::test_client(&mock_server.uri());

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
        let client = TfeClient::test_client(&mock_server.uri());

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
        let client = TfeClient::test_client(&mock_server.uri());

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
        let client = TfeClient::test_client(&mock_server.uri());

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
        let client = TfeClient::test_client(&mock_server.uri());

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
        let client = TfeClient::test_client(&mock_server.uri());

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
        let client = TfeClient::test_client(&mock_server.uri());

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
        let client = TfeClient::test_client(&mock_server.uri());

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
        let client = TfeClient::test_client(&mock_server.uri());

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
        let client = TfeClient::test_client(&mock_server.uri());

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
        let client = TfeClient::test_client(&mock_server.uri());

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

        let client = TfeClient::test_client(&mock_server.uri());
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

        let client = TfeClient::test_client(&mock_server.uri());
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

        let client = TfeClient::test_client(&mock_server.uri());
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

        let client = TfeClient::test_client(&mock_server.uri());
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

        let client = TfeClient::test_client(&mock_server.uri());
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

        let client = TfeClient::test_client(&mock_server.uri());
        let result = client.cancel_run("run-notfound").await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_runs_with_pending_query_sends_status_filter() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/runs"))
            .and(query_param("filter[status]", "pending"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {
                        "id": "run-pend1",
                        "type": "runs",
                        "attributes": {"status": "pending"},
                        "relationships": {
                            "workspace": {"data": {"id": "ws-aaa", "type": "workspaces"}}
                        }
                    },
                    {
                        "id": "run-pend2",
                        "type": "runs",
                        "attributes": {"status": "pending"},
                        "relationships": {
                            "workspace": {"data": {"id": "ws-aaa", "type": "workspaces"}}
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
            })))
            .mount(&mock_server)
            .await;

        let query = RunQuery::pending();
        let result = client
            .get_runs_for_organization("my-org", query, None)
            .await;

        assert!(result.is_ok());
        let runs = result.unwrap();
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].status(), "pending");
        assert_eq!(runs[0].workspace_id(), Some("ws-aaa"));
    }

    #[tokio::test]
    async fn test_get_runs_with_pending_query_for_workspace() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-test456/runs"))
            .and(query_param("filter[status]", "pending"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {
                        "id": "run-pend3",
                        "type": "runs",
                        "attributes": {"status": "pending"},
                        "relationships": {
                            "workspace": {"data": {"id": "ws-test456", "type": "workspaces"}}
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
            })))
            .mount(&mock_server)
            .await;

        let query = RunQuery::pending();
        let result = client
            .get_runs_for_workspace("ws-test456", query, None)
            .await;

        assert!(result.is_ok());
        let runs = result.unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].id, "run-pend3");
    }

    #[tokio::test]
    async fn test_get_runs_pending_empty_response() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/runs"))
            .and(query_param("filter[status]", "pending"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [],
                "meta": {
                    "pagination": {
                        "current-page": 1,
                        "page-size": 20,
                        "next-page": null,
                        "prev-page": null
                    }
                }
            })))
            .mount(&mock_server)
            .await;

        let query = RunQuery::pending();
        let result = client
            .get_runs_for_organization("my-org", query, None)
            .await;

        assert!(result.is_ok());
        let runs = result.unwrap();
        assert!(runs.is_empty());
    }
}
