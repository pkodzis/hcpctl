//! TFE HTTP client for API interactions

use futures::stream::{self, StreamExt};
use log::debug;
use reqwest::Client;
use serde::de::DeserializeOwned;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::Duration;
use tokio::sync::{watch, Mutex};
use tokio::time::Instant;

use crate::config::api;
use crate::error::{Result, TfeError};
use crate::hcp::traits::PaginatedResponse;

/// A single fetched page: (page number, items, JSON:API `included` resources)
type PageData<T> = (u32, Vec<T>, Vec<serde_json::Value>);

/// Pagination info returned from first page fetch
#[derive(Debug, Clone)]
pub struct PaginationInfo {
    /// Total number of items across all pages
    pub total_count: u32,
    /// Total number of pages
    pub total_pages: u32,
}

/// Retry behaviour for bulk operations (rate limits and transient server errors)
///
/// Existing single-request commands do not use a retry policy; only bulk
/// log download opts in through `send_with_retry`.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    coordination: Arc<Mutex<RateState>>,
    diagnostics: Arc<StdMutex<Vec<String>>>,
    cancellation: watch::Sender<bool>,
    api_interval: Duration,
    /// Retries for 502/503/504 responses
    pub max_server_retries: u32,
    /// First backoff delay for server errors (doubles each retry)
    pub backoff_base: Duration,
    /// Upper bound for server-error backoff
    pub backoff_cap: Duration,
    /// Retries for 429 responses
    pub max_rate_retries: u32,
    /// Wait used when a 429 carries no usable header
    pub default_rate_wait: Duration,
    /// Lower bound for any 429 wait
    pub min_rate_wait: Duration,
    /// Cumulative 429 wait allowed per request
    pub max_total_rate_wait: Duration,
    /// Minimum interval between pages of the (rate-limited) runs list endpoint
    pub page_interval: Duration,
}

#[derive(Debug, Default)]
struct RateState {
    gate: Option<Instant>,
    api_slot: Option<Instant>,
    history_slot: Option<Instant>,
}

impl RetryPolicy {
    /// Production values for bulk operations
    pub fn bulk() -> Self {
        Self {
            coordination: Arc::new(Mutex::new(RateState::default())),
            diagnostics: Arc::new(StdMutex::new(Vec::new())),
            cancellation: watch::channel(false).0,
            api_interval: Duration::from_millis(50),
            max_server_retries: 5,
            backoff_base: Duration::from_secs(1),
            backoff_cap: Duration::from_secs(30),
            max_rate_retries: 20,
            default_rate_wait: Duration::from_secs(60),
            min_rate_wait: Duration::from_secs(1),
            max_total_rate_wait: Duration::from_secs(600),
            page_interval: Duration::from_secs(2),
        }
    }

    /// Policy that never retries
    pub fn none() -> Self {
        Self {
            max_server_retries: 0,
            max_rate_retries: 0,
            api_interval: Duration::ZERO,
            page_interval: Duration::ZERO,
            ..Self::bulk()
        }
    }

    /// Policy with all waits set to zero (tests)
    #[cfg(test)]
    pub fn instant() -> Self {
        Self {
            backoff_base: Duration::ZERO,
            backoff_cap: Duration::ZERO,
            default_rate_wait: Duration::ZERO,
            min_rate_wait: Duration::ZERO,
            page_interval: Duration::ZERO,
            api_interval: Duration::ZERO,
            ..Self::bulk()
        }
    }

    /// Compute the wait for a 429 response: `Retry-After` (integer seconds),
    /// then `x-ratelimit-reset` (non-negative decimal seconds, rounded up),
    /// then the default. Never below `min_rate_wait`.
    pub fn rate_wait(&self, headers: &reqwest::header::HeaderMap) -> Duration {
        let header = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());

