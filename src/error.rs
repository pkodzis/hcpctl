use std::fmt;

/// Custom error type for TFE operations
#[derive(Debug)]
pub enum TfeError {
    /// HTTP request failed
    Http(reqwest::Error),
    /// API returned an error response
    Api { status: u16, message: String },
    /// Token not found in any source
    TokenNotFound(String),
    /// Host not found in any source
    HostNotFound(String),
    /// Failed to read or parse credentials file
    Credentials(String),
    /// JSON parsing error
    Json(String),
    /// Configuration error
    Config(String),
}

impl fmt::Display for TfeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TfeError::Http(e) => write!(f, "HTTP request failed: {}", e),
            TfeError::Api { status, message } => {
                write!(f, "API error (status {}): {}", status, message)
            }
            TfeError::TokenNotFound(msg) => write!(f, "{}", msg),
            TfeError::HostNotFound(msg) => write!(f, "{}", msg),
            TfeError::Credentials(msg) => write!(f, "{}", msg),
            TfeError::Json(msg) => write!(f, "JSON error: {}", msg),
            TfeError::Config(msg) => write!(f, "Configuration error: {}", msg),
        }
    }
}

impl std::error::Error for TfeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TfeError::Http(e) => Some(e),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for TfeError {
    fn from(err: reqwest::Error) -> Self {
        TfeError::Http(err)
    }
}

impl From<serde_json::Error> for TfeError {
    fn from(err: serde_json::Error) -> Self {
        TfeError::Json(err.to_string())
    }
}

impl From<std::io::Error> for TfeError {
    fn from(err: std::io::Error) -> Self {
        TfeError::Credentials(err.to_string())
    }
}

impl From<std::env::VarError> for TfeError {
    fn from(err: std::env::VarError) -> Self {
        TfeError::Config(err.to_string())
    }
}

/// Result type alias for TFE operations
pub type Result<T> = std::result::Result<T, TfeError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = TfeError::TokenNotFound("test host".to_string());
        assert!(err.to_string().contains("test host"));
    }

    #[test]
    fn test_api_error_display() {
        let err = TfeError::Api {
            status: 404,
            message: "Not found".to_string(),
        };
        assert!(err.to_string().contains("404"));
        assert!(err.to_string().contains("Not found"));
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        // Verify TfeError is Send + Sync for async usage
        assert_send_sync::<TfeError>();
    }
}
