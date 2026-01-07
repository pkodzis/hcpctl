//! Update checker module
//!
//! Checks for new versions of hcpctl and notifies the user.
//! Does NOT perform automatic updates - only shows instructions.

use log::debug;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;
use tokio::sync::oneshot;

use crate::config::update as config;

/// Cache file for update check results
#[derive(Debug, Serialize, Deserialize)]
struct UpdateCache {
    last_check: u64, // Unix timestamp
    latest_version: String,
}

/// Handle for receiving update check result
pub struct UpdateHandle {
    receiver: oneshot::Receiver<Option<String>>,
}

impl UpdateHandle {
    /// Get the update message if available (non-blocking check of completed task)
    pub fn get(mut self) -> Option<String> {
        self.receiver.try_recv().unwrap_or_default()
    }

    /// Wait for the update check to complete and get the message
    pub async fn wait(self) -> Option<String> {
        self.receiver.await.ok().flatten()
    }
}

/// Update checker
pub struct UpdateChecker {
    current_version: &'static str,
    cache_path: PathBuf,
}

impl UpdateChecker {
    /// Create a new update checker
    pub fn new() -> Self {
        let cache_dir = dirs::cache_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));

        Self {
            current_version: env!("CARGO_PKG_VERSION"),
            cache_path: cache_dir.join(".hcpctl").join("update-check.json"),
        }
    }

    /// Check if we should perform a version check (based on cache age)
    fn should_check(&self) -> bool {
        let cache = match self.read_cache() {
            Some(c) => c,
            None => return true, // No cache, should check
        };

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let elapsed = now.saturating_sub(cache.last_check);
        elapsed >= config::CHECK_INTERVAL.as_secs()
    }

    /// Read cache from disk
    fn read_cache(&self) -> Option<UpdateCache> {
        let content = fs::read_to_string(&self.cache_path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Spawn background version check (non-blocking)
    /// Returns a handle that can be used to get the result later
    pub fn check_async(&self) -> Option<UpdateHandle> {
        if !self.should_check() {
            // Check cache for existing update notification
            if let Some(cache) = self.read_cache() {
                if is_newer(&cache.latest_version, self.current_version) {
                    let msg = format_update_message(self.current_version, &cache.latest_version);
                    let (tx, rx) = oneshot::channel();
                    let _ = tx.send(Some(msg));
                    return Some(UpdateHandle { receiver: rx });
                }
            }
            return None;
        }

        let current = self.current_version.to_string();
        let cache_path = self.cache_path.clone();
        let (tx, rx) = oneshot::channel();

        tokio::spawn(async move {
            let result = check_version(&current, &cache_path).await;
            let _ = tx.send(result);
        });

        Some(UpdateHandle { receiver: rx })
    }
}

impl Default for UpdateChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// Async version check
async fn check_version(current_version: &str, cache_path: &PathBuf) -> Option<String> {
    debug!("Checking for updates...");

    // Fetch latest version from GitHub
    let latest = match fetch_latest_version().await {
        Ok(v) => v,
        Err(e) => {
            debug!("Failed to check for updates: {}", e);
            return None;
        }
    };

    debug!("Current: {}, Latest: {}", current_version, latest);

    // Update cache
    let cache = UpdateCache {
        last_check: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        latest_version: latest.clone(),
    };

    if let Some(parent) = cache_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string(&cache) {
        let _ = fs::write(cache_path, content);
    }

    // Check if update available
    if is_newer(&latest, current_version) {
        Some(format_update_message(current_version, &latest))
    } else {
        None
    }
}

/// Fetch latest version from GitHub Releases API
async fn fetch_latest_version() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        config::GITHUB_REPO
    );

    let client = Client::builder().timeout(config::REQUEST_TIMEOUT).build()?;

    let response = client
        .get(&url)
        .header("User-Agent", "hcpctl-update-checker")
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await?;

    let release: GitHubRelease = response.json().await?;
    let version = release.tag_name.trim_start_matches('v').to_string();

    Ok(version)
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
}

/// Compare versions (simple semver comparison)
fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> (u32, u32, u32) {
        let parts: Vec<u32> = v
            .trim_start_matches('v')
            .split('.')
            .filter_map(|p| p.parse().ok())
            .collect();
        (
            parts.first().copied().unwrap_or(0),
            parts.get(1).copied().unwrap_or(0),
            parts.get(2).copied().unwrap_or(0),
        )
    };

    parse(latest) > parse(current)
}

/// Format the update notification message using comfy_table for proper borders
fn format_update_message(current: &str, latest: &str) -> String {
    use comfy_table::{presets::UTF8_BORDERS_ONLY, Table};

    let install_cmd = get_install_command();

    let mut table = Table::new();
    table.load_preset(UTF8_BORDERS_ONLY);

    table.add_row(vec![format!(
        "A new version of hcpctl is available: {} → {}",
        current, latest
    )]);
    table.add_row(vec![String::new()]);
    table.add_row(vec!["To update, run:".to_string()]);
    table.add_row(vec![format!("  {}", install_cmd)]);

    format!("\n{}", table)
}

/// Get platform-specific install command
fn get_install_command() -> String {
    #[cfg(target_os = "windows")]
    {
        format!(
            "Invoke-RestMethod {} | Invoke-Expression",
            config::install::WINDOWS_SCRIPT
        )
    }

    #[cfg(not(target_os = "windows"))]
    {
        format!("curl -fsSL {} | bash", config::install::UNIX_SCRIPT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer() {
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(is_newer("0.4.0", "0.3.1"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(is_newer("v1.0.0", "0.9.9"));

        assert!(!is_newer("0.3.1", "0.3.1"));
        assert!(!is_newer("0.3.0", "0.3.1"));
        assert!(!is_newer("0.2.9", "0.3.1"));
    }

    #[test]
    fn test_get_install_command_not_empty() {
        let cmd = get_install_command();
        assert!(!cmd.is_empty());
        assert!(cmd.contains("hcpctl"));
    }

    #[test]
    fn test_format_update_message() {
        let msg = format_update_message("0.3.1", "0.4.0");
        assert!(msg.contains("0.3.1"));
        assert!(msg.contains("0.4.0"));
        assert!(msg.contains("hcpctl"));
    }
}
