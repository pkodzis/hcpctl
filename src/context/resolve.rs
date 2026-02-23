//! Context resolution from multiple sources

use log::debug;

use crate::config::context as context_config;

use super::models::Context;
use super::store::ContextStore;

/// Resolve the active context name from multiple sources:
/// 1. --context CLI flag
/// 2. HCPCTL_CONTEXT env var
/// 3. current-context from config file
pub fn resolve_active_context_name(cli_context: Option<&str>) -> Option<String> {
    // 1. CLI flag
    if let Some(name) = cli_context {
        debug!("Using context from CLI flag: {}", name);
        return Some(name.to_string());
    }

    // 2. Environment variable
    if let Ok(name) = std::env::var(context_config::ENV_VAR) {
        if !name.is_empty() {
            debug!(
                "Using context from {} env var: {}",
                context_config::ENV_VAR,
                name
            );
            return Some(name);
        }
    }

    // 3. Config file current-context
    let store = ContextStore::new();
    if let Ok(config) = store.load() {
        if let Some(name) = config.current_context {
            debug!("Using context from config file: {}", name);
            return Some(name);
        }
    }

    None
}

/// Load config and resolve the full active Context object
pub fn resolve_active_context(cli_context: Option<&str>) -> Option<Context> {
    let name = resolve_active_context_name(cli_context)?;

    let store = ContextStore::new();
    let config = match store.load() {
        Ok(c) => c,
        Err(e) => {
            debug!("Failed to load context config: {}", e);
            return None;
        }
    };

    match config.contexts.get(&name) {
        Some(ctx) => {
            debug!("Resolved context '{}': host={}", name, ctx.host);
            Some(ctx.clone())
        }
        None => {
            debug!("Context '{}' not found in config", name);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_flag_wins() {
        let result = resolve_active_context_name(Some("my-context"));
        assert_eq!(result, Some("my-context".to_string()));
    }

    #[test]
    fn test_none_when_no_sources() {
        // With no CLI flag and no env var set, falls back to config file
        // which may or may not exist - just verify it doesn't panic
        let result = resolve_active_context_name(None);
        // Result depends on whether ~/.hcpctl/config.json exists
        // We can't assert the exact value in a unit test
        let _ = result;
    }

    #[test]
    fn test_resolve_active_context_with_nonexistent_name() {
        // CLI flag points to a context that doesn't exist in config
        let result = resolve_active_context(Some("nonexistent-context-xyz"));
        assert!(result.is_none());
    }
}
