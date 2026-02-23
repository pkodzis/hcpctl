//! Context configuration data models

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Top-level context configuration
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ContextConfig {
    /// Name of the currently active context
    #[serde(rename = "current-context", skip_serializing_if = "Option::is_none")]
    pub current_context: Option<String>,
    /// Map of context name to context configuration
    #[serde(default)]
    pub contexts: BTreeMap<String, Context>,
}

/// A named context with connection parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    /// TFE/HCP host URL
    pub host: String,
    /// API token (stored in config file)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// Default organization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub org: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_config_default() {
        let config = ContextConfig::default();
        assert!(config.current_context.is_none());
        assert!(config.contexts.is_empty());
    }

    #[test]
    fn test_serde_roundtrip() {
        let mut config = ContextConfig {
            current_context: Some("prod".to_string()),
            ..Default::default()
        };
        config.contexts.insert(
            "prod".to_string(),
            Context {
                host: "app.terraform.io".to_string(),
                token: Some("secret-token".to_string()),
                org: Some("my-org".to_string()),
            },
        );
        config.contexts.insert(
            "dev".to_string(),
            Context {
                host: "tfe-dev.corp.com".to_string(),
                token: None,
                org: None,
            },
        );

        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: ContextConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.current_context, Some("prod".to_string()));
        assert_eq!(parsed.contexts.len(), 2);
        assert_eq!(parsed.contexts["prod"].host, "app.terraform.io".to_string());
        assert_eq!(
            parsed.contexts["prod"].token,
            Some("secret-token".to_string())
        );
        assert_eq!(parsed.contexts["prod"].org, Some("my-org".to_string()));
        assert_eq!(parsed.contexts["dev"].host, "tfe-dev.corp.com".to_string());
        assert!(parsed.contexts["dev"].token.is_none());
        assert!(parsed.contexts["dev"].org.is_none());
    }

    #[test]
    fn test_skip_serializing_if_none() {
        let config = ContextConfig {
            current_context: None,
            contexts: BTreeMap::new(),
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(!json.contains("current-context"));
    }

    #[test]
    fn test_skip_serializing_optional_fields() {
        let mut config = ContextConfig::default();
        config.contexts.insert(
            "test".to_string(),
            Context {
                host: "example.com".to_string(),
                token: None,
                org: None,
            },
        );
        let json = serde_json::to_string(&config).unwrap();
        assert!(!json.contains("token"));
        assert!(!json.contains("org"));
    }

    #[test]
    fn test_btreemap_ordering() {
        let mut config = ContextConfig::default();
        config.contexts.insert(
            "zebra".to_string(),
            Context {
                host: "z.com".to_string(),
                token: None,
                org: None,
            },
        );
        config.contexts.insert(
            "alpha".to_string(),
            Context {
                host: "a.com".to_string(),
                token: None,
                org: None,
            },
        );
        config.contexts.insert(
            "middle".to_string(),
            Context {
                host: "m.com".to_string(),
                token: None,
                org: None,
            },
        );

        let keys: Vec<&String> = config.contexts.keys().collect();
        assert_eq!(keys, vec!["alpha", "middle", "zebra"]);
    }

    #[test]
    fn test_deserialize_with_missing_contexts() {
        let json = r#"{"current-context": "prod"}"#;
        let config: ContextConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.current_context, Some("prod".to_string()));
        assert!(config.contexts.is_empty());
    }

    #[test]
    fn test_deserialize_empty_json() {
        let json = "{}";
        let config: ContextConfig = serde_json::from_str(json).unwrap();
        assert!(config.current_context.is_none());
        assert!(config.contexts.is_empty());
    }

    #[test]
    fn test_context_clone() {
        let ctx = Context {
            host: "example.com".to_string(),
            token: Some("tok".to_string()),
            org: Some("org".to_string()),
        };
        let cloned = ctx.clone();
        assert_eq!(cloned.host, ctx.host);
        assert_eq!(cloned.token, ctx.token);
        assert_eq!(cloned.org, ctx.org);
    }
}
