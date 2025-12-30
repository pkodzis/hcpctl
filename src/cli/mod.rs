//! CLI argument parsing with kubectl-style subcommands
//!
//! Command structure:
//! - hcpctl get org [NAME]           - list organizations or get one
//! - hcpctl get prj [NAME] -o ORG    - list projects or get one
//! - hcpctl get ws [NAME] -o ORG     - list workspaces or get one

use clap::{Parser, Subcommand, ValueEnum};

use crate::config::defaults;

/// HCP/TFE CLI - Explore HashiCorp Cloud Platform and Terraform Enterprise
#[derive(Parser, Debug)]
#[command(name = "hcpctl")]
#[command(version)]
#[command(about = "Explore HCP Terraform / Terraform Enterprise resources", long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// TFE/HCP host URL (falls back to TFE_HOSTNAME env var or credentials file)
    #[arg(short = 'H', long, global = true)]
    pub host: Option<String>,

    /// API token (overrides env vars and credentials file)
    #[arg(short = 't', long, global = true)]
    pub token: Option<String>,

    /// Log level (error, warn, info, debug, trace)
    #[arg(short, long, global = true, default_value = defaults::LOG_LEVEL)]
    pub log_level: String,

    /// Batch mode - no interactive prompts, no spinners
    #[arg(short, long, global = true, default_value_t = false)]
    pub batch: bool,

    /// Omit header row in table/CSV output
    #[arg(long, global = true, default_value_t = false)]
    pub no_header: bool,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Get resources (organizations, projects, workspaces)
    Get {
        #[command(subcommand)]
        resource: GetResource,
    },
}

/// Resource types for the 'get' command
#[derive(Subcommand, Debug)]
pub enum GetResource {
    /// Get organizations
    #[command(
        visible_alias = "orgs",
        visible_alias = "organization",
        visible_alias = "organizations"
    )]
    Org(OrgArgs),

    /// Get projects
    #[command(
        visible_alias = "prjs",
        visible_alias = "project",
        visible_alias = "projects"
    )]
    Prj(PrjArgs),

    /// Get workspaces
    #[command(visible_alias = "workspace", visible_alias = "workspaces")]
    Ws(WsArgs),
}

/// Arguments for 'get org' subcommand
#[derive(Parser, Debug)]
pub struct OrgArgs {
    /// Organization name (if specified, shows details for that organization)
    pub name: Option<String>,

    /// Filter organizations by name (substring match)
    #[arg(short, long)]
    pub filter: Option<String>,

    /// Output format
    #[arg(short = 'o', long, value_enum, default_value_t = OutputFormat::Table)]
    pub output: OutputFormat,
}

/// Arguments for 'get prj' subcommand
#[derive(Parser, Debug)]
pub struct PrjArgs {
    /// Project name or ID (if specified, shows details for that project)
    pub name: Option<String>,

    /// Organization name (required for single project, optional for list)
    #[arg(long = "org")]
    pub org: Option<String>,

    /// Filter projects by name (substring match)
    #[arg(short, long)]
    pub filter: Option<String>,

    /// Output format
    #[arg(short = 'o', long, value_enum, default_value_t = OutputFormat::Table)]
    pub output: OutputFormat,

    /// Sort results by field
    #[arg(short, long, value_enum, default_value_t = PrjSortField::Name)]
    pub sort: PrjSortField,

    /// Reverse sort order (descending)
    #[arg(short = 'r', long, default_value_t = false)]
    pub reverse: bool,

    /// Disable grouping by organization
    #[arg(long, default_value_t = false)]
    pub no_group_org: bool,

    /// Include workspace information (count, names, IDs)
    #[arg(long, default_value_t = false)]
    pub with_ws: bool,

    /// Show workspace names column (implies --with-ws)
    #[arg(long, default_value_t = false)]
    pub with_ws_names: bool,

    /// Show workspace IDs column (implies --with-ws)
    #[arg(long, default_value_t = false)]
    pub with_ws_ids: bool,

    /// Show workspaces as "name (id)" format (implies --with-ws)
    #[arg(long, default_value_t = false)]
    pub with_ws_details: bool,
}

/// Arguments for 'get ws' subcommand
#[derive(Parser, Debug)]
pub struct WsArgs {
    /// Workspace name or ID (if specified, shows details for that workspace)
    pub name: Option<String>,

    /// Organization name (required for single workspace, optional for list)
    #[arg(long = "org")]
    pub org: Option<String>,

    /// Filter by project (name or ID)
    #[arg(short, long)]
    pub prj: Option<String>,

    /// Filter workspaces by name (substring match)
    #[arg(short, long)]
    pub filter: Option<String>,

    /// Output format
    #[arg(short = 'o', long, value_enum, default_value_t = OutputFormat::Table)]
    pub output: OutputFormat,