        let retry_after = header("retry-after").and_then(|v| v.trim().parse::<u64>().ok());
        let reset = header("x-ratelimit-reset")
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v >= 0.0 && *v < u64::MAX as f64)
            .map(|v| v.ceil() as u64);

        let wait = match retry_after.or(reset) {
            Some(secs) => Duration::from_secs(secs),
            None => self.default_rate_wait,
        };
        wait.max(self.min_rate_wait)
    }

    fn server_backoff(&self, attempt: u32) -> Duration {
        let factor = 1u32.checked_shl(attempt).unwrap_or(u32::MAX);
        self.backoff_base
            .checked_mul(factor)
            .unwrap_or(self.backoff_cap)
            .min(self.backoff_cap)
    }

    /// Coordinate actual sends under one lock, rechecking the gate after waits.
    async fn acquire(&self, history: bool, blob: bool) -> Result<()> {
        loop {
            self.check_cancelled()?;
            let mut state = self.coordination.lock().await;
            let now = Instant::now();
            let mut deadline = state.gate.unwrap_or(now);
            if !blob {
                deadline = deadline.max(state.api_slot.unwrap_or(now));
                if history {
                    deadline = deadline.max(state.history_slot.unwrap_or(now));
                }
            }
            if deadline > now {
                drop(state);
                self.sleep(deadline.saturating_duration_since(now)).await?;
                continue;
            }
            if !blob {
                state.api_slot = Some(now + self.api_interval);
                if history {
                    state.history_slot = Some(now + self.page_interval);
                }
            }
            return Ok(());
        }
    }

    pub(crate) fn diagnostic(&self, message: String) {
        if let Ok(mut messages) = self.diagnostics.lock() {
            messages.push(message);
        }
    }

    pub(crate) fn take_diagnostics(&self) -> Vec<String> {
        self.diagnostics
            .lock()
            .map(|mut messages| std::mem::take(&mut *messages))
            .unwrap_or_default()
    }

    pub(crate) fn cancel(&self) {
        self.cancellation.send_replace(true);
    }

    pub(crate) fn check_cancelled(&self) -> Result<()> {
        if *self.cancellation.borrow() {
            Err(TfeError::UserCancelled)
        } else {
            Ok(())
        }
    }

    pub(crate) async fn cancelled(&self) {
        let mut receiver = self.cancellation.subscribe();
        let _ = receiver.wait_for(|cancelled| *cancelled).await;
    }

    async fn sleep(&self, wait: Duration) -> Result<()> {
        tokio::select! {
            _ = self.cancelled() => Err(TfeError::UserCancelled),
            _ = tokio::time::sleep(wait) => Ok(()),
        }
    }

    pub(crate) async fn pause(&self, wait: Duration) -> Result<()> {
        let mut state = self.coordination.lock().await;
        let until = Instant::now().checked_add(wait);
        state.gate = state.gate.max(until);
        drop(state);
        self.acquire(false, true).await
    }

    pub(crate) async fn wait_for_gate(&self) -> Result<()> {
        self.acquire(false, true).await
    }
}

/// Send a request, retrying on 429 and 502/503/504 according to `policy`.
///
/// `make_request` builds a fresh request for every attempt. Any other status
/// (including 4xx other than 429) is returned to the caller unchanged.
pub(crate) async fn send_with_retry<F>(
    policy: &RetryPolicy,
    make_request: F,
) -> Result<reqwest::Response>
where
    F: Fn() -> reqwest::RequestBuilder,
{
    send_coordinated(policy, make_request, false, false).await
}

pub(crate) async fn send_coordinated<F>(
    policy: &RetryPolicy,
    make_request: F,
    history: bool,
    blob: bool,
) -> Result<reqwest::Response>
where
    F: Fn() -> reqwest::RequestBuilder,
{
    let mut rate_retries = 0u32;
    let mut server_retries = 0u32;
    let mut cumulative_wait = Duration::ZERO;

    loop {
        policy.acquire(history, blob).await?;
        let builder = make_request();
        let target = if log::log_enabled!(log::Level::Debug) {
            builder.try_clone().and_then(|b| b.build().ok()).map(|r| {
                let mut url = r.url().clone();
                if blob {
                    // Signed blob URLs carry credentials in the query
                    url.set_query(None);
                }
                format!("{} {}", r.method(), url)
            })
        } else {
            None
        };
        if let Some(target) = &target {
            debug!("Request: {}", target);
        }
        let response = tokio::select! {
            _ = policy.cancelled() => return Err(TfeError::UserCancelled),
            response = builder.send() => response?,
        };
        if let Some(target) = &target {
            debug!("Response {}: {}", response.status().as_u16(), target);
        }
        match response.status().as_u16() {
            429 if blob => return Ok(response),
            429 => {
                if rate_retries >= policy.max_rate_retries {
                    return Err(TfeError::Api {
                        status: 429,
                        message: format!(
                            "rate limited: gave up after {} retries",
                            policy.max_rate_retries
                        ),
                    });
                }
                let wait = policy.rate_wait(response.headers());
                if Instant::now().checked_add(wait).is_none() {
                    return Err(TfeError::Config(
                        "Server rate wait exceeds supported clock range".into(),
                    ));
                }
                if wait > policy.max_total_rate_wait.saturating_sub(cumulative_wait) {
                    return Err(TfeError::Api {
                        status: 429,
                        message: format!(
                            "rate limited: required wait of {}s exceeds the retry budget",
                            wait.as_secs()
                        ),
                    });
                }
                policy.diagnostic(format!(
                    "Rate limited by server, waiting {}s...",
                    wait.as_secs()
                ));
                policy.pause(wait).await?;
                cumulative_wait += wait;
                rate_retries += 1;
            }
            502..=504 if server_retries < policy.max_server_retries => {
                let wait = policy.server_backoff(server_retries);
                policy.diagnostic(format!(
                    "Transient server error {}, waiting {}s before retry",
                    response.status(),
                    wait.as_secs()
                ));
                policy.sleep(wait).await?;
                server_retries += 1;
            }
            _ => return Ok(response),
        }
    }
}

