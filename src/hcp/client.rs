//! TFE HTTP client for API interactions

use futures::stream::{self, StreamExt};
use log::debug;
use reqwest::Client;
use serde::de::DeserializeOwned;
use std::time::Duration;

use crate::config::api;
use crate::error::{Result, TfeError};
use crate::hcp::traits::PaginatedResponse;

/// Pagination info returned from first page fetch
#[derive(Debug, Clone)]
pub struct PaginationInfo {
    /// Total number of items across all pages
    pub total_count: u32,
    /// Total number of pages
    pub total_pages: u32,
}

/// TFE API client
pub struct TfeClient {
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

        let response = self.get(&first_page_url).send().await?;

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

        let response = self.get(&url).send().await?;

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
    ) -> Result<Vec<T>>
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

        let response = self.get(&first_page_url).send().await?;

        let first_resp: R = self.parse_api_response(response, error_context).await?;
        let meta = first_resp.meta().cloned();
        let mut all_items = first_resp.into_data();

        // Extract pagination info
        let (total_pages, total_count) = match meta {
            Some(ref m) => match m.pagination {
                Some(ref p) => (p.total_pages, p.total_count),
                None => return Ok(all_items), // No pagination info = single page
            },
            None => return Ok(all_items), // No meta = single page
        };

        debug!("Page 1/{}, total items: {}", total_pages, total_count);

        // If only one page, we're done
        if total_pages <= 1 {
            return Ok(all_items);
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
        let results: Vec<Result<(u32, Vec<T>)>> = stream::iter(page_futures)
            .buffer_unordered(api::MAX_CONCURRENT_PAGE_REQUESTS)
            .collect()
            .await;

        // Collect results, maintaining order by page number
        let mut page_results: Vec<(u32, Vec<T>)> = Vec::with_capacity(results.len());
        for result in results {
            match result {
                Ok((page_num, items)) => page_results.push((page_num, items)),
                Err(e) => return Err(e),
            }
        }

        // Sort by page number to maintain consistent ordering
        page_results.sort_by_key(|(page_num, _)| *page_num);

        // Extend all_items with results from remaining pages
        for (_, items) in page_results {
            all_items.extend(items);
        }

        debug!(
            "Fetched {} total items for {}",
            all_items.len(),
            error_context
        );
        Ok(all_items)
    }

    /// Fetch a single page (helper for parallel pagination)
    async fn fetch_single_page<T, R>(
        &self,
        url: String,
        page_num: u32,
        error_context: &str,
    ) -> Result<(u32, Vec<T>)>
    where
        R: DeserializeOwned + PaginatedResponse<T>,
    {
        debug!("Fetching page {} from: {}", page_num, url);

        let response = self.get(&url).send().await?;

        let page_context = format!("{} (page {})", error_context, page_num);
        let resp: R = self.parse_api_response(response, &page_context).await?;
        let items = resp.into_data();

        debug!("Page {} returned {} items", page_num, items.len());
        Ok((page_num, items))
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