    /// Sort results by field
    #[arg(short, long, value_enum, default_value_t = WsSortField::Name)]
    pub sort: WsSortField,

    /// Reverse sort order (descending)
    #[arg(short = 'r', long, default_value_t = false)]
    pub reverse: bool,

    /// Disable grouping by organization
    #[arg(long, default_value_t = false)]
    pub no_group_org: bool,

    /// Enable grouping by project (can be combined with org grouping)
    #[arg(long, default_value_t = false)]
    pub group_by_prj: bool,
}

impl WsArgs {
    /// Check if grouping by org is enabled (default: true, unless --no-group-org)
    pub fn group_by_org(&self) -> bool {
        !self.no_group_org
    }
}

/// Output format options
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// ASCII table (default)
    Table,
    /// Comma-separated values
    Csv,
    /// JSON array
    Json,
}

/// Sort field options for projects
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PrjSortField {
    /// Sort by project name (default)
    Name,
    /// Sort by workspace count
    Workspaces,
}

/// Sort field options for workspaces
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum WsSortField {
    /// Sort by workspace name (default)
    Name,
    /// Sort by resource count
    Resources,
    /// Sort by last update time
    UpdatedAt,
    /// Sort by Terraform version
    TfVersion,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Table => write!(f, "table"),
            OutputFormat::Csv => write!(f, "csv"),
            OutputFormat::Json => write!(f, "json"),
        }
    }
}

impl std::fmt::Display for WsSortField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WsSortField::Name => write!(f, "name"),
            WsSortField::Resources => write!(f, "resources"),
            WsSortField::UpdatedAt => write!(f, "updated-at"),
            WsSortField::TfVersion => write!(f, "tf-version"),
        }
    }
}

