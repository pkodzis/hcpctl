//! Context command handlers

use comfy_table::{presets::UTF8_FULL_CONDENSED, Cell, ContentArrangement, Table};

use crate::cli::{ConfigAction, SetContextArgs};
use crate::error::TfeError;

use super::models::Context;
use super::store::ContextStore;

/// Dispatch context subcommands
pub fn run_context_command(action: &ConfigAction) -> Result<(), Box<dyn std::error::Error>> {
    let store = ContextStore::new();
    match action {
        ConfigAction::GetContexts => run_context_list(&store),
        ConfigAction::SetContext(args) => run_context_set(&store, args),
        ConfigAction::UseContext(args) => run_context_use(&store, &args.name),
        ConfigAction::DeleteContext(args) => run_context_delete(&store, &args.name),
        ConfigAction::CurrentContext => run_context_show(&store),
        ConfigAction::View => run_config_view(&store),
    }
}

/// List all contexts
fn run_context_list(store: &ContextStore) -> Result<(), Box<dyn std::error::Error>> {
    let config = store.load()?;

    if config.contexts.is_empty() {
        println!("No contexts configured.");
        println!("\nUse 'hcpctl config set-context <name> --host <host>' to create one.");
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("CURRENT"),
            Cell::new("NAME"),
            Cell::new("HOST"),
            Cell::new("ORG"),
            Cell::new("TOKEN"),
        ]);

    for (name, ctx) in &config.contexts {
        let is_current = config.current_context.as_ref().is_some_and(|c| c == name);

        let current_marker = if is_current { "*" } else { "" };
        let org_display = ctx.org.as_deref().unwrap_or("<not set>");
        let token_display = mask_token(ctx.token.as_deref());

        table.add_row(vec![
            Cell::new(current_marker),
            Cell::new(name),
            Cell::new(&ctx.host),
            Cell::new(org_display),
            Cell::new(&token_display),
        ]);
    }

    println!("{table}");
    Ok(())
}

/// Show the current context details
fn run_context_show(store: &ContextStore) -> Result<(), Box<dyn std::error::Error>> {
    let config = store.load()?;

    let current_name = config.current_context.as_ref().ok_or_else(|| {
        TfeError::Config(
            "No current context set. Use 'hcpctl config use-context <name>' to set one."
                .to_string(),
        )
    })?;

    let ctx = config.contexts.get(current_name).ok_or_else(|| {
        TfeError::Config(format!(
            "Current context '{}' not found in config. Available: {}",
            current_name,
            config
                .contexts
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ))
    })?;

    println!("Current context: {}", current_name);
    println!("  Host:  {}", ctx.host);
    println!("  Token: {}", mask_token(ctx.token.as_deref()));
    println!("  Org:   {}", ctx.org.as_deref().unwrap_or("<not set>"));

    Ok(())
}

/// Create or update a named context
fn run_context_set(
    store: &ContextStore,
    args: &SetContextArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = store.load()?;

    if let Some(existing) = config.contexts.get_mut(&args.name) {
        // Update existing context - merge provided fields
        if let Some(host) = &args.host {
            existing.host = host.clone();
        }
        if args.token.is_some() {
            existing.token = args.token.clone();
        }
        if args.org.is_some() {
            existing.org = args.org.clone();
        }
        store.save(&config)?;
        println!("✓ Updated context '{}'", args.name);
    } else {
        // Create new context - host is required
        let host = args.host.as_ref().ok_or_else(|| {
            TfeError::Config(format!(
                "--host is required when creating a new context. Usage:\n  \
                 hcpctl config set-context {} --host <HOST> [--token <TOKEN>] [--org <ORG>]",
                args.name
            ))
        })?;

        let ctx = Context {
            host: host.clone(),
            token: args.token.clone(),
            org: args.org.clone(),
        };

        config.contexts.insert(args.name.clone(), ctx);

        // Auto-set current-context if this is the first context
        if config.contexts.len() == 1 {
            config.current_context = Some(args.name.clone());
        }

        store.save(&config)?;
        println!("✓ Created context '{}'", args.name);
    }

    Ok(())
}

