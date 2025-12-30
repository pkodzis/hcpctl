//! Host resolution from multiple sources

use dialoguer::{theme::ColorfulTheme, Select};
use log::debug;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

use crate::config::{credentials, host as host_config};
use crate::error::{Result, TfeError};

/// Credentials file structure (shared with TokenResolver)
#[derive(Deserialize, Debug)]
struct TfeCredentials {
    credentials: HashMap<String, serde_json::Value>,
}

/// Host resolution with fallback logic
pub struct HostResolver;

impl HostResolver {
    /// Resolve host from multiple sources with fallback:
    /// 1. CLI argument (if provided)
    /// 2. Environment variable (TFE_HOSTNAME)
    /// 3. Credentials file:
    ///    - If 1 host: use it
    ///    - If multiple hosts: interactive selection (or error in batch mode)
    ///    - If no hosts: error
    ///
    /// # Arguments
    /// * `cli_host` - Host from CLI argument (--host)
    /// * `batch_mode` - If true, error on multiple hosts instead of interactive selection
    pub fn resolve(cli_host: Option<&str>, batch_mode: bool) -> Result<String> {
        // 1. CLI argument takes precedence
        if let Some(host) = cli_host {
            debug!("Using host from CLI argument: {}", host);
            return Ok(host.to_string());
        }

        // 2. Environment variable
        if let Ok(host) = std::env::var(host_config::ENV_VAR) {
            debug!(
                "Using host from {} environment variable: {}",
                host_config::ENV_VAR,
                host
            );
            return Ok(host);
        }

        // 3. Credentials file
        debug!(
            "No host in CLI or {}, trying credentials file",
            host_config::ENV_VAR
        );
        Self::resolve_from_credentials_file(batch_mode)
    }

    /// Read available hosts from Terraform credentials file
    fn resolve_from_credentials_file(batch_mode: bool) -> Result<String> {
        let credentials_path = Self::get_credentials_path()
            .ok_or_else(|| TfeError::HostNotFound(Self::host_not_found_message(None, None)))?;

        debug!(
            "Looking for credentials file at: {}",
            credentials_path.display()
        );

        let content = match fs::read_to_string(&credentials_path) {
            Ok(content) => content,
            Err(_) => {
                return Err(TfeError::HostNotFound(Self::host_not_found_message(
                    Some(&credentials_path),
                    None,
                )));
            }
        };

        let creds: TfeCredentials = serde_json::from_str(&content).map_err(|e| {
            TfeError::Credentials(format!(
                "Could not parse credentials file {}: {}",
                credentials_path.display(),
                e
            ))
        })?;

        let mut hosts: Vec<String> = creds.credentials.keys().cloned().collect();
        hosts.sort(); // Sort for consistent ordering

        match hosts.len() {
            0 => Err(TfeError::HostNotFound(Self::host_not_found_message(
                Some(&credentials_path),
                None,
            ))),
            1 => {
                let host = hosts.into_iter().next().unwrap();
                debug!(
                    "Using single host from credentials file {}: {}",
                    credentials_path.display(),
                    host
                );
                Ok(host)
            }
            _ => {
                if batch_mode {
                    Err(TfeError::HostNotFound(Self::host_not_found_message(
                        Some(&credentials_path),
                        Some(&hosts),
                    )))
                } else {
                    Self::interactive_host_selection(&hosts, &credentials_path)
                }
            }
        }
    }

    /// Prompt user to select a host interactively
    fn interactive_host_selection(
        hosts: &[String],
        credentials_path: &std::path::Path,
    ) -> Result<String> {
        eprintln!("\nMultiple hosts found in {}:", credentials_path.display());

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select a host")
            .items(hosts)
            .default(0)
            .interact()
            .map_err(|e| TfeError::HostNotFound(format!("Failed to select host: {}", e)))?;

        let host = hosts[selection].clone();
        debug!("User selected host: {}", host);
        Ok(host)
    }

    /// Generate helpful error message when host is not found
    fn host_not_found_message(
        credentials_path: Option<&std::path::Path>,
        available_hosts: Option<&[String]>,
    ) -> String {
        let creds_info = match (credentials_path, available_hosts) {
            (Some(p), Some(hosts)) => format!(
                "\n   Credentials file: {} ({} hosts found)\n   Available hosts: {}",
                p.display(),
                hosts.len(),
                hosts.join(", ")
            ),
            (Some(p), None) => {
                format!("\n   Credentials file: {} (no hosts found)", p.display())
            }
            (None, _) => "\n   Credentials file: not found".to_string(),
        };

        format!(
            "No TFE/HCP host specified. Please provide a host using one of:\n\
             \n\
             1. CLI argument:      hcpctl --host <HOST>\n\
             2. Environment var:   export {}=<HOST>\n\
             3. Terraform login:   terraform login <HOST>\n\
             \n\
             Checked:{}\n",
            host_config::ENV_VAR,
            creds_info
        )
    }

    /// Get the path to Terraform credentials file (platform-specific)
    fn get_credentials_path() -> Option<std::path::PathBuf> {
        #[cfg(windows)]
        {
            dirs::config_dir().map(|p| p.join(credentials::FILE_NAME))
        }

        #[cfg(not(windows))]
        {
            dirs::home_dir().map(|p| p.join(credentials::FILE_PATH_UNIX))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_host_takes_precedence() {
        let result = HostResolver::resolve(Some("my-custom-host.com"), false);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "my-custom-host.com");
    }

    #[test]
    fn test_cli_host_takes_precedence_batch() {
        let result = HostResolver::resolve(Some("my-custom-host.com"), true);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "my-custom-host.com");
    }

    #[test]
    fn test_host_not_found_message_format() {
        let msg = HostResolver::host_not_found_message(None, None);
        assert!(msg.contains("hcpctl --host"));
        assert!(msg.contains(host_config::ENV_VAR));
        assert!(msg.contains("terraform login"));
    }

    #[test]
    fn test_host_not_found_message_with_available_hosts() {
        let hosts = vec![
            "host1.example.com".to_string(),
            "host2.example.com".to_string(),
        ];
        let path = std::path::Path::new("/test/path");
        let msg = HostResolver::host_not_found_message(Some(path), Some(&hosts));
        assert!(msg.contains("host1.example.com"));
        assert!(msg.contains("host2.example.com"));
        assert!(msg.contains("2 hosts found"));
    }
}
