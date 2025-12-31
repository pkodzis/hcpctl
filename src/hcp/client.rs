//! TFE HTTP client for API interactions

use reqwest::Client;
use std::time::Duration;

use crate::config::api;

/// TFE API client
pub struct TfeClient {
    client: Client,
    token: String,
    host: String,
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
        }
    }

    /// Build the base URL for API requests
    pub(crate) fn base_url(&self) -> String {
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
    #[allow(dead_code)]
    pub(crate) fn patch(&self, url: &str) -> reqwest::RequestBuilder {
        self.with_headers(self.client.patch(url))
    }

    /// Create a DELETE request builder with standard headers
    #[allow(dead_code)]
    pub(crate) fn delete(&self, url: &str) -> reqwest::RequestBuilder {
        self.with_headers(self.client.delete(url))
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
    }
}
