//! CLI argument parsing with kubectl-style subcommands
//!
//! Command structure:
//! - hcpctl get org [NAME]           - list organizations or get one
//! - hcpctl get prj [NAME] -o ORG    - list projects or get one
//! - hcpctl get ws [NAME] -o ORG     - list workspaces or get one

use clap::{builder::ArgPredicate, Parser, Subcommand, ValueEnum};

use crate::config::defaults;

const AFTER_LONG_HELP: &str = r#"HOST RESOLUTION:
  The host is resolved in the following order (first match wins):
  1. CLI argument (-H, --host)
  2. Environment variable: TFE_HOSTNAME
  3. Credentials file (~/.terraform.d/credentials.tfrc.json):
     - If 1 host configured: use it automatically
     - If multiple hosts: interactive selection (or error in batch mode)

TOKEN RESOLUTION:
  The API token is resolved in the following order (first match wins):
  1. CLI argument (-t, --token)
  2. Environment variables (in order): HCP_TOKEN, TFC_TOKEN, TFE_TOKEN
  3. Credentials file (~/.terraform.d/credentials.tfrc.json)
     Token is read from the entry matching the resolved host.

EXAMPLES:
  hcpctl get org                     List all organizations
  hcpctl get ws --org myorg          List workspaces in organization
  hcpctl get ws myws --org myorg     Get workspace details
  hcpctl -H app.terraform.io get ws  Use specific host"#;

/// HCP/TFE CLI - Explore HashiCorp Cloud Platform and Terraform Enterprise
#[derive(Parser, Debug)]
#[command(name = "hcpctl")]
#[command(version)]
#[command(about = "Explore HCP Terraform / Terraform Enterprise resources")]
#[command(after_long_help = AFTER_LONG_HELP)]
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

    /// Get OAuth clients (VCS connections)
    #[command(
        visible_alias = "oauth-client",
        visible_alias = "oauth-clients",
        visible_alias = "oauthclient",
        visible_alias = "oauthclients"
    )]
    Oc(OcArgs),

    /// Get runs (active runs by default - non_final states)
    #[command(visible_alias = "runs")]
    Run(RunArgs),
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

    /// Output format (defaults to yaml when --subresource is used)
    #[arg(
        short = 'o',
        long,
        value_enum,
        default_value_t = OutputFormat::Table,
        default_value_if("subresource", ArgPredicate::IsPresent, "yaml")
    )]
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

    /// Fetch a related subresource (run=current-run, state=current-state-version,
    /// config=current-configuration-version, assessment=current-assessment-result).
    /// Only works with single workspace lookup and JSON/YAML output.
    #[arg(long, value_enum)]
    pub subresource: Option<WsSubresource>,
}

impl WsArgs {
    /// Check if grouping by org is enabled (default: true, unless --no-group-org)
    pub fn group_by_org(&self) -> bool {
        !self.no_group_org
    }
}

/// Arguments for 'get oc' subcommand (OAuth Clients)
#[derive(Parser, Debug)]
pub struct OcArgs {
    /// OAuth client name or ID (if specified, shows details for that client)
    pub name: Option<String>,

    /// Organization name (required for single client, optional for list)
    #[arg(long = "org")]
    pub org: Option<String>,

    /// Filter OAuth clients by name (substring match)
    #[arg(short, long)]
    pub filter: Option<String>,

    /// Output format
    #[arg(short = 'o', long, value_enum, default_value_t = OutputFormat::Table)]
    pub output: OutputFormat,
}

/// Arguments for 'get run' subcommand
///
/// Lists only active (non-final) runs. Use --status to filter by specific statuses.
#[derive(Parser, Debug, Clone)]
#[command(
    after_help = "NOTE: This command shows only active (non-final) runs.\n\
                        Use --status to filter by specific non-final statuses (e.g. planning,applying).\n\
                        Completed runs (applied, errored, canceled) are not shown."
)]
pub struct RunArgs {
    /// Run ID (if specified, shows details for that run)
    pub name: Option<String>,

    /// Organization name (lists runs across org workspaces)
    #[arg(long = "org", conflicts_with = "ws")]
    pub org: Option<String>,

    /// Workspace ID (lists runs for specific workspace, must start with ws-)
    #[arg(long = "ws", conflicts_with = "org")]
    pub ws: Option<String>,

    /// Filter by workspace names (comma-separated, only with --org)
    #[arg(long = "workspace-names", requires = "org")]
    pub workspace_names: Option<String>,

