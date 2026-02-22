//! Tag command resource definitions and arguments

use clap::{Parser, Subcommand};

use super::common::OutputFormat;

/// Target resource types for tag operations
#[derive(Subcommand, Debug)]
pub enum SetTagResource {
    /// Set tags on a workspace
    #[command(visible_alias = "workspace", visible_alias = "workspaces")]
    Ws(SetTagWsArgs),

    /// Set tags on a project
    #[command(visible_alias = "project", visible_alias = "projects")]
    Prj(SetTagPrjArgs),
}

/// Wrapper args for 'get tag' — supports org-level listing and per-resource listing
#[derive(Parser, Debug)]
#[command(args_conflicts_with_subcommands = true)]
pub struct GetTagArgs {
    #[command(subcommand)]
    pub resource: Option<GetTagResource>,

    /// Tag name (shows/filters a specific tag at org level)
    pub name: Option<String>,

    /// Organization name (required for org-level listing; optional for per-resource)
    #[arg(long = "org", global = true)]
    pub org: Option<String>,

    /// Output format
    #[arg(short = 'o', long, value_enum, default_value_t = OutputFormat::Table, global = true)]
    pub output: OutputFormat,

    /// Filter tags by name (org-level only)
    #[arg(short = 'f', long, global = true)]
    pub filter: Option<String>,
}

/// Target resource types for getting tags
#[derive(Subcommand, Debug)]
pub enum GetTagResource {
    /// Get tags on a workspace
    #[command(visible_alias = "workspace", visible_alias = "workspaces")]
    Ws(GetTagWsArgs),

    /// Get tags on a project
    #[command(visible_alias = "project", visible_alias = "projects")]
    Prj(GetTagPrjArgs),
}

/// Target resource types for deleting tags
#[derive(Subcommand, Debug)]
pub enum DeleteTagResource {
    /// Delete tags from a workspace
    #[command(visible_alias = "workspace", visible_alias = "workspaces")]
    Ws(DeleteTagWsArgs),

    /// Delete tags from a project
    #[command(visible_alias = "project", visible_alias = "projects")]
    Prj(DeleteTagPrjArgs),
}

/// Arguments for 'set tag ws' subcommand
#[derive(Parser, Debug)]
pub struct SetTagWsArgs {
    /// Workspace name or ID (ws-xxx)
    pub workspace: String,

    /// Space-separated list of tags: flat names (e.g., env team) and/or key=value bindings (e.g., env=prod)
    #[arg(required = true, num_args = 1..)]
    pub tags: Vec<String>,

    /// Organization name (required when using workspace name)
    #[arg(long = "org")]
    pub org: Option<String>,

    /// Skip confirmation prompt
    #[arg(short = 'y', long, default_value_t = false)]
    pub yes: bool,
}

/// Arguments for 'set tag prj' subcommand
#[derive(Parser, Debug)]
pub struct SetTagPrjArgs {
    /// Project name or ID (prj-xxx)
    pub project: String,

    /// Space-separated list of key=value tag bindings (projects only support key=value, not flat tags)
    #[arg(required = true, num_args = 1..)]
    pub tags: Vec<String>,

    /// Organization name (required when using project name)
    #[arg(long = "org")]
    pub org: Option<String>,

    /// Skip confirmation prompt
    #[arg(short = 'y', long, default_value_t = false)]
    pub yes: bool,
}

/// Arguments for 'get tag ws' subcommand
#[derive(Parser, Debug)]
pub struct GetTagWsArgs {
    /// Workspace name or ID (ws-xxx)
    pub workspace: String,
}

/// Arguments for 'get tag prj' subcommand
#[derive(Parser, Debug)]
pub struct GetTagPrjArgs {
    /// Project name or ID (prj-xxx)
    pub project: String,
}

/// Arguments for 'delete tag ws' subcommand
#[derive(Parser, Debug)]
pub struct DeleteTagWsArgs {
    /// Workspace name or ID (ws-xxx)
    pub workspace: String,

    /// Space-separated list of tag names to remove: flat tags and/or binding keys (e.g., env team costcenter)
    #[arg(required = true, num_args = 1..)]
    pub keys: Vec<String>,

    /// Organization name (required when using workspace name)
    #[arg(long = "org")]
    pub org: Option<String>,

    /// Skip confirmation prompt
    #[arg(short = 'y', long, default_value_t = false)]
    pub yes: bool,
}

/// Arguments for 'delete tag prj' subcommand
#[derive(Parser, Debug)]
pub struct DeleteTagPrjArgs {
    /// Project name or ID (prj-xxx)
    pub project: String,

    /// Space-separated list of tag binding keys to remove (e.g., env team costcenter)
    #[arg(required = true, num_args = 1..)]
    pub keys: Vec<String>,

    /// Organization name (required when using project name)
    #[arg(long = "org")]
    pub org: Option<String>,

    /// Skip confirmation prompt
    #[arg(short = 'y', long, default_value_t = false)]
    pub yes: bool,
}

/// Classified tag input: flat string tags vs key=value tag bindings
#[derive(Debug, Default)]
pub struct ClassifiedTags {
    /// Flat string tag names (no '=' sign)
    pub flat_tags: Vec<String>,
    /// Key=value tag bindings
    pub bindings: Vec<(String, String)>,
}