impl std::fmt::Display for PrjSortField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrjSortField::Name => write!(f, "name"),
            PrjSortField::Workspaces => write!(f, "workspaces"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_format_display() {
        assert_eq!(OutputFormat::Table.to_string(), "table");
        assert_eq!(OutputFormat::Csv.to_string(), "csv");
        assert_eq!(OutputFormat::Json.to_string(), "json");
    }

    #[test]
    fn test_ws_sort_field_display() {
        assert_eq!(WsSortField::Name.to_string(), "name");
        assert_eq!(WsSortField::Resources.to_string(), "resources");
        assert_eq!(WsSortField::UpdatedAt.to_string(), "updated-at");
        assert_eq!(WsSortField::TfVersion.to_string(), "tf-version");
    }

    #[test]
    fn test_prj_sort_field_display() {
        assert_eq!(PrjSortField::Name.to_string(), "name");
        assert_eq!(PrjSortField::Workspaces.to_string(), "workspaces");
    }

    // === Get org tests ===

    #[test]
    fn test_get_org_list() {
        let cli = Cli::parse_from(["hcp", "get", "org"]);
        match cli.command {
            Command::Get {
                resource: GetResource::Org(args),
            } => {
                assert!(args.name.is_none());
            }
            _ => panic!("Expected Get Org command"),
        }
    }

    #[test]
    fn test_get_org_single() {
        let cli = Cli::parse_from(["hcp", "get", "org", "my-org"]);
        match cli.command {
            Command::Get {
                resource: GetResource::Org(args),
            } => {
                assert_eq!(args.name, Some("my-org".to_string()));
            }
            _ => panic!("Expected Get Org command"),
        }
    }

    #[test]
    fn test_get_org_alias() {
        let cli = Cli::parse_from(["hcp", "get", "orgs"]);
        assert!(matches!(
            cli.command,
            Command::Get {
                resource: GetResource::Org(_)
            }
        ));
    }

    // === Get prj tests ===

    #[test]
    fn test_get_prj_list_all() {
        let cli = Cli::parse_from(["hcp", "get", "prj"]);
        match cli.command {
            Command::Get {
                resource: GetResource::Prj(args),
            } => {
                assert!(args.name.is_none());
                assert!(args.org.is_none());
            }
            _ => panic!("Expected Get Prj command"),
        }
    }

    #[test]
    fn test_get_prj_list_in_org() {
        let cli = Cli::parse_from(["hcp", "get", "prj", "--org", "my-org"]);
        match cli.command {
            Command::Get {
                resource: GetResource::Prj(args),
            } => {
                assert!(args.name.is_none());
                assert_eq!(args.org, Some("my-org".to_string()));
            }
            _ => panic!("Expected Get Prj command"),
        }
    }

    #[test]
    fn test_get_prj_single() {
        let cli = Cli::parse_from(["hcp", "get", "prj", "my-project", "--org", "my-org"]);
        match cli.command {
            Command::Get {
                resource: GetResource::Prj(args),
            } => {
                assert_eq!(args.name, Some("my-project".to_string()));
                assert_eq!(args.org, Some("my-org".to_string()));
            }
            _ => panic!("Expected Get Prj command"),
        }
    }

    #[test]
    fn test_get_prj_alias() {
        let cli = Cli::parse_from(["hcp", "get", "projects"]);
        assert!(matches!(
            cli.command,
            Command::Get {
                resource: GetResource::Prj(_)
            }
        ));
    }

    // === Get ws tests ===

    #[test]
    fn test_get_ws_list_all() {
        let cli = Cli::parse_from(["hcp", "get", "ws"]);
        match cli.command {
            Command::Get {
                resource: GetResource::Ws(args),
            } => {
                assert!(args.name.is_none());
                assert!(args.org.is_none());
            }
            _ => panic!("Expected Get Ws command"),
        }
    }

    #[test]
    fn test_get_ws_list_in_org() {
        let cli = Cli::parse_from(["hcp", "get", "ws", "--org", "my-org"]);
        match cli.command {
            Command::Get {
                resource: GetResource::Ws(args),
            } => {
                assert!(args.name.is_none());
                assert_eq!(args.org, Some("my-org".to_string()));
            }
            _ => panic!("Expected Get Ws command"),
        }
    }

    #[test]
    fn test_get_ws_single() {
        let cli = Cli::parse_from(["hcp", "get", "ws", "my-workspace", "--org", "my-org"]);
        match cli.command {
            Command::Get {
                resource: GetResource::Ws(args),
            } => {
                assert_eq!(args.name, Some("my-workspace".to_string()));
                assert_eq!(args.org, Some("my-org".to_string()));
            }
            _ => panic!("Expected Get Ws command"),
        }
    }

    #[test]
    fn test_get_ws_with_filter() {
        let cli = Cli::parse_from(["hcp", "get", "ws", "-f", "prod"]);
        match cli.command {
            Command::Get {
                resource: GetResource::Ws(args),
            } => {
                assert_eq!(args.filter, Some("prod".to_string()));
            }
            _ => panic!("Expected Get Ws command"),
        }
    }

    #[test]
    fn test_get_ws_with_project() {
        let cli = Cli::parse_from(["hcp", "get", "ws", "--org", "my-org", "-p", "my-project"]);
        match cli.command {
            Command::Get {
                resource: GetResource::Ws(args),
            } => {
                assert_eq!(args.org, Some("my-org".to_string()));
                assert_eq!(args.prj, Some("my-project".to_string()));
            }
            _ => panic!("Expected Get Ws command"),
        }
    }

    #[test]
    fn test_get_ws_grouping_default() {
        let cli = Cli::parse_from(["hcp", "get", "ws"]);
        match cli.command {
            Command::Get {
                resource: GetResource::Ws(args),
            } => {
                assert!(args.group_by_org()); // default: grouped by org
                assert!(!args.group_by_prj); // default: not grouped by prj
            }
            _ => panic!("Expected Get Ws command"),
        }
    }

    #[test]
    fn test_get_ws_grouping_by_prj() {
        let cli = Cli::parse_from(["hcp", "get", "ws", "--group-by-prj"]);
        match cli.command {
            Command::Get {
                resource: GetResource::Ws(args),
            } => {
                assert!(args.group_by_org()); // still grouped by org
                assert!(args.group_by_prj); // also grouped by prj
            }
            _ => panic!("Expected Get Ws command"),
        }
    }

    #[test]
    fn test_get_ws_alias() {
        let cli = Cli::parse_from(["hcp", "get", "workspaces"]);
        assert!(matches!(
            cli.command,
            Command::Get {
                resource: GetResource::Ws(_)
            }
        ));
    }

    // === Global options ===

    #[test]
    fn test_global_options() {
        let cli = Cli::parse_from([
            "hcp",
            "-H",
            "custom.host.com",
            "-t",
            "my-token",
            "-l",
            "debug",
            "get",
            "org",
        ]);
        assert_eq!(cli.host, Some("custom.host.com".to_string()));
        assert_eq!(cli.token, Some("my-token".to_string()));
        assert_eq!(cli.log_level, "debug");
    }

    #[test]
    fn test_host_optional() {
        let cli = Cli::parse_from(["hcp", "get", "org"]);
        assert!(cli.host.is_none());
    }

    #[test]
    fn test_batch_option() {
        let cli = Cli::parse_from(["hcp", "-b", "get", "org"]);
        assert!(cli.batch);
    }

    #[test]
    fn test_no_header_option() {
        let cli = Cli::parse_from(["hcp", "--no-header", "get", "org"]);
        assert!(cli.no_header);
    }

    #[test]
    fn test_output_format_json() {
        let cli = Cli::parse_from(["hcp", "get", "org", "-o", "json"]);
        match cli.command {
            Command::Get {
                resource: GetResource::Org(args),
            } => {
                assert_eq!(args.output, OutputFormat::Json);
            }
            _ => panic!("Expected Get Org command"),
        }
    }
}