    /// Filter by specific non-final run statuses (comma-separated).
    /// Valid values: pending, fetching, queuing, plan_queued, planning, planned,
    /// cost_estimating, cost_estimated, policy_checking, policy_override,
    /// policy_soft_failed, policy_checked, confirmed, post_plan_running,
    /// post_plan_completed, applying, apply_queued
    #[arg(long)]
    pub status: Option<String>,

    /// Output format
    #[arg(short = 'o', long, value_enum, default_value_t = OutputFormat::Table)]
    pub output: OutputFormat,

    /// Fetch a related subresource (events, plan, apply). Requires run ID.
    #[arg(long, value_enum, requires = "name")]
    pub subresource: Option<RunSubresource>,

    /// Download and display the full log (requires --subresource plan or apply)
    #[arg(long, default_value_t = false)]
    pub get_log: bool,

    /// Tail the log in real-time until completion (requires --subresource plan or apply)
    #[arg(long, default_value_t = false, conflicts_with = "get_log")]
    pub tail_log: bool,

    /// Output raw log without parsing (default: extract @message from JSON lines)
    #[arg(long, default_value_t = false)]
    pub raw: bool,

    /// Sort results by field (default: created-at, newest first)
    #[arg(short, long, value_enum, default_value_t = RunSortField::CreatedAt)]
    pub sort: RunSortField,

    /// Reverse sort order
    #[arg(short = 'r', long, default_value_t = false)]
    pub reverse: bool,