/// Switch the active context
fn run_context_use(store: &ContextStore, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = store.load()?;

    if !config.contexts.contains_key(name) {
        return Err(TfeError::Config(format!(
            "Context '{}' not found. Available contexts: {}",
            name,
            config
                .contexts
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ))
        .into());
    }

    config.current_context = Some(name.to_string());
    store.save(&config)?;
    println!("✓ Switched to context '{}'", name);

    Ok(())
}

/// Delete a named context
fn run_context_delete(store: &ContextStore, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = store.load()?;

    if config.contexts.remove(name).is_none() {
        return Err(TfeError::Config(format!(
            "Context '{}' not found. Available contexts: {}",
            name,
            config
                .contexts
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ))
        .into());
    }

    // Clear current-context if it matched the deleted one
    if config.current_context.as_deref() == Some(name) {
        config.current_context = None;
    }

    store.save(&config)?;
    println!("✓ Deleted context '{}'", name);

    Ok(())
}

/// Display the raw config file contents
fn run_config_view(store: &ContextStore) -> Result<(), Box<dyn std::error::Error>> {
    let config = store.load()?;
    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| TfeError::Config(format!("Failed to serialize config: {}", e)))?;
    println!("{}", json);
    Ok(())
}

/// Mask a token for display — show last 4 chars or "<not set>"
fn mask_token(token: Option<&str>) -> String {
    match token {
        Some(t) if t.len() >= 4 => format!("****{}", &t[t.len() - 4..]),
        Some(_) => "****".to_string(),
        None => "<not set>".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::models::{Context, ContextConfig};
    use tempfile::TempDir;

    fn test_store(dir: &TempDir) -> ContextStore {
        ContextStore::with_path(dir.path().join("config.json"))
    }

    #[test]
    fn test_mask_token_long() {
        assert_eq!(mask_token(Some("abcdefghijklmnop")), "****mnop");
    }

    #[test]
    fn test_mask_token_short() {
        assert_eq!(mask_token(Some("ab")), "****");
    }

    #[test]
    fn test_mask_token_none() {
        assert_eq!(mask_token(None), "<not set>");
    }

    #[test]
    fn test_mask_token_exactly_4() {
        assert_eq!(mask_token(Some("abcd")), "****abcd");
    }

    #[test]
    fn test_context_set_new_requires_host() {
        let dir = TempDir::new().unwrap();
        let store = test_store(&dir);
        let args = SetContextArgs {
            name: "test".to_string(),
            host: None,
            token: None,
            org: None,
        };
        let result = run_context_set(&store, &args);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("--host is required"));
    }

    #[test]
    fn test_context_set_new_creates() {
        let dir = TempDir::new().unwrap();
        let store = test_store(&dir);
        let args = SetContextArgs {
            name: "prod".to_string(),
            host: Some("app.terraform.io".to_string()),
            token: Some("my-token".to_string()),
            org: Some("my-org".to_string()),
        };
        run_context_set(&store, &args).unwrap();

        let config = store.load().unwrap();
        assert_eq!(config.contexts.len(), 1);
        assert_eq!(config.contexts["prod"].host, "app.terraform.io");
        assert_eq!(config.contexts["prod"].token, Some("my-token".to_string()));
        assert_eq!(config.contexts["prod"].org, Some("my-org".to_string()));
        // First context auto-sets current-context
        assert_eq!(config.current_context, Some("prod".to_string()));
    }

    #[test]
    fn test_context_set_update_merges() {
        let dir = TempDir::new().unwrap();
        let store = test_store(&dir);

        // Create initial context
        let args = SetContextArgs {
            name: "prod".to_string(),
            host: Some("old-host.com".to_string()),
            token: Some("old-token".to_string()),
            org: Some("old-org".to_string()),
        };
        run_context_set(&store, &args).unwrap();

        // Update only org
        let args = SetContextArgs {
            name: "prod".to_string(),
            host: None,
            token: None,
            org: Some("new-org".to_string()),
        };
        run_context_set(&store, &args).unwrap();

        let config = store.load().unwrap();
        assert_eq!(config.contexts["prod"].host, "old-host.com"); // unchanged
        assert_eq!(config.contexts["prod"].token, Some("old-token".to_string())); // unchanged
        assert_eq!(config.contexts["prod"].org, Some("new-org".to_string())); // updated
    }

    #[test]
    fn test_context_use_sets_current() {
        let dir = TempDir::new().unwrap();
        let store = test_store(&dir);

        // Create two contexts
        let mut config = ContextConfig::default();
        config.contexts.insert(
            "prod".to_string(),
            Context {
                host: "prod.com".to_string(),
                token: None,
                org: None,
            },
        );
        config.contexts.insert(
            "dev".to_string(),
            Context {
                host: "dev.com".to_string(),
                token: None,
                org: None,
            },
        );
        store.save(&config).unwrap();

        run_context_use(&store, "dev").unwrap();

        let config = store.load().unwrap();
        assert_eq!(config.current_context, Some("dev".to_string()));
    }

    #[test]
    fn test_context_use_nonexistent_errors() {
        let dir = TempDir::new().unwrap();
        let store = test_store(&dir);
        let result = run_context_use(&store, "nonexistent");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"));
    }

    #[test]
    fn test_context_delete_removes() {
        let dir = TempDir::new().unwrap();
        let store = test_store(&dir);

        let mut config = ContextConfig {
            current_context: Some("prod".to_string()),
            ..Default::default()
        };
        config.contexts.insert(
            "prod".to_string(),
            Context {
                host: "prod.com".to_string(),
                token: None,
                org: None,
            },
        );
        store.save(&config).unwrap();

        run_context_delete(&store, "prod").unwrap();

        let config = store.load().unwrap();
        assert!(config.contexts.is_empty());
        assert!(config.current_context.is_none()); // cleared
    }

    #[test]
    fn test_context_delete_nonexistent_errors() {
        let dir = TempDir::new().unwrap();
        let store = test_store(&dir);
        let result = run_context_delete(&store, "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_context_delete_preserves_current_if_different() {
        let dir = TempDir::new().unwrap();
        let store = test_store(&dir);

        let mut config = ContextConfig {
            current_context: Some("prod".to_string()),
            ..Default::default()
        };
        config.contexts.insert(
            "prod".to_string(),
            Context {
                host: "prod.com".to_string(),
                token: None,
                org: None,
            },
        );
        config.contexts.insert(
            "dev".to_string(),
            Context {
                host: "dev.com".to_string(),
                token: None,
                org: None,
            },
        );
        store.save(&config).unwrap();

        run_context_delete(&store, "dev").unwrap();

        let config = store.load().unwrap();
        assert_eq!(config.current_context, Some("prod".to_string())); // preserved
        assert_eq!(config.contexts.len(), 1);
    }

    #[test]
    fn test_context_set_second_does_not_auto_set_current() {
        let dir = TempDir::new().unwrap();
        let store = test_store(&dir);

        // Create first - should auto-set current
        let args = SetContextArgs {
            name: "first".to_string(),
            host: Some("first.com".to_string()),
            token: None,
            org: None,
        };
        run_context_set(&store, &args).unwrap();
        assert_eq!(
            store.load().unwrap().current_context,
            Some("first".to_string())
        );

        // Create second - should NOT change current
        let args = SetContextArgs {
            name: "second".to_string(),
            host: Some("second.com".to_string()),
            token: None,
            org: None,
        };
        run_context_set(&store, &args).unwrap();
        assert_eq!(
            store.load().unwrap().current_context,
            Some("first".to_string())
        );
    }
}
