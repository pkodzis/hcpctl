/// Configuration constants for TFE API
pub mod api {
    /// Base path for TFE API v2
    pub const BASE_PATH: &str = "/api/v2";

    /// Organizations endpoint
    pub const ORGANIZATIONS: &str = "organizations";

    /// Workspaces endpoint
    pub const WORKSPACES: &str = "workspaces";

    /// Default page size for API requests
    pub const DEFAULT_PAGE_SIZE: u32 = 100;
}

/// Configuration constants for credentials
pub mod credentials {
    /// Credentials file name
    pub const FILE_NAME: &str = "terraform.d/credentials.tfrc.json";

    /// Path to Terraform credentials file on Unix (relative to HOME)
    pub const FILE_PATH_UNIX: &str = ".terraform.d/credentials.tfrc.json";

    /// Environment variable names for token (checked in order)
    pub const TOKEN_ENV_VARS: &[&str] = &["HCP_TOKEN", "TFC_TOKEN", "TFE_TOKEN"];
}

/// Default values for CLI
pub mod defaults {
    /// Default TFE host
    pub const HOST: &str = "app.terraform.io";

    /// Default log level
    pub const LOG_LEVEL: &str = "warn";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_base_path_format() {
        assert!(api::BASE_PATH.starts_with('/'));
    }

    #[test]
    fn test_credentials_env_vars() {
        assert_eq!(
            credentials::TOKEN_ENV_VARS,
            &["HCP_TOKEN", "TFC_TOKEN", "TFE_TOKEN"]
        );
    }

    #[test]
    fn test_default_host_is_valid() {
        assert!(defaults::HOST.contains('.'));
        assert!(!defaults::HOST.starts_with("https://"));
    }
}
