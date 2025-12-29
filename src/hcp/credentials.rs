//! TFE token resolution from multiple sources

use log::debug;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

use crate::config::{credentials, defaults};
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
    /// 2. Environment variable (TFE_TOKEN)
    /// 3. Credentials file (~/.terraform.d/credentials.tfrc.json)
    pub fn resolve(&self, cli_token: Option<&str>) -> Result<String> {
        // 1. CLI argument takes precedence
        if let Some(token) = cli_token {
            debug!("Using token from CLI argument");
            return Ok(token.to_string());
        }

        // 2. Environment variable
        if let Ok(token) = std::env::var(credentials::TOKEN_ENV_VAR) {
            debug!(
                "Using token from {} environment variable",
                credentials::TOKEN_ENV_VAR
            );
            return Ok(token);
        }

        // 3. Credentials file
        debug!(
            "{} not found, trying credentials file",
            credentials::TOKEN_ENV_VAR
        );
        self.read_from_credentials_file()
    }

    /// Read token from Terraform credentials file
    fn read_from_credentials_file(&self) -> Result<String> {
        // Cross-platform home directory (works on Linux, macOS, Windows)
        let home = dirs::home_dir().ok_or_else(|| {
            TfeError::Credentials("Could not determine home directory".to_string())
        })?;

        let credentials_path = home.join(credentials::FILE_PATH);

        let content = fs::read_to_string(&credentials_path).map_err(|e| {
            TfeError::Credentials(format!(
                "Could not read credentials file {}: {}",
                credentials_path.display(),
                e
            ))
        })?;

        let creds: TfeCredentials = serde_json::from_str(&content).map_err(|e| {
            TfeError::Credentials(format!("Could not parse credentials file: {}", e))
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
                TfeError::TokenNotFound(format!(
                    "No token found for host '{}' in credentials file {} or {} env var",
                    self.host,
                    credentials_path.display(),
                    credentials::TOKEN_ENV_VAR
                ))
            })
    }
}

impl Default for TokenResolver {
    fn default() -> Self {
        Self::new(defaults::HOST)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolver_cli_token_takes_precedence() {
        let resolver = TokenResolver::new("test.example.com");
        let result = resolver.resolve(Some("cli-token-123"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "cli-token-123");
    }

    #[test]
    fn test_resolver_new() {
        let resolver = TokenResolver::new("custom.host.com");
        assert_eq!(resolver.host, "custom.host.com");
    }

    #[test]
    fn test_resolver_default() {
        let resolver = TokenResolver::default();
        assert_eq!(resolver.host, defaults::HOST);
    }
}