/// TFE API client
#[derive(Clone)]
pub struct TfeClient {
    request_policy: Option<RetryPolicy>,
    client: Client,
    token: String,
    host: String,
    /// Custom base URL override (for testing with mock servers)
    base_url_override: Option<String>,
    /// Batch mode - disables interactive prompts
    batch_mode: bool,
    /// Default organization from active context
    context_org: Option<String>,
}

impl TfeClient {
    /// Opt in to bulk retries and shared coordination without changing other commands.
    pub(crate) fn with_request_policy(&self, policy: RetryPolicy) -> Self {
        let mut client = self.clone();
        client.request_policy = Some(policy);
        client
    }

    pub(crate) async fn send_get(&self, url: &str) -> Result<reqwest::Response> {
        match &self.request_policy {
            Some(policy) => send_with_retry(policy, || self.get(url)).await,
            None => Ok(self.get(url).send().await?),
        }
    }

    /// Sequential, paced variant of fetch_all_pages for bulk history.
    ///
    /// Stops as soon as `enough` reports that the items read so far suffice,
    /// so only the needed leading pages are requested. Pagination metadata is
    /// not required to be stable between pages: a live history shifts as runs
    /// are created, and callers deduplicate.
    pub(crate) async fn fetch_all_pages_bulk<T, R>(
        &self,
        path: &str,
        page_size: usize,
        policy: &RetryPolicy,
        enough: &dyn Fn(&[T]) -> bool,
    ) -> Result<(Vec<T>, Vec<serde_json::Value>)>
    where
        R: DeserializeOwned + PaginatedResponse<T>,
    {
        let mut items = Vec::new();
        let mut included = Vec::new();
        let mut page = 1;
        loop {
            let separator = if path.contains('?') { "&" } else { "?" };
            let url = format!(
                "{}{}{}page[size]={}&page[number]={}",
                self.base_url(),
                path,
                separator,
                page_size,
                page
            );
            let response = send_coordinated(policy, || self.get(&url), true, false).await?;
            if !response.status().is_success() {
                return Err(TfeError::Api {
                    status: response.status().as_u16(),
                    message: response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Could not read API error".into()),
                });
            }
            let mut parsed: R = response.json().await?;
            let total_pages = parsed
                .meta()
                .and_then(|m| m.pagination.as_ref())
                .map(|p| p.total_pages)
                .unwrap_or(1);
            included.extend(parsed.take_included());
            let data = parsed.into_data();
            let empty = data.is_empty();
            items.extend(data);
            if empty || page >= total_pages.max(1) || enough(&items) {
                return Ok((items, included));
            }
            page += 1;
        }
    }

