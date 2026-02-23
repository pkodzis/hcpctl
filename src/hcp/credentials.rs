//! TFE token resolution from multiple sources

use log::debug;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

use crate::config::credentials;
use crate::error::{Result, TfeError};

/// Credentials file structure
#[derive(Deserialize, Debug)]
struct TfeCredentials {
    credentials: HashMap<String, TfeCredential>,
}

/// Single credential entry
#[derive(Deserialize, Debug)]
struct TfeCredential {
    token: String,
}

/// Token resolution with fallback logic
pub struct TokenResolver {
    host: String,
}

impl TokenResolver {
    /// Create a new token resolver for the given host
    pub fn new(host: &str) -> Self {
        Self {
            host: host.to_string(),
        }
    }

    /// Resolve token from multiple sources with fallback:
    /// 1. CLI argument (if provided)
    /// 2. Environment variables (HCP_TOKEN, TFC_TOKEN, TFE_TOKEN - in order)
    /// 3. Active context token
    /// 4. Credentials file (~/.terraform.d/credentials.tfrc.json)
    pub fn resolve(&self, cli_token: Option<&str>, context_token: Option<&str>) -> Result<String> {
        // 1. CLI argument takes precedence
        if let Some(token) = cli_token {
            debug!("Using token from CLI argument");
            return Ok(token.to_string());
        }

        // 2. Environment variables (try in order)
        for env_var in credentials::TOKEN_ENV_VARS {
            if let Ok(token) = std::env::var(env_var) {
                debug!("Using token from {} environment variable", env_var);
                return Ok(token);
            }
        }

        // 3. Context token
        if let Some(token) = context_token {
            debug!("Using token from active context");
            return Ok(token.to_string());
        }

        // 4. Credentials file
        debug!(
            "No token found in environment variables {:?} or context, trying credentials file",
            credentials::TOKEN_ENV_VARS
        );
        self.read_from_credentials_file()
    }

    /// Read token from Terraform credentials file
    fn read_from_credentials_file(&self) -> Result<String> {
        let credentials_path = Self::get_credentials_path()
            .ok_or_else(|| TfeError::TokenNotFound(self.token_not_found_message(None)))?;

        debug!(
            "Looking for credentials file at: {}",
            credentials_path.display()
        );

        let content = match fs::read_to_string(&credentials_path) {
            Ok(content) => content,
            Err(_) => {
                return Err(TfeError::TokenNotFound(
                    self.token_not_found_message(Some(&credentials_path)),
                ));
            }
        };

        let creds: TfeCredentials = serde_json::from_str(&content).map_err(|e| {
            TfeError::Credentials(format!(
                "Could not parse credentials file {}: {}",
                credentials_path.display(),
                e
            ))
        })?;

        creds
            .credentials
            .get(&self.host)
            .map(|cred| {
                debug!(
                    "Using token from credentials file {} for host: {}",
                    credentials_path.display(),
                    self.host
                );
                cred.token.clone()
            })
            .ok_or_else(|| {
                TfeError::TokenNotFound(self.token_not_found_message(Some(&credentials_path)))
            })
    }

    /// Generate helpful error message when token is not found
    fn token_not_found_message(&self, credentials_path: Option<&std::path::Path>) -> String {
        let env_vars = credentials::TOKEN_ENV_VARS.join(", ");
        let creds_info = credentials_path
            .map(|p| format!(" or in credentials file {}", p.display()))
            .unwrap_or_default();

        format!(
            "No API token found for host '{}'. Please provide a token using one of:\n\
             \n\
             1. CLI argument:      hcpctl --token <TOKEN>\n\
             2. Environment var:   export HCP_TOKEN=<TOKEN>  (also: TFC_TOKEN, TFE_TOKEN)\n\
             3. Terraform login:   terraform login {}\n\
             \n\
             Checked: env vars [{}]{}",
            self.host, self.host, env_vars, creds_info
        )
    }

    /// Get the path to Terraform credentials file (platform-specific)
    /// - Windows: %APPDATA%\terraform.d\credentials.tfrc.json
    /// - Linux/macOS: ~/.terraform.d/credentials.tfrc.json
    fn get_credentials_path() -> Option<std::path::PathBuf> {
        #[cfg(windows)]
        {
            // On Windows, Terraform uses %APPDATA%\terraform.d\
            dirs::config_dir().map(|p| p.join(credentials::FILE_NAME))
        }

        #[cfg(not(windows))]
        {
            // On Linux/macOS, Terraform uses ~/.terraform.d/
            dirs::home_dir().map(|p| p.join(credentials::FILE_PATH_UNIX))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolver_cli_token_takes_precedence() {
        let resolver = TokenResolver::new("test.example.com");
        let result = resolver.resolve(Some("cli-token-123"), None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "cli-token-123");
    }

    #[test]
    fn test_resolver_new() {
        let resolver = TokenResolver::new("custom.host.com");
        assert_eq!(resolver.host, "custom.host.com");
    }

    #[test]
    fn test_token_not_found_message_format() {
        let resolver = TokenResolver::new("app.terraform.io");
        let msg = resolver.token_not_found_message(None);
        assert!(msg.contains("app.terraform.io"));
        assert!(msg.contains("hcpctl --token"));
        assert!(msg.contains("HCP_TOKEN"));
        assert!(msg.contains("terraform login"));
    }

    #[test]
    fn test_token_not_found_message_with_path() {
        let resolver = TokenResolver::new("app.terraform.io");
        let path = std::path::Path::new("/home/user/.terraform.d/credentials.tfrc.json");
        let msg = resolver.token_not_found_message(Some(path));
        assert!(msg.contains("app.terraform.io"));
        assert!(msg.contains("/home/user/.terraform.d/credentials.tfrc.json"));
    }

    #[test]
    fn test_credentials_file_parsing() {
        let json = r#"{
            "credentials": {
                "app.terraform.io": {
                    "token": "test-token-123"
                },
                "custom.host.com": {
                    "token": "custom-token-456"
                }
            }
        }"#;

        let creds: TfeCredentials = serde_json::from_str(json).unwrap();
        assert_eq!(creds.credentials.len(), 2);
        assert_eq!(
            creds.credentials.get("app.terraform.io").unwrap().token,
            "test-token-123"
        );
        assert_eq!(
            creds.credentials.get("custom.host.com").unwrap().token,
            "custom-token-456"
        );
    }

    #[test]
    fn test_credentials_file_parsing_empty() {
        let json = r#"{"credentials": {}}"#;
        let creds: TfeCredentials = serde_json::from_str(json).unwrap();
        assert!(creds.credentials.is_empty());
    }

    #[test]
    fn test_get_credentials_path() {
        let path = TokenResolver::get_credentials_path();
        // Should return Some path on any platform
        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.to_string_lossy().contains("credentials.tfrc.json"));
    }
}
