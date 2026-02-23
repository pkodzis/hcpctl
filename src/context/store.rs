//! Context configuration file I/O

use std::fs;
use std::path::PathBuf;

use crate::config::context as context_config;
use crate::error::TfeError;

use super::models::ContextConfig;

/// Handles reading and writing the context configuration file
pub struct ContextStore {
    config_path: PathBuf,
}

impl Default for ContextStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextStore {
    /// Create a new store using the default config path (~/.hcpctl/config.json)
    pub fn new() -> Self {
        Self {
            config_path: Self::default_config_path(),
        }
    }

    /// Create a store with a custom config path (for testing)
    pub fn with_path(path: PathBuf) -> Self {
        Self { config_path: path }
    }

    /// Get the default config file path
    fn default_config_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(context_config::DIR_NAME)
            .join(context_config::FILE_NAME)
    }

    /// Load the context configuration from disk.
    /// Returns Default if file doesn't exist, errors on corrupt JSON.
    pub fn load(&self) -> Result<ContextConfig, TfeError> {
        if !self.config_path.exists() {
            return Ok(ContextConfig::default());
        }

        let content = fs::read_to_string(&self.config_path).map_err(|e| {
            TfeError::Config(format!(
                "Failed to read context config {}: {}",
                self.config_path.display(),
                e
            ))
        })?;

        serde_json::from_str(&content).map_err(|e| {
            TfeError::Config(format!(
                "Failed to parse context config {}: {}",
                self.config_path.display(),
                e
            ))
        })
    }

    /// Save the context configuration to disk.
    /// Uses atomic write (tmp file + rename) and creates parent dir if needed.
    pub fn save(&self, config: &ContextConfig) -> Result<(), TfeError> {
        // Create parent directory if missing
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                TfeError::Config(format!(
                    "Failed to create config directory {}: {}",
                    parent.display(),
                    e
                ))
            })?;
        }

        let json = serde_json::to_string_pretty(config)
            .map_err(|e| TfeError::Config(format!("Failed to serialize context config: {}", e)))?;

        // Atomic write: write to tmp file, then rename
        let tmp_path = self.config_path.with_extension("json.tmp");
        fs::write(&tmp_path, &json).map_err(|e| {
            TfeError::Config(format!(
                "Failed to write temp config file {}: {}",
                tmp_path.display(),
                e
            ))
        })?;

        // Set 0600 permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = fs::Permissions::from_mode(0o600);
            fs::set_permissions(&tmp_path, permissions).map_err(|e| {
                TfeError::Config(format!("Failed to set permissions on config file: {}", e))
            })?;
        }

        fs::rename(&tmp_path, &self.config_path).map_err(|e| {
            TfeError::Config(format!(
                "Failed to rename temp config file to {}: {}",
                self.config_path.display(),
                e
            ))
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::models::Context;
    use tempfile::TempDir;

    fn test_store(dir: &TempDir) -> ContextStore {
        ContextStore::with_path(dir.path().join("config.json"))
    }

    #[test]
    fn test_load_missing_file_returns_default() {
        let dir = TempDir::new().unwrap();
        let store = test_store(&dir);
        let config = store.load().unwrap();
        assert!(config.current_context.is_none());
        assert!(config.contexts.is_empty());
    }

    #[test]
    fn test_load_corrupt_json_errors() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, "not valid json!!!").unwrap();
        let store = ContextStore::with_path(path);
        let result = store.load();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Failed to parse context config"));
    }

    #[test]
    fn test_save_creates_parent_dir() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("subdir").join("config.json");
        let store = ContextStore::with_path(path.clone());
        let config = ContextConfig::default();
        store.save(&config).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_save_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let store = test_store(&dir);

        let mut config = ContextConfig {
            current_context: Some("prod".to_string()),
            ..Default::default()
        };
        config.contexts.insert(
            "prod".to_string(),
            Context {
                host: "app.terraform.io".to_string(),
                token: Some("my-token".to_string()),
                org: Some("my-org".to_string()),
            },
        );

        store.save(&config).unwrap();
        let loaded = store.load().unwrap();

        assert_eq!(loaded.current_context, Some("prod".to_string()));
        assert_eq!(loaded.contexts.len(), 1);
        assert_eq!(loaded.contexts["prod"].host, "app.terraform.io");
        assert_eq!(loaded.contexts["prod"].token, Some("my-token".to_string()));
        assert_eq!(loaded.contexts["prod"].org, Some("my-org".to_string()));
    }

    #[test]
    fn test_save_overwrites_existing() {
        let dir = TempDir::new().unwrap();
        let store = test_store(&dir);

        let mut config1 = ContextConfig::default();
        config1.contexts.insert(
            "first".to_string(),
            Context {
                host: "first.com".to_string(),
                token: None,
                org: None,
            },
        );
        store.save(&config1).unwrap();

        let mut config2 = ContextConfig::default();
        config2.contexts.insert(
            "second".to_string(),
            Context {
                host: "second.com".to_string(),
                token: None,
                org: None,
            },
        );
        store.save(&config2).unwrap();

        let loaded = store.load().unwrap();
        assert_eq!(loaded.contexts.len(), 1);
        assert!(loaded.contexts.contains_key("second"));
        assert!(!loaded.contexts.contains_key("first"));
    }

    #[cfg(unix)]
    #[test]
    fn test_save_sets_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new().unwrap();
        let store = test_store(&dir);
        let config = ContextConfig::default();
        store.save(&config).unwrap();

        let metadata = fs::metadata(&store.config_path).unwrap();
        let mode = metadata.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn test_default_config_path() {
        let path = ContextStore::default_config_path();
        assert!(path.to_string_lossy().contains(context_config::DIR_NAME));
        assert!(path.to_string_lossy().contains(context_config::FILE_NAME));
    }
}