    /// Create a new TFE client with optimized connection settings
    pub fn new(token: String, host: String) -> Self {
        let client = Client::builder()
            // Connection pool settings - reuse connections
            .pool_max_idle_per_host(20)
            .pool_idle_timeout(Duration::from_secs(90))
            // TCP keepalive to maintain connections
            .tcp_keepalive(Duration::from_secs(60))
            // Timeouts
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            request_policy: None,
            client,
            token,
            host,
            base_url_override: None,
            batch_mode: false,
            context_org: None,
        }
    }

    /// Create a client with custom base URL (for testing with mock servers)
    #[cfg(test)]
    pub fn with_base_url(token: String, host: String, base_url: String) -> Self {
        let client = Client::builder().build().unwrap_or_else(|_| Client::new());

        Self {
            request_policy: None,
            client,
            token,
            host,
            base_url_override: Some(base_url),
            batch_mode: false,
            context_org: None,
        }
    }

    /// Set batch mode (disables interactive prompts for large result sets)
    pub fn set_batch_mode(&mut self, batch: bool) {
        self.batch_mode = batch;
    }

    /// Check if batch mode is enabled
    pub fn is_batch_mode(&self) -> bool {
        self.batch_mode
    }

    /// Set the default organization from active context
    pub fn set_context_org(&mut self, org: Option<String>) {
        self.context_org = org;
    }

    /// Resolve org: explicit CLI value wins, then context default
    pub fn effective_org(&self, explicit: Option<&String>) -> Option<String> {
        explicit.cloned().or_else(|| self.context_org.clone())
    }

    /// Build the base URL for API requests
    pub(crate) fn base_url(&self) -> String {
        if let Some(ref url) = self.base_url_override {
            return url.clone();
        }
        format!(
            "https://{}/{}",
            self.host,
            api::BASE_PATH.trim_start_matches('/')
        )
    }

    /// Get the host for building URLs
    pub(crate) fn host(&self) -> &str {
        &self.host
    }

    /// Add standard headers to a request builder
    fn with_headers(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        builder
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Content-Type", "application/vnd.api+json")
    }

    /// Create a GET request builder with standard headers
    pub(crate) fn get(&self, url: &str) -> reqwest::RequestBuilder {
        self.with_headers(self.client.get(url))
    }

    /// Create a POST request builder with standard headers
    #[allow(dead_code)]
    pub(crate) fn post(&self, url: &str) -> reqwest::RequestBuilder {
        self.with_headers(self.client.post(url))
    }

    /// Create a PATCH request builder with standard headers
    pub(crate) fn patch(&self, url: &str) -> reqwest::RequestBuilder {
        self.with_headers(self.client.patch(url))
    }

    /// Create a DELETE request builder with standard headers
    #[allow(dead_code)]
    pub(crate) fn delete(&self, url: &str) -> reqwest::RequestBuilder {
        self.with_headers(self.client.delete(url))
    }

    /// Parse an API response, returning error for non-success status codes
    ///
    /// Simplifies the common pattern of checking status and parsing JSON.
    pub(crate) async fn parse_api_response<T>(
        &self,
        response: reqwest::Response,
        error_context: &str,
    ) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        if !response.status().is_success() {
            return Err(TfeError::Api {
                status: response.status().as_u16(),
                message: format!("Failed to fetch {}", error_context),
            });
        }
        Ok(response.json().await?)
    }

    /// Fetch all pages from a paginated API endpoint with parallel fetching
    ///
    /// This method fetches page 1 first to learn total_pages, then fetches
    /// remaining pages in parallel (with concurrency limit).
    ///
    /// **Note**: This method does NOT check for large result sets. If you need
    /// to warn users about large operations, use `prefetch_pagination_info` first
    /// to get the scale, then call this method after confirmation.
    ///
    /// # Arguments
    /// * `path` - API path (e.g., "/organizations/my-org/teams" or with query "...?search=foo")
    /// * `error_context` - Context for error messages (e.g., "teams for organization 'my-org'")
    ///
    /// # Type Parameters
    /// * `T` - The item type (e.g., Team, Workspace)
    /// * `R` - The response type that implements PaginatedResponse<T>
    pub async fn fetch_all_pages<T, R>(&self, path: &str, error_context: &str) -> Result<Vec<T>>
    where
        T: Send,
        R: DeserializeOwned + PaginatedResponse<T> + Send,
    {
        let (items, _included) = self
            .fetch_all_pages_internal::<T, R>(path, error_context)
            .await?;
        Ok(items)
    }

    /// Fetch all pages and also return the aggregated JSON:API `included` resources.
    ///
    /// Use this with an `include=` query parameter to pull related resources (e.g.
    /// `include=current_run`) in the same paginated request set — no per-item calls.
    pub async fn fetch_all_pages_with_included<T, R>(
        &self,
        path: &str,
        error_context: &str,
    ) -> Result<(Vec<T>, Vec<serde_json::Value>)>
    where
        T: Send,
        R: DeserializeOwned + PaginatedResponse<T> + Send,
    {
        self.fetch_all_pages_internal::<T, R>(path, error_context)
            .await
    }

    /// Prefetch pagination info without fetching all data
    ///
    /// This method fetches only the first page to get pagination metadata.
    /// Use this to check the scale of an operation before committing to fetch all pages.
    ///
    /// Returns `None` if there's no pagination info (single page or no results).
    ///
    /// # Type Parameters
    /// * `T` - The item type (must match what you'll use in fetch_all_pages)
    /// * `R` - The response type that implements PaginatedResponse<T>
    pub async fn prefetch_pagination_info<T, R>(
        &self,
        path: &str,
        error_context: &str,
    ) -> Result<Option<PaginationInfo>>
    where
        T: Send,
        R: DeserializeOwned + PaginatedResponse<T> + Send,
    {
        let separator = if path.contains('?') { "&" } else { "?" };

        let first_page_url = format!(
            "{}{}{}page[size]={}&page[number]=1",
            self.base_url(),
            path,
            separator,
            api::DEFAULT_PAGE_SIZE,
        );

        debug!("Prefetching pagination info from: {}", first_page_url);

        let response = self.send_get(&first_page_url).await?;

        let first_resp: R = self.parse_api_response(response, error_context).await?;

        match first_resp.meta() {
            Some(m) => match &m.pagination {
                Some(p) => Ok(Some(PaginationInfo {
                    total_count: p.total_count,
                    total_pages: p.total_pages,
                })),
                None => Ok(None),
            },
            None => Ok(None),
        }
    }

    /// Fetch a single resource by API path
    ///
    /// Generic helper that handles the common pattern of:
    /// - GET a resource by path
    /// - Parse JSON response into typed model + raw JSON
    /// - Return None for 404
    /// - Return error for other non-success status codes
    ///
    /// # Arguments
    /// * `path` - API path (e.g., "/workspaces/ws-abc123")
    /// * `resource_label` - Human-readable label for error messages (e.g., "workspace 'ws-abc123'")
    pub async fn fetch_resource_by_path<T>(
        &self,
        path: &str,
        resource_label: &str,
    ) -> Result<Option<(T, serde_json::Value)>>
    where
        T: serde::de::DeserializeOwned,
    {
        let url = format!("{}{}", self.base_url(), path);
        debug!("Fetching {} from: {}", resource_label, url);

        let response = self.send_get(&url).await?;

        match response.status().as_u16() {
            200 => {
                let raw: serde_json::Value = response.json().await?;
                let item: T =
                    serde_json::from_value(raw["data"].clone()).map_err(|e| TfeError::Api {
                        status: 200,
                        message: format!("Failed to parse {}: {}", resource_label, e),
                    })?;
                Ok(Some((item, raw)))
            }
            404 => Ok(None),
            status => Err(TfeError::Api {
                status,
                message: format!("Failed to fetch {}", resource_label),
            }),
        }
    }

    /// Internal implementation of parallel pagination
    async fn fetch_all_pages_internal<T, R>(
        &self,
        path: &str,
        error_context: &str,
    ) -> Result<(Vec<T>, Vec<serde_json::Value>)>
    where
        T: Send,
        R: DeserializeOwned + PaginatedResponse<T> + Send,
    {
        // Detect if path already has query params
        let separator = if path.contains('?') { "&" } else { "?" };

        // STEP 1: Fetch first page to get pagination info
        let first_page_url = format!(
            "{}{}{}page[size]={}&page[number]=1",
            self.base_url(),
            path,
            separator,
            api::DEFAULT_PAGE_SIZE,
        );

        debug!("Fetching page 1 from: {}", first_page_url);

        let response = self.send_get(&first_page_url).await?;

        let mut first_resp: R = self.parse_api_response(response, error_context).await?;
        let meta = first_resp.meta().cloned();
        let mut all_included = first_resp.take_included();
        let mut all_items = first_resp.into_data();

        // Extract pagination info
        let (total_pages, total_count) = match meta {
            Some(ref m) => match m.pagination {
                Some(ref p) => (p.total_pages, p.total_count),
                None => return Ok((all_items, all_included)), // No pagination info = single page
            },
            None => return Ok((all_items, all_included)), // No meta = single page
        };

        debug!("Page 1/{}, total items: {}", total_pages, total_count);

        // If only one page, we're done
        if total_pages <= 1 {
            return Ok((all_items, all_included));
        }

        // STEP 2: Fetch remaining pages in parallel
        let remaining_pages: Vec<u32> = (2..=total_pages).collect();

        debug!(
            "Fetching {} remaining pages in parallel (max {} concurrent)",
            remaining_pages.len(),
            api::MAX_CONCURRENT_PAGE_REQUESTS
        );

        // Create futures for all remaining pages
        let page_futures = remaining_pages.into_iter().map(|page_num| {
            let url = format!(
                "{}{}{}page[size]={}&page[number]={}",
                self.base_url(),
                path,
                separator,
                api::DEFAULT_PAGE_SIZE,
                page_num
            );
            self.fetch_single_page::<T, R>(url, page_num, error_context)
        });

        // Execute with concurrency limit
        let results: Vec<Result<PageData<T>>> = stream::iter(page_futures)
            .buffer_unordered(api::MAX_CONCURRENT_PAGE_REQUESTS)
            .collect()
            .await;

        // Collect results, maintaining order by page number
        let mut page_results: Vec<PageData<T>> = Vec::with_capacity(results.len());
        for result in results {
            match result {
                Ok(page) => page_results.push(page),
                Err(e) => return Err(e),
            }
        }

        // Sort by page number to maintain consistent ordering
        page_results.sort_by_key(|(page_num, _, _)| *page_num);

        // Extend all_items with results from remaining pages
        for (_, items, included) in page_results {
            all_items.extend(items);
            all_included.extend(included);
        }

        debug!(
            "Fetched {} total items for {}",
            all_items.len(),
            error_context
        );
        Ok((all_items, all_included))
    }

    /// Fetch a single page (helper for parallel pagination)
    async fn fetch_single_page<T, R>(
        &self,
        url: String,
        page_num: u32,
        error_context: &str,
    ) -> Result<PageData<T>>
    where
        R: DeserializeOwned + PaginatedResponse<T>,
    {
        debug!("Fetching page {} from: {}", page_num, url);

        let response = self.send_get(&url).await?;

        let page_context = format!("{} (page {})", error_context, page_num);
        let mut resp: R = self.parse_api_response(response, &page_context).await?;
        let included = resp.take_included();
        let items = resp.into_data();

        debug!("Page {} returned {} items", page_num, items.len());
        Ok((page_num, items, included))
    }
}

