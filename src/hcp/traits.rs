//! Common traits for TFE resources

use crate::hcp::PaginationMeta;

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
}
