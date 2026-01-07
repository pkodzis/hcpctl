/// Configuration constants for TFE API
pub mod api {
    /// Base path for TFE API v2
    pub const BASE_PATH: &str = "/api/v2";

    /// Organizations endpoint
    pub const ORGANIZATIONS: &str = "organizations";

    /// Projects endpoint
    pub const PROJECTS: &str = "projects";

    /// Workspaces endpoint
    pub const WORKSPACES: &str = "workspaces";

    /// Runs endpoint
    pub const RUNS: &str = "runs";

    /// Teams endpoint
    pub const TEAMS: &str = "teams";

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

/// Configuration constants for host resolution
pub mod host {
    /// Environment variable for hostname
    pub const ENV_VAR: &str = "TFE_HOSTNAME";
}

/// Default values for CLI
pub mod defaults {
    /// Default log level
    pub const LOG_LEVEL: &str = "warn";
}

/// Configuration for update checker
pub mod update {
    use std::time::Duration;

    /// GitHub repository for releases (owner/repo)
    pub const GITHUB_REPO: &str = "pkodzis/hcpctl";

    /// How often to check for updates
    pub const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60); // 24 hours

    /// Timeout for GitHub API request
    pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);

    /// Install script URLs
    pub mod install {
        /// Unix install script
        pub const UNIX_SCRIPT: &str =
            "https://raw.githubusercontent.com/pkodzis/hcpctl/main/scripts/install.sh";

        /// Windows install script
        pub const WINDOWS_SCRIPT: &str =
            "https://raw.githubusercontent.com/pkodzis/hcpctl/main/scripts/install.ps1";
    }
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
    fn test_host_env_var() {
        assert_eq!(host::ENV_VAR, "TFE_HOSTNAME");
    }

    #[test]
    fn test_api_endpoints() {
        assert_eq!(api::ORGANIZATIONS, "organizations");
        assert_eq!(api::PROJECTS, "projects");
        assert_eq!(api::WORKSPACES, "workspaces");
        assert_eq!(api::RUNS, "runs");
        assert_eq!(api::TEAMS, "teams");
    }

    #[test]
    fn test_default_page_size() {
        assert_eq!(api::DEFAULT_PAGE_SIZE, 100);
    }

    #[test]
    fn test_credentials_file_paths() {
        assert!(credentials::FILE_NAME.contains("credentials.tfrc.json"));
        assert!(credentials::FILE_PATH_UNIX.contains(".terraform.d"));
    }

    #[test]
    fn test_default_log_level() {
        assert_eq!(defaults::LOG_LEVEL, "warn");
    }
}