#[cfg(test)]
impl TfeClient {
    /// Create a test client with mock base URL
    ///
    /// Convenience method to replace the `create_test_client` boilerplate
    /// that was duplicated across 15+ test modules.
    pub fn test_client(base_url: &str) -> Self {
        Self::with_base_url(
            "test-token".to_string(),
            "mock.terraform.io".to_string(),
            base_url.to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn bulk_server_backoff_caps_and_handles_overflow() {
        let policy = RetryPolicy::bulk();
        for (attempt, seconds) in [
            (0, 1),
            (1, 2),
            (4, 16),
            (5, 30),
            (31, 30),
            (32, 30),
            (u32::MAX, 30),
        ] {
            assert_eq!(policy.server_backoff(attempt), Duration::from_secs(seconds));
        }
        let policy = RetryPolicy {
            backoff_base: Duration::MAX,
            ..policy
        };
        assert_eq!(policy.server_backoff(1), policy.backoff_cap);
    }

    #[tokio::test]
    async fn bulk_retry_opt_in_does_not_mutate_original_client() {
        let server = wiremock::MockServer::start().await;
        let attempts = AtomicUsize::new(0);
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(move |_: &wiremock::Request| {
                wiremock::ResponseTemplate::new(if attempts.fetch_add(1, Ordering::SeqCst) < 2 {
                    503
                } else {
                    200
                })
            })
            .expect(3)
            .mount(&server)
            .await;
        let client = TfeClient::test_client(&server.uri());
        let bulk = client.with_request_policy(RetryPolicy::instant());
        assert!(client.request_policy.is_none());
        assert!(bulk.request_policy.is_some());
        assert_eq!(client.send_get(&server.uri()).await.unwrap().status(), 503);
        assert_eq!(bulk.send_get(&server.uri()).await.unwrap().status(), 200);
    }

    #[test]
    fn bulk_rate_header_precedence_and_validation() {
        let policy = RetryPolicy::bulk();
        let mut headers = reqwest::header::HeaderMap::new();
        assert_eq!(policy.rate_wait(&headers), Duration::from_secs(60));
        for (value, expected) in [
            ("7", 7),
            ("0", 1),
            ("120", 120),
            ("18446744073709551615", u64::MAX),
        ] {
            headers.insert("retry-after", value.parse().unwrap());
            assert_eq!(policy.rate_wait(&headers), Duration::from_secs(expected));
        }
        headers.insert("retry-after", "invalid".parse().unwrap());
        headers.insert("x-ratelimit-reset", "2.3".parse().unwrap());
        assert_eq!(policy.rate_wait(&headers), Duration::from_secs(3));
        for value in ["NaN", "inf", "-1"] {
            headers.insert("x-ratelimit-reset", value.parse().unwrap());
            assert_eq!(policy.rate_wait(&headers), Duration::from_secs(60));
        }
    }

    #[tokio::test]
    async fn bulk_retry_attempt_bounds_and_no_retry_statuses() {
        for (status, attempts) in [
            (429, 21),
            (502, 6),
            (503, 6),
            (504, 6),
            (401, 1),
            (403, 1),
            (404, 1),
            (400, 1),
            (422, 1),
            (500, 1),
        ] {
            let server = wiremock::MockServer::start().await;
            wiremock::Mock::given(wiremock::matchers::method("GET"))
                .respond_with(wiremock::ResponseTemplate::new(status))
                .expect(attempts)
                .mount(&server)
                .await;
            let http = reqwest::Client::new();
            let result = send_with_retry(&RetryPolicy::instant(), || http.get(server.uri())).await;
            if status == 429 {
                assert!(result.unwrap_err().to_string().contains("20 retries"));
            } else {
                assert_eq!(result.unwrap().status().as_u16(), status);
            }
        }
    }

    #[tokio::test]
    async fn twenty_rate_limits_then_success_and_budget_failure() {
        let server = wiremock::MockServer::start().await;
        let counter = AtomicUsize::new(0);
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(move |_: &wiremock::Request| {
                wiremock::ResponseTemplate::new(if counter.fetch_add(1, Ordering::SeqCst) < 20 {
                    429
                } else {
                    200
                })
            })
            .expect(21)
            .mount(&server)
            .await;
        let http = reqwest::Client::new();
        assert_eq!(
            send_with_retry(&RetryPolicy::instant(), || http.get(server.uri()))
                .await
                .unwrap()
                .status(),
            200
        );
        server.reset().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(wiremock::ResponseTemplate::new(429).insert_header("Retry-After", "601"))
            .expect(1)
            .mount(&server)
            .await;
        assert!(
            send_with_retry(&RetryPolicy::instant(), || http.get(server.uri()))
                .await
                .unwrap_err()
                .to_string()
                .contains("budget")
        );
    }

    #[tokio::test(start_paused = true)]
    async fn bulk_gate_rechecks_extension_and_send_slots_do_not_bunch() {
        let policy = RetryPolicy::bulk();
        let start = Instant::now();
        policy.coordination.lock().await.gate = Some(start + Duration::from_secs(30));
        let waiter = tokio::spawn({
            let policy = policy.clone();
            async move {
                policy.acquire(false, false).await.unwrap();
                Instant::now()
            }
        });
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(10)).await;
        policy.coordination.lock().await.gate = Some(start + Duration::from_secs(90));
        tokio::time::advance(Duration::from_secs(20)).await;
        assert!(!waiter.is_finished());
        tokio::time::advance(Duration::from_secs(60)).await;
        assert!(waiter.await.unwrap() >= start + Duration::from_secs(90));
        let calls = (0..100).map(|_| {
            let policy = policy.clone();
            tokio::spawn(async move {
                policy.acquire(false, false).await.unwrap();
                Instant::now()
            })
        });
        let mut times = futures::future::join_all(calls)
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect::<Vec<_>>();
        times.sort();
        for pair in times.windows(2) {
            assert!(pair[1] - pair[0] >= Duration::from_millis(50));
        }
        assert!(times[99] - times[0] >= Duration::from_millis(4950));
    }

    #[tokio::test(start_paused = true)]
    async fn history_and_blob_coordination() {
        let policy = RetryPolicy::bulk();
        let start = Instant::now();
        for _ in 0..40 {
            policy.acquire(true, false).await.unwrap();
        }
        assert!(Instant::now() - start >= Duration::from_secs(78));
        let before = Instant::now();
        for _ in 0..100 {
            policy.acquire(false, true).await.unwrap();
        }
        assert_eq!(before, Instant::now());
        policy.coordination.lock().await.gate = Some(before + Duration::from_secs(90));
        policy.acquire(false, true).await.unwrap();
        assert_eq!(Instant::now() - before, Duration::from_secs(90));
    }

    #[tokio::test(start_paused = true)]
    async fn cancellation_releases_shared_rate_wait_immediately() {
        let policy = RetryPolicy::bulk();
        let start = Instant::now();
        let waiter = tokio::spawn({
            let policy = policy.clone();
            async move { policy.pause(Duration::from_secs(600)).await }
        });
        tokio::task::yield_now().await;
        policy.cancel();
        assert!(matches!(
            waiter.await.unwrap(),
            Err(TfeError::UserCancelled)
        ));
        assert_eq!(Instant::now(), start);
    }

    #[test]
    fn test_base_url() {
        let client = TfeClient::new("token".to_string(), "example.com".to_string());
        assert_eq!(client.base_url(), "https://example.com/api/v2");
    }

    #[test]
    fn test_client_creation() {
        let client = TfeClient::new("my-token".to_string(), "tfe.example.com".to_string());
        assert_eq!(client.host, "tfe.example.com");
        assert_eq!(client.token, "my-token");
        assert!(!client.batch_mode); // Default is false
    }

    #[test]
    fn test_batch_mode() {
        let mut client = TfeClient::new("token".to_string(), "example.com".to_string());
        assert!(!client.is_batch_mode());

        client.set_batch_mode(true);
        assert!(client.is_batch_mode());

        client.set_batch_mode(false);
        assert!(!client.is_batch_mode());
    }

    #[test]
    fn test_host_getter() {
        let client = TfeClient::new("token".to_string(), "custom.terraform.io".to_string());
        assert_eq!(client.host(), "custom.terraform.io");
    }

    #[test]
    fn test_base_url_with_app_terraform_io() {
        let client = TfeClient::new("token".to_string(), "app.terraform.io".to_string());
        assert_eq!(client.base_url(), "https://app.terraform.io/api/v2");
    }

    #[test]
    fn test_base_url_strips_leading_slash() {
        // Ensure base_url works correctly regardless of BASE_PATH format
        let client = TfeClient::new("token".to_string(), "test.com".to_string());
        let url = client.base_url();
        assert!(!url.contains("//api")); // No double slashes
        assert!(url.starts_with("https://"));
    }

    #[test]
    fn test_path_separator_detection() {
        // Test that fetch_all_pages correctly handles ? vs & for query params
        let path_without_query = "/organizations/my-org/teams";
        let path_with_query = "/organizations/my-org/workspaces?search[name]=foo";

        assert!(!path_without_query.contains('?'));
        assert!(path_with_query.contains('?'));
    }
}

#[cfg(test)]
mod pagination_tests {
    use super::*;
    use serde::Deserialize;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::hcp::traits::PaginatedResponse;
    use crate::hcp::PaginationMeta;