/// Classify tag inputs into flat string tags and key=value bindings.
/// Tags containing '=' are treated as key=value bindings.
/// Tags without '=' are treated as flat string tags.
pub fn classify_tags(tags: &[String]) -> Result<ClassifiedTags, String> {
    let mut result = ClassifiedTags::default();
    for tag in tags {
        if tag.contains('=') {
            let (key, value) = parse_tag(tag)?;
            result.bindings.push((key, value));
        } else {
            let name = tag.trim().to_string();
            if name.is_empty() {
                return Err("Empty tag name".to_string());
            }
            result.flat_tags.push(name);
        }
    }
    Ok(result)
}

/// Parse a "key=value" tag string into (key, value) tuple
pub fn parse_tag(s: &str) -> Result<(String, String), String> {
    match s.split_once('=') {
        Some((key, value)) => {
            let key = key.trim().to_string();
            let value = value.trim().to_string();
            if key.is_empty() {
                return Err(format!("Empty key in tag '{}'", s));
            }
            Ok((key, value))
        }
        None => Err(format!("Invalid tag format '{}'. Expected key=value", s)),
    }
}

/// Parse multiple "key=value" tag strings
pub fn parse_tags(tags: &[String]) -> Result<Vec<(String, String)>, String> {
    tags.iter().map(|t| parse_tag(t)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tag_valid() {
        let (key, value) = parse_tag("env=prod").unwrap();
        assert_eq!(key, "env");
        assert_eq!(value, "prod");
    }

    #[test]
    fn test_parse_tag_with_equals_in_value() {
        let (key, value) = parse_tag("formula=a=b+c").unwrap();
        assert_eq!(key, "formula");
        assert_eq!(value, "a=b+c");
    }

    #[test]
    fn test_parse_tag_empty_value() {
        let (key, value) = parse_tag("key=").unwrap();
        assert_eq!(key, "key");
        assert_eq!(value, "");
    }

    #[test]
    fn test_parse_tag_with_spaces() {
        let (key, value) = parse_tag(" env = prod ").unwrap();
        assert_eq!(key, "env");
        assert_eq!(value, "prod");
    }

    #[test]
    fn test_parse_tag_no_equals() {
        let result = parse_tag("invalid");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Expected key=value"));
    }

    #[test]
    fn test_parse_tag_empty_key() {
        let result = parse_tag("=value");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Empty key"));
    }

    #[test]
    fn test_parse_tags_multiple() {
        let tags = vec!["env=prod".to_string(), "team=backend".to_string()];
        let result = parse_tags(&tags).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], ("env".to_string(), "prod".to_string()));
        assert_eq!(result[1], ("team".to_string(), "backend".to_string()));
    }

    #[test]
    fn test_parse_tags_with_error() {
        let tags = vec!["env=prod".to_string(), "invalid".to_string()];
        let result = parse_tags(&tags);
        assert!(result.is_err());
    }

    // ===== classify_tags tests =====

    #[test]
    fn test_classify_tags_only_flat() {
        let tags = vec!["env".to_string(), "team".to_string()];
        let result = classify_tags(&tags).unwrap();
        assert_eq!(result.flat_tags, vec!["env", "team"]);
        assert!(result.bindings.is_empty());
    }

    #[test]
    fn test_classify_tags_only_bindings() {
        let tags = vec!["env=prod".to_string(), "team=backend".to_string()];
        let result = classify_tags(&tags).unwrap();
        assert!(result.flat_tags.is_empty());
        assert_eq!(result.bindings.len(), 2);
        assert_eq!(result.bindings[0], ("env".to_string(), "prod".to_string()));
        assert_eq!(
            result.bindings[1],
            ("team".to_string(), "backend".to_string())
        );
    }

    #[test]
    fn test_classify_tags_mixed() {
        let tags = vec![
            "flat-tag".to_string(),
            "env=prod".to_string(),
            "another".to_string(),
            "team=backend".to_string(),
        ];
        let result = classify_tags(&tags).unwrap();
        assert_eq!(result.flat_tags, vec!["flat-tag", "another"]);
        assert_eq!(result.bindings.len(), 2);
        assert_eq!(result.bindings[0], ("env".to_string(), "prod".to_string()));
    }

    #[test]
    fn test_classify_tags_empty() {
        let tags: Vec<String> = vec![];
        let result = classify_tags(&tags).unwrap();
        assert!(result.flat_tags.is_empty());
        assert!(result.bindings.is_empty());
    }

    #[test]
    fn test_classify_tags_empty_name_error() {
        let tags = vec!["  ".to_string()];
        let result = classify_tags(&tags);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Empty tag name"));
    }

    #[test]
    fn test_classify_tags_empty_key_error() {
        let tags = vec!["=value".to_string()];
        let result = classify_tags(&tags);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Empty key"));
    }

    #[test]
    fn test_classify_tags_binding_with_equals_in_value() {
        let tags = vec!["formula=a=b+c".to_string()];
        let result = classify_tags(&tags).unwrap();
        assert!(result.flat_tags.is_empty());
        assert_eq!(result.bindings.len(), 1);
        assert_eq!(
            result.bindings[0],
            ("formula".to_string(), "a=b+c".to_string())
        );
    }
}
