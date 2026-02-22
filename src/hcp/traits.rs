//! Common traits for TFE resources

use crate::hcp::PaginationMeta;
use serde::Deserialize;

/// Common trait for all TFE resources (organizations, projects, workspaces)
///
/// This trait provides a unified interface for resource identification
/// and matching, which is useful for CRUD operations.
pub trait TfeResource {
    /// Get the resource ID
    fn id(&self) -> &str;

    /// Get the human-readable name
    fn name(&self) -> &str;

    /// Check if the resource matches by name or ID
    ///
    /// Default implementation checks for exact match on either field.
    fn matches(&self, input: &str) -> bool {
        self.id() == input || self.name() == input
    }
}

/// Trait for API responses that contain paginated data
///
/// Implement this trait for any `XResponse` struct to enable use with
/// `TfeClient::fetch_all_pages()` helper.
pub trait PaginatedResponse<T> {
    /// Consume self and return the data items
    fn into_data(self) -> Vec<T>;
    /// Get reference to pagination metadata
    fn meta(&self) -> Option<&PaginationMeta>;
}

/// Generic API list response wrapper for paginated endpoints
///
/// Replaces per-resource response structs (e.g., TeamsResponse, ProjectsResponse)
/// with a single generic type that works with `fetch_all_pages`.
#[derive(Deserialize, Debug)]
pub struct ApiListResponse<T> {
    pub data: Vec<T>,
    #[serde(default)]
    pub meta: Option<crate::hcp::PaginationMeta>,
}

impl<T> PaginatedResponse<T> for ApiListResponse<T> {
    fn into_data(self) -> Vec<T> {
        self.data
    }

    fn meta(&self) -> Option<&crate::hcp::PaginationMeta> {
        self.meta.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestResource {
        id: String,
        name: String,
    }

    impl TfeResource for TestResource {
        fn id(&self) -> &str {
            &self.id
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    #[test]
    fn test_matches_by_id() {
        let resource = TestResource {
            id: "res-123".to_string(),
            name: "my-resource".to_string(),
        };
        assert!(resource.matches("res-123"));
    }

    #[test]
    fn test_matches_by_name() {
        let resource = TestResource {
            id: "res-123".to_string(),
            name: "my-resource".to_string(),
        };
        assert!(resource.matches("my-resource"));
    }

    #[test]
    fn test_no_match() {
        let resource = TestResource {
            id: "res-123".to_string(),
            name: "my-resource".to_string(),
        };
        assert!(!resource.matches("other"));
    }

    #[test]
    fn test_api_list_response_into_data() {
        let response: ApiListResponse<serde_json::Value> =
            serde_json::from_value(serde_json::json!({
                "data": [{"id": "item-1"}, {"id": "item-2"}],
                "meta": {
                    "pagination": {
                        "current-page": 1,
                        "total-pages": 1,
                        "total-count": 2
                    }
                }
            }))
            .unwrap();
        let data = response.into_data();
        assert_eq!(data.len(), 2);
    }

    #[test]
    fn test_api_list_response_meta() {
        let response: ApiListResponse<serde_json::Value> =
            serde_json::from_value(serde_json::json!({
                "data": [{"id": "item-1"}],
                "meta": {
                    "pagination": {
                        "current-page": 1,
                        "total-pages": 3,
                        "total-count": 5
                    }
                }
            }))
            .unwrap();
        let meta = response.meta().unwrap();
        let pagination = meta.pagination.as_ref().unwrap();
        assert_eq!(pagination.total_pages, 3);
        assert_eq!(pagination.total_count, 5);
    }

    #[test]
    fn test_api_list_response_without_meta() {
        let response: ApiListResponse<serde_json::Value> =
            serde_json::from_value(serde_json::json!({
                "data": [{"id": "item-1"}]
            }))
            .unwrap();
        assert!(response.meta().is_none());
        assert_eq!(response.into_data().len(), 1);
    }

    #[test]
    fn test_api_list_response_empty_data() {
        let response: ApiListResponse<serde_json::Value> =
            serde_json::from_value(serde_json::json!({
                "data": []
            }))
            .unwrap();
        assert!(response.into_data().is_empty());
    }
}