    /// Test item type
    #[derive(Deserialize, Debug, Clone)]
    struct TestItem {
        id: String,
        name: String,
    }

    /// Test response type
    #[derive(Deserialize, Debug)]
    struct TestItemsResponse {
        data: Vec<TestItem>,
        #[serde(default)]
        meta: Option<PaginationMeta>,
    }

    impl PaginatedResponse<TestItem> for TestItemsResponse {
        fn into_data(self) -> Vec<TestItem> {
            self.data
        }

        fn meta(&self) -> Option<&PaginationMeta> {
            self.meta.as_ref()
        }
    }

    fn test_item_json(id: &str, name: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "name": name
        })
    }

    #[tokio::test]
    async fn test_fetch_all_pages_single_page() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        let response_body = serde_json::json!({
            "data": [
                test_item_json("item-1", "Item 1"),
                test_item_json("item-2", "Item 2")
            ],
            "meta": {
                "pagination": {
                    "current-page": 1,
                    "total-pages": 1,
                    "total-count": 2
                }
            }
        });

        Mock::given(method("GET"))
            .and(path("/test-items"))
            .and(query_param("page[number]", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&mock_server)
            .await;

        let result = client
            .fetch_all_pages::<TestItem, TestItemsResponse>("/test-items", "test items")
            .await;

        assert!(result.is_ok());
        let items = result.unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].name, "Item 1");
        assert_eq!(items[1].name, "Item 2");
    }

    #[tokio::test]
    async fn test_fetch_all_pages_multiple_pages_parallel() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        // Page 1
        Mock::given(method("GET"))
            .and(path("/test-items"))
            .and(query_param("page[number]", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    test_item_json("item-1", "Item 1"),
                    test_item_json("item-2", "Item 2")
                ],
                "meta": {
                    "pagination": {
                        "current-page": 1,
                        "total-pages": 3,
                        "total-count": 6
                    }
                }
            })))
            .mount(&mock_server)
            .await;

        // Page 2
        Mock::given(method("GET"))
            .and(path("/test-items"))
            .and(query_param("page[number]", "2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    test_item_json("item-3", "Item 3"),
                    test_item_json("item-4", "Item 4")
                ],
                "meta": {
                    "pagination": {
                        "current-page": 2,
                        "total-pages": 3,
                        "total-count": 6
                    }
                }
            })))
            .mount(&mock_server)
            .await;

        // Page 3
        Mock::given(method("GET"))
            .and(path("/test-items"))
            .and(query_param("page[number]", "3"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    test_item_json("item-5", "Item 5"),
                    test_item_json("item-6", "Item 6")
                ],
                "meta": {
                    "pagination": {
                        "current-page": 3,
                        "total-pages": 3,
                        "total-count": 6
                    }
                }
            })))
            .mount(&mock_server)
            .await;

        let result = client
            .fetch_all_pages::<TestItem, TestItemsResponse>("/test-items", "test items")
            .await;

        assert!(result.is_ok());
        let items = result.unwrap();
        assert_eq!(items.len(), 6);

        // Verify order is maintained (page 1, then page 2, then page 3)
        assert_eq!(items[0].id, "item-1");
        assert_eq!(items[1].id, "item-2");
        assert_eq!(items[2].id, "item-3");
        assert_eq!(items[3].id, "item-4");
        assert_eq!(items[4].id, "item-5");
        assert_eq!(items[5].id, "item-6");
    }

    #[tokio::test]
    async fn test_fetch_all_pages_no_pagination_meta() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        // Response without pagination meta
        let response_body = serde_json::json!({
            "data": [
                test_item_json("item-1", "Item 1")
            ]
        });

        Mock::given(method("GET"))
            .and(path("/test-items"))
            .and(query_param("page[number]", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&mock_server)
            .await;

        let result = client
            .fetch_all_pages::<TestItem, TestItemsResponse>("/test-items", "test items")
            .await;

        assert!(result.is_ok());
        let items = result.unwrap();
        assert_eq!(items.len(), 1);
    }

    #[tokio::test]
    async fn test_fetch_all_pages_api_error_on_first_page() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/test-items"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&mock_server)
            .await;

        let result = client
            .fetch_all_pages::<TestItem, TestItemsResponse>("/test-items", "test items")
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            TfeError::Api { status, .. } => assert_eq!(status, 403),
            _ => panic!("Expected TfeError::Api"),
        }
    }

    #[tokio::test]
    async fn test_fetch_all_pages_api_error_on_subsequent_page() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        // Page 1 succeeds
        Mock::given(method("GET"))
            .and(path("/test-items"))
            .and(query_param("page[number]", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [test_item_json("item-1", "Item 1")],
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

        // Page 2 fails
        Mock::given(method("GET"))
            .and(path("/test-items"))
            .and(query_param("page[number]", "2"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let result = client
            .fetch_all_pages::<TestItem, TestItemsResponse>("/test-items", "test items")
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            TfeError::Api { status, message } => {
                assert_eq!(status, 500);
                assert!(message.contains("page 2"));
            }
            _ => panic!("Expected TfeError::Api"),
        }
    }

    #[tokio::test]
    async fn test_fetch_all_pages_with_existing_query_params() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        let response_body = serde_json::json!({
            "data": [test_item_json("item-1", "Filtered Item")],
            "meta": {
                "pagination": {
                    "current-page": 1,
                    "total-pages": 1,
                    "total-count": 1
                }
            }
        });

        // Path already has query params, so page params should use &
        Mock::given(method("GET"))
            .and(path("/test-items"))
            .and(query_param("search[name]", "test"))
            .and(query_param("page[number]", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&mock_server)
            .await;

        let result = client
            .fetch_all_pages::<TestItem, TestItemsResponse>(
                "/test-items?search[name]=test",
                "test items",
            )
            .await;

        assert!(result.is_ok());
        let items = result.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "Filtered Item");
    }

    #[tokio::test]
    async fn test_fetch_all_pages_empty_result() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        let response_body = serde_json::json!({
            "data": [],
            "meta": {
                "pagination": {
                    "current-page": 1,
                    "total-pages": 0,
                    "total-count": 0
                }
            }
        });

        Mock::given(method("GET"))
            .and(path("/test-items"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&mock_server)
            .await;

        let result = client
            .fetch_all_pages::<TestItem, TestItemsResponse>("/test-items", "test items")
            .await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