    /// Skip confirmation prompt when results exceed 100
    #[arg(short = 'y', long, default_value_t = false)]
    pub yes: bool,
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
    /// YAML format
    Yaml,
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

/// Sort field options for runs
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum RunSortField {
    /// Sort by creation time (default: newest first)
    #[default]
    CreatedAt,
    /// Sort by status
    Status,
    /// Sort by workspace ID
    #[value(name = "ws-id")]
    WsId,
}

/// Run subresources that can be fetched
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RunSubresource {
    /// Run events (run-events)
    Events,
    /// Plan details with log access
    Plan,
    /// Apply details with log access
    Apply,
}

/// Workspace subresources that can be fetched
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum WsSubresource {
    /// Current run (current-run)
    Run,
    /// Current state version (current-state-version)
    State,
    /// Current configuration version (current-configuration-version)
    Config,
    /// Current assessment result (current-assessment-result)
    Assessment,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Table => write!(f, "table"),
            OutputFormat::Csv => write!(f, "csv"),
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::Yaml => write!(f, "yaml"),
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

impl std::fmt::Display for RunSortField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunSortField::CreatedAt => write!(f, "created-at"),
            RunSortField::Status => write!(f, "status"),
            RunSortField::WsId => write!(f, "ws-id"),
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
        assert_eq!(OutputFormat::Yaml.to_string(), "yaml");
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

    // === Get run tests ===

    #[test]
    fn test_get_run_single() {
        let cli = Cli::parse_from(["hcp", "get", "run", "run-abc123"]);
        match cli.command {
            Command::Get {
                resource: GetResource::Run(args),
            } => {
                assert_eq!(args.name, Some("run-abc123".to_string()));
                assert!(args.org.is_none());
                assert!(args.ws.is_none());
            }
            _ => panic!("Expected Get Run command"),
        }
    }

    #[test]
    fn test_get_run_with_org() {
        let cli = Cli::parse_from(["hcp", "get", "run", "--org", "my-org"]);
        match cli.command {
            Command::Get {
                resource: GetResource::Run(args),
            } => {
                assert!(args.name.is_none());
                assert_eq!(args.org, Some("my-org".to_string()));
                assert!(args.ws.is_none());
            }
            _ => panic!("Expected Get Run command"),
        }
    }

    #[test]
    fn test_get_run_with_ws() {
        let cli = Cli::parse_from(["hcp", "get", "run", "--ws", "ws-abc123"]);
        match cli.command {
            Command::Get {
                resource: GetResource::Run(args),
            } => {
                assert!(args.name.is_none());
                assert!(args.org.is_none());
                assert_eq!(args.ws, Some("ws-abc123".to_string()));
            }
            _ => panic!("Expected Get Run command"),
        }
    }

    #[test]
    fn test_get_run_with_subresource() {
        let cli = Cli::parse_from(["hcp", "get", "run", "run-abc123", "--subresource", "events"]);
        match cli.command {
            Command::Get {
                resource: GetResource::Run(args),
            } => {
                assert_eq!(args.name, Some("run-abc123".to_string()));
                assert_eq!(args.subresource, Some(RunSubresource::Events));
            }
            _ => panic!("Expected Get Run command"),
        }
    }

    #[test]
    fn test_get_run_with_subresource_plan() {
        let cli = Cli::parse_from(["hcp", "get", "run", "run-abc123", "--subresource", "plan"]);
        match cli.command {
            Command::Get {
                resource: GetResource::Run(args),
            } => {
                assert_eq!(args.name, Some("run-abc123".to_string()));
                assert_eq!(args.subresource, Some(RunSubresource::Plan));
                assert!(!args.get_log);
                assert!(!args.tail_log);
            }
            _ => panic!("Expected Get Run command"),
        }
    }

    #[test]
    fn test_get_run_with_subresource_apply() {
        let cli = Cli::parse_from(["hcp", "get", "run", "run-abc123", "--subresource", "apply"]);
        match cli.command {
            Command::Get {
                resource: GetResource::Run(args),
            } => {
                assert_eq!(args.name, Some("run-abc123".to_string()));
                assert_eq!(args.subresource, Some(RunSubresource::Apply));
            }
            _ => panic!("Expected Get Run command"),
        }
    }

    #[test]
    fn test_get_run_with_get_log() {
        let cli = Cli::parse_from([
            "hcp",
            "get",
            "run",
            "run-abc123",
            "--subresource",
            "plan",
            "--get-log",
        ]);
        match cli.command {
            Command::Get {
                resource: GetResource::Run(args),
            } => {
                assert_eq!(args.subresource, Some(RunSubresource::Plan));
                assert!(args.get_log);
                assert!(!args.tail_log);
            }
            _ => panic!("Expected Get Run command"),
        }
    }

    #[test]
    fn test_get_run_with_tail_log() {
        let cli = Cli::parse_from([
            "hcp",
            "get",
            "run",
            "run-abc123",
            "--subresource",
            "apply",
            "--tail-log",
        ]);
        match cli.command {
            Command::Get {
                resource: GetResource::Run(args),
            } => {
                assert_eq!(args.subresource, Some(RunSubresource::Apply));
                assert!(!args.get_log);
                assert!(args.tail_log);
                assert!(!args.raw);
            }
            _ => panic!("Expected Get Run command"),
        }
    }

    #[test]
    fn test_get_run_with_raw_log() {
        let cli = Cli::parse_from([
            "hcp",
            "get",
            "run",
            "run-abc123",
            "--subresource",
            "plan",
            "--get-log",
            "--raw",
        ]);
        match cli.command {
            Command::Get {
                resource: GetResource::Run(args),
            } => {
                assert!(args.get_log);
                assert!(args.raw);
            }
            _ => panic!("Expected Get Run command"),
        }
    }

    #[test]
    fn test_get_run_with_status_filter() {
        let cli = Cli::parse_from([
            "hcp",
            "get",
            "run",
            "--org",
            "my-org",
            "--status",
            "planning,applying",
        ]);
        match cli.command {
            Command::Get {
                resource: GetResource::Run(args),
            } => {
                assert_eq!(args.status, Some("planning,applying".to_string()));
            }
            _ => panic!("Expected Get Run command"),
        }
    }

    #[test]
    fn test_get_run_sort_options() {
        let cli = Cli::parse_from(["hcp", "get", "run", "--org", "my-org", "-s", "ws-id"]);
        match cli.command {
            Command::Get {
                resource: GetResource::Run(args),
            } => {
                assert_eq!(args.sort, RunSortField::WsId);
            }
            _ => panic!("Expected Get Run command"),
        }
    }

    #[test]
    fn test_get_run_yes_flag() {
        let cli = Cli::parse_from(["hcp", "get", "run", "--org", "my-org", "-y"]);
        match cli.command {
            Command::Get {
                resource: GetResource::Run(args),
            } => {
                assert!(args.yes);
            }
            _ => panic!("Expected Get Run command"),
        }
    }

    #[test]
    fn test_get_run_alias() {
        let cli = Cli::parse_from(["hcp", "get", "runs"]);
        assert!(matches!(
            cli.command,
            Command::Get {
                resource: GetResource::Run(_)
            }
        ));
    }

    #[test]
    fn test_run_sort_field_display() {
        assert_eq!(RunSortField::CreatedAt.to_string(), "created-at");
        assert_eq!(RunSortField::Status.to_string(), "status");
        assert_eq!(RunSortField::WsId.to_string(), "ws-id");
    }
}
