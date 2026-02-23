//! CLI argument parsing with kubectl-style subcommands
//!
//! Command structure:
//! - hcpctl get org [NAME]           - list organizations or get one
//! - hcpctl get prj [NAME] -o ORG    - list projects or get one
//! - hcpctl get ws [NAME] -o ORG     - list workspaces or get one
//! - hcpctl purge state <ws-id>      - purge all resources from workspace state
//! - hcpctl download config <ws>     - download workspace configuration

mod common;
mod context;
mod delete;
mod download;
mod enums;
mod get;
mod invite;
mod logs;
mod purge;
mod set;
mod tag;
mod watch;

use clap::{Parser, Subcommand};

use crate::config::defaults;

// Re-export all types for public API
pub use common::OutputFormat;
pub use context::{ConfigAction, DeleteContextArgs, SetContextArgs, UseContextArgs};
pub use delete::{DeleteOrgMemberArgs, DeleteResource};
pub use download::{DownloadConfigArgs, DownloadResource};
pub use enums::{PrjSortField, RunSortField, RunSubresource, WsSortField, WsSubresource};
pub use get::{GetResource, OcArgs, OrgArgs, OrgMemberArgs, PrjArgs, RunArgs, TeamArgs, WsArgs};
pub use invite::InviteArgs;
pub use logs::LogsArgs;
pub use purge::{PurgeResource, PurgeRunArgs, PurgeStateArgs};
pub use set::{SetResource, SetWsArgs};
pub use tag::{
    classify_tags, parse_tags, DeleteTagPrjArgs, DeleteTagResource, DeleteTagWsArgs, GetTagArgs,
    GetTagPrjArgs, GetTagResource, GetTagWsArgs, SetTagPrjArgs, SetTagResource, SetTagWsArgs,
};
pub use watch::{WatchResource, WatchWsArgs};

const AFTER_LONG_HELP: &str = r#"HOST RESOLUTION:

The host is resolved in the following order (first match wins):

  1. CLI argument (-H, --host)
  2. Environment variable: TFE_HOSTNAME
  3. Active context (from --context, HCPCTL_CONTEXT env, or current-context)
  4. Credentials file (~/.terraform.d/credentials.tfrc.json):
     - If 1 host configured: use it automatically
     - If multiple hosts: interactive selection (or error in batch mode)

TOKEN RESOLUTION:

The API token is resolved in the following order (first match wins):

  1. CLI argument (-t, --token)
  2. Environment variables (in order): HCP_TOKEN, TFC_TOKEN, TFE_TOKEN
  3. Active context
  4. Credentials file (~/.terraform.d/credentials.tfrc.json)
     Token is read from the entry matching the resolved host.

CONTEXT:

  Contexts store connection defaults (host, token, org) for quick switching:

    - hcpctl config set-context prod --host app.terraform.io --org my-org
    - hcpctl config use-context prod

  Resolution (first match wins):

    - Host:  -H flag → TFE_HOSTNAME env → context → credentials file
    - Token: -t flag → HCP_TOKEN/TFC_TOKEN/TFE_TOKEN env → context → credentials file
    - Org:   --org flag → context

EXAMPLES:

  - hcpctl get org                     # List all organizations
  - hcpctl get ws --org myorg          # List workspaces in organization
  - hcpctl get ws myws --org myorg     # Get workspace details
  - hcpctl -H app.terraform.io get ws  # Use specific host
  - hcpctl -c prod get ws              # Use 'prod' context"#;

/// HCP/TFE CLI - Explore HashiCorp Cloud Platform and Terraform Enterprise
#[derive(Parser, Debug)]
#[command(name = "hcpctl")]
#[command(version)]
#[command(about = "Explore HCP Terraform / Terraform Enterprise resources")]
#[command(after_help = AFTER_LONG_HELP)]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Use a specific named context (overrides current-context)
    #[arg(short = 'c', long, global = true)]
    pub context: Option<String>,

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

    /// Generate Markdown documentation for all commands (hidden)
    #[arg(long, hide = true)]
    pub markdown_help: bool,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Get resources (organizations, projects, workspaces)
    Get {
        #[command(subcommand)]
        resource: GetResource,
    },

    /// Delete resources
    Delete {
        #[command(subcommand)]
        resource: DeleteResource,
    },

    /// Purge resources (destructive operations with mandatory confirmation)
    ///
    /// These operations are IRREVERSIBLE and always require interactive confirmation.
    /// The --batch flag is ignored for purge commands.
    #[command(verbatim_doc_comment)]
    Purge {
        #[command(subcommand)]
        resource: PurgeResource,
    },

    /// Download resources (configuration files, etc.)
    Download {
        #[command(subcommand)]
        resource: DownloadResource,
    },

    /// View logs for a run (plan or apply)
    ///
    /// Target can be:
    ///   run-xxx  Run ID - directly fetches logs for that run
    ///   ws-xxx   Workspace ID - fetches current-run logs
    ///   name     Workspace name - fetches current-run logs (requires --org)
    #[command(visible_alias = "log", verbatim_doc_comment)]
    Logs(LogsArgs),

    /// Watch resources for changes
    Watch {
        #[command(subcommand)]
        resource: WatchResource,
    },

    /// Invite a user to an organization
    Invite(InviteArgs),

    /// Set resource properties (assign workspace to project, etc.)
    Set {
        #[command(subcommand)]
        resource: SetResource,
    },

    /// Manage connection contexts for multiple TFE/HCP instances
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Update hcpctl to the latest version
    Update,
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

    #[test]
    fn test_logs_command_with_run_id() {
        let cli = Cli::parse_from(["hcp", "logs", "run-abc123"]);
        match cli.command {
            Command::Logs(args) => {
                assert_eq!(args.target, "run-abc123");
                assert!(!args.apply);
                assert!(!args.follow);
                assert!(!args.raw);
                assert!(args.org.is_none());
            }
            _ => panic!("Expected Logs command"),
        }
    }

    #[test]
    fn test_logs_command_with_workspace_name() {
        let cli = Cli::parse_from(["hcp", "logs", "my-workspace", "-O", "my-org"]);
        match cli.command {
            Command::Logs(args) => {
                assert_eq!(args.target, "my-workspace");
                assert_eq!(args.org, Some("my-org".to_string()));
            }
            _ => panic!("Expected Logs command"),
        }
    }

    #[test]
    fn test_logs_command_with_apply_flag() {
        let cli = Cli::parse_from(["hcp", "logs", "run-abc123", "--apply"]);
        match cli.command {
            Command::Logs(args) => {
                assert!(args.apply);
            }
            _ => panic!("Expected Logs command"),
        }
    }

    #[test]
    fn test_logs_command_with_follow_flag() {
        let cli = Cli::parse_from(["hcp", "logs", "run-abc123", "-f"]);
        match cli.command {
            Command::Logs(args) => {
                assert!(args.follow);
            }
            _ => panic!("Expected Logs command"),
        }
    }

    #[test]
    fn test_logs_command_with_raw_flag() {
        let cli = Cli::parse_from(["hcp", "logs", "run-abc123", "--raw"]);
        match cli.command {
            Command::Logs(args) => {
                assert!(args.raw);
            }
            _ => panic!("Expected Logs command"),
        }
    }

    #[test]
    fn test_logs_command_all_options() {
        let cli = Cli::parse_from(["hcp", "logs", "my-ws", "-O", "my-org", "-a", "-f", "--raw"]);
        match cli.command {
            Command::Logs(args) => {
                assert_eq!(args.target, "my-ws");
                assert_eq!(args.org, Some("my-org".to_string()));
                assert!(args.apply);
                assert!(args.follow);
                assert!(args.raw);
            }
            _ => panic!("Expected Logs command"),
        }
    }

    #[test]
    fn test_logs_alias() {
        let cli = Cli::parse_from(["hcp", "log", "run-abc123"]);
        assert!(matches!(cli.command, Command::Logs(_)));
    }

    #[test]
    fn test_purge_state_parses_workspace_id() {
        let cli = Cli::parse_from(["hcp", "purge", "state", "ws-abc123"]);
        match cli.command {
            Command::Purge {
                resource: PurgeResource::State(args),
            } => {
                assert_eq!(args.workspace_id, "ws-abc123");
                assert!(!args.my_resume_is_updated);
            }
            _ => panic!("Expected Purge State command"),
        }
    }

    #[test]
    fn test_purge_state_my_resume_is_updated_flag() {
        let cli = Cli::parse_from([
            "hcp",
            "purge",
            "state",
            "ws-abc123",
            "--my-resume-is-updated",
        ]);
        match cli.command {
            Command::Purge {
                resource: PurgeResource::State(args),
            } => {
                assert_eq!(args.workspace_id, "ws-abc123");
                assert!(args.my_resume_is_updated);
            }
            _ => panic!("Expected Purge State command"),
        }
    }

    #[test]
    fn test_purge_state_my_resume_is_updated_defaults_to_false() {
        let cli = Cli::parse_from(["hcp", "purge", "state", "ws-test"]);
        match cli.command {
            Command::Purge {
                resource: PurgeResource::State(args),
            } => {
                assert!(!args.my_resume_is_updated);
            }
            _ => panic!("Expected Purge State command"),
        }
    }

    // === Set ws tests ===

    #[test]
    fn test_set_ws_with_ids() {
        let cli = Cli::parse_from(["hcp", "set", "ws", "ws-abc123", "--prj", "prj-xyz789"]);
        match cli.command {
            Command::Set {
                resource: SetResource::Ws(args),
            } => {
                assert_eq!(args.workspace, "ws-abc123");
                assert_eq!(args.project, "prj-xyz789");
                assert!(args.org.is_none());
                assert!(!args.yes);
            }
            _ => panic!("Expected Set Ws command"),
        }
    }

    #[test]
    fn test_set_ws_with_names() {
        let cli = Cli::parse_from([
            "hcp",
            "set",
            "ws",
            "my-workspace",
            "--prj",
            "my-project",
            "--org",
            "my-org",
        ]);
        match cli.command {
            Command::Set {
                resource: SetResource::Ws(args),
            } => {
                assert_eq!(args.workspace, "my-workspace");
                assert_eq!(args.project, "my-project");
                assert_eq!(args.org, Some("my-org".to_string()));
                assert!(!args.yes);
            }
            _ => panic!("Expected Set Ws command"),
        }
    }

    #[test]
    fn test_set_ws_with_yes_flag() {
        let cli = Cli::parse_from(["hcp", "set", "ws", "ws-abc123", "--prj", "prj-xyz789", "-y"]);
        match cli.command {
            Command::Set {
                resource: SetResource::Ws(args),
            } => {
                assert!(args.yes);
            }
            _ => panic!("Expected Set Ws command"),
        }
    }

    #[test]
    fn test_set_ws_with_yes_long_flag() {
        let cli = Cli::parse_from([
            "hcp",
            "set",
            "ws",
            "ws-abc123",
            "--prj",
            "prj-xyz789",
            "--yes",
        ]);
        match cli.command {
            Command::Set {
                resource: SetResource::Ws(args),
            } => {
                assert!(args.yes);
            }
            _ => panic!("Expected Set Ws command"),
        }
    }

    #[test]
    fn test_set_ws_short_prj_flag() {
        let cli = Cli::parse_from(["hcp", "set", "ws", "ws-abc123", "-p", "prj-xyz789"]);
        match cli.command {
            Command::Set {
                resource: SetResource::Ws(args),
            } => {
                assert_eq!(args.project, "prj-xyz789");
            }
            _ => panic!("Expected Set Ws command"),
        }
    }

    #[test]
    fn test_set_ws_alias_workspace() {
        let cli = Cli::parse_from([
            "hcp",
            "set",
            "workspace",
            "ws-abc123",
            "--prj",
            "prj-xyz789",
        ]);
        assert!(matches!(
            cli.command,
            Command::Set {
                resource: SetResource::Ws(_)
            }
        ));
    }

    #[test]
    fn test_set_ws_alias_workspaces() {
        let cli = Cli::parse_from([
            "hcp",
            "set",
            "workspaces",
            "ws-abc123",
            "--prj",
            "prj-xyz789",
        ]);
        assert!(matches!(
            cli.command,
            Command::Set {
                resource: SetResource::Ws(_)
            }
        ));
    }

    #[test]
    fn test_set_ws_requires_workspace() {
        let result = Cli::try_parse_from(["hcp", "set", "ws", "--prj", "prj-xyz789"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_ws_requires_prj() {
        let result = Cli::try_parse_from(["hcp", "set", "ws", "ws-abc123"]);
        assert!(result.is_err());
    }

    // === Set tag tests ===

    #[test]
    fn test_set_tag_ws() {
        let cli = Cli::parse_from([
            "hcp",
            "set",
            "tag",
            "ws",
            "ws-abc123",
            "env=prod",
            "team=backend",
        ]);
        match cli.command {
            Command::Set {
                resource:
                    SetResource::Tag {
                        resource: tag::SetTagResource::Ws(args),
                    },
            } => {
                assert_eq!(args.workspace, "ws-abc123");
                assert_eq!(args.tags, vec!["env=prod", "team=backend"]);
                assert!(args.org.is_none());
                assert!(!args.yes);
            }
            _ => panic!("Expected Set Tag Ws command"),
        }
    }

    #[test]
    fn test_set_tag_ws_with_org_and_yes() {
        let cli = Cli::parse_from([
            "hcp",
            "set",
            "tag",
            "ws",
            "my-workspace",
            "env=prod",
            "--org",
            "my-org",
            "-y",
        ]);
        match cli.command {
            Command::Set {
                resource:
                    SetResource::Tag {
                        resource: tag::SetTagResource::Ws(args),
                    },
            } => {
                assert_eq!(args.workspace, "my-workspace");
                assert_eq!(args.tags, vec!["env=prod"]);
                assert_eq!(args.org, Some("my-org".to_string()));
                assert!(args.yes);
            }
            _ => panic!("Expected Set Tag Ws command"),
        }
    }

    #[test]
    fn test_set_tag_prj() {
        let cli = Cli::parse_from([
            "hcp",
            "set",
            "tag",
            "prj",
            "my-project",
            "env=staging",
            "--org",
            "my-org",
        ]);
        match cli.command {
            Command::Set {
                resource:
                    SetResource::Tag {
                        resource: tag::SetTagResource::Prj(args),
                    },
            } => {
                assert_eq!(args.project, "my-project");
                assert_eq!(args.tags, vec!["env=staging"]);
                assert_eq!(args.org, Some("my-org".to_string()));
            }
            _ => panic!("Expected Set Tag Prj command"),
        }
    }

    #[test]
    fn test_set_tag_ws_requires_tags() {
        let result = Cli::try_parse_from(["hcp", "set", "tag", "ws", "ws-abc123"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_tag_alias() {
        let cli = Cli::parse_from(["hcp", "set", "tags", "ws", "ws-abc123", "env=prod"]);
        assert!(matches!(
            cli.command,
            Command::Set {
                resource: SetResource::Tag { .. }
            }
        ));
    }

    #[test]
    fn test_set_tag_ws_alias_workspace() {
        let cli = Cli::parse_from(["hcp", "set", "tag", "workspace", "ws-abc123", "env=prod"]);
        assert!(matches!(
            cli.command,
            Command::Set {
                resource: SetResource::Tag {
                    resource: tag::SetTagResource::Ws(_)
                }
            }
        ));
    }

    // === Get tag tests ===

    #[test]
    fn test_get_tag_ws() {
        let cli = Cli::parse_from(["hcp", "get", "tag", "ws", "ws-abc123"]);
        match cli.command {
            Command::Get {
                resource: GetResource::Tag(ref tag_args),
            } => {
                match &tag_args.resource {
                    Some(tag::GetTagResource::Ws(args)) => {
                        assert_eq!(args.workspace, "ws-abc123");
                    }
                    _ => panic!("Expected Ws subcommand"),
                }
                assert!(tag_args.org.is_none());
                assert_eq!(tag_args.output, OutputFormat::Table);
            }
            _ => panic!("Expected Get Tag Ws command"),
        }
    }

    #[test]
    fn test_get_tag_ws_with_org_and_format() {
        let cli = Cli::parse_from([
            "hcp",
            "get",
            "tag",
            "ws",
            "my-workspace",
            "--org",
            "my-org",
            "-o",
            "json",
        ]);
        match cli.command {
            Command::Get {
                resource: GetResource::Tag(ref tag_args),
            } => {
                match &tag_args.resource {
                    Some(tag::GetTagResource::Ws(args)) => {
                        assert_eq!(args.workspace, "my-workspace");
                    }
                    _ => panic!("Expected Ws subcommand"),
                }
                assert_eq!(tag_args.org, Some("my-org".to_string()));
                assert_eq!(tag_args.output, OutputFormat::Json);
            }
            _ => panic!("Expected Get Tag Ws command"),
        }
    }

    #[test]
    fn test_get_tag_prj() {
        let cli = Cli::parse_from(["hcp", "get", "tag", "prj", "prj-abc123", "--org", "my-org"]);
        match cli.command {
            Command::Get {
                resource: GetResource::Tag(ref tag_args),
            } => {
                match &tag_args.resource {
                    Some(tag::GetTagResource::Prj(args)) => {
                        assert_eq!(args.project, "prj-abc123");
                    }
                    _ => panic!("Expected Prj subcommand"),
                }
                assert_eq!(tag_args.org, Some("my-org".to_string()));
            }
            _ => panic!("Expected Get Tag Prj command"),
        }
    }

    #[test]
    fn test_get_tag_org_level() {
        let cli = Cli::parse_from(["hcp", "get", "tag", "--org", "my-org"]);
        match cli.command {
            Command::Get {
                resource: GetResource::Tag(ref tag_args),
            } => {
                assert!(tag_args.resource.is_none());
                assert_eq!(tag_args.org, Some("my-org".to_string()));
                assert_eq!(tag_args.output, OutputFormat::Table);
            }
            _ => panic!("Expected Get Tag command"),
        }
    }

    #[test]
    fn test_get_tag_org_level_with_filter() {
        let cli = Cli::parse_from([
            "hcp", "get", "tag", "--org", "my-org", "-f", "env", "-o", "json",
        ]);
        match cli.command {
            Command::Get {
                resource: GetResource::Tag(ref tag_args),
            } => {
                assert!(tag_args.resource.is_none());
                assert_eq!(tag_args.org, Some("my-org".to_string()));
                assert_eq!(tag_args.filter, Some("env".to_string()));
                assert_eq!(tag_args.output, OutputFormat::Json);
            }
            _ => panic!("Expected Get Tag command"),
        }
    }

    #[test]
    fn test_get_tag_by_name() {
        let cli = Cli::parse_from(["hcp", "get", "tag", "model__env", "--org", "my-org"]);
        match cli.command {
            Command::Get {
                resource: GetResource::Tag(ref tag_args),
            } => {
                assert!(tag_args.resource.is_none());
                assert_eq!(tag_args.name, Some("model__env".to_string()));
                assert_eq!(tag_args.org, Some("my-org".to_string()));
            }
            _ => panic!("Expected Get Tag command"),
        }
    }

    #[test]
    fn test_get_tag_by_name_with_format() {
        let cli = Cli::parse_from(["hcp", "get", "tag", "env", "--org", "dev", "-o", "json"]);
        match cli.command {
            Command::Get {
                resource: GetResource::Tag(ref tag_args),
            } => {
                assert!(tag_args.resource.is_none());
                assert_eq!(tag_args.name, Some("env".to_string()));
                assert_eq!(tag_args.org, Some("dev".to_string()));
                assert_eq!(tag_args.output, OutputFormat::Json);
            }
            _ => panic!("Expected Get Tag command"),
        }
    }

    #[test]
    fn test_get_tag_alias() {
        let cli = Cli::parse_from(["hcp", "get", "tags", "ws", "ws-abc123"]);
        assert!(matches!(
            cli.command,
            Command::Get {
                resource: GetResource::Tag(_)
            }
        ));
    }

    #[test]
    fn test_get_tag_ws_requires_workspace() {
        let result = Cli::try_parse_from(["hcp", "get", "tag", "ws"]);
        assert!(result.is_err());
    }

    // === Delete tag tests ===

    #[test]
    fn test_delete_tag_ws() {
        let cli = Cli::parse_from(["hcp", "delete", "tag", "ws", "ws-abc123", "env", "team"]);
        match cli.command {
            Command::Delete {
                resource:
                    DeleteResource::Tag {
                        resource: tag::DeleteTagResource::Ws(args),
                    },
            } => {
                assert_eq!(args.workspace, "ws-abc123");
                assert_eq!(args.keys, vec!["env", "team"]);
                assert!(args.org.is_none());
                assert!(!args.yes);
            }
            _ => panic!("Expected Delete Tag Ws command"),
        }
    }

    #[test]
    fn test_delete_tag_ws_with_org_and_yes() {
        let cli = Cli::parse_from([
            "hcp",
            "delete",
            "tag",
            "ws",
            "my-workspace",
            "env",
            "--org",
            "my-org",
            "-y",
        ]);
        match cli.command {
            Command::Delete {
                resource:
                    DeleteResource::Tag {
                        resource: tag::DeleteTagResource::Ws(args),
                    },
            } => {
                assert_eq!(args.workspace, "my-workspace");
                assert_eq!(args.keys, vec!["env"]);
                assert_eq!(args.org, Some("my-org".to_string()));
                assert!(args.yes);
            }
            _ => panic!("Expected Delete Tag Ws command"),
        }
    }

    #[test]
    fn test_delete_tag_prj() {
        let cli = Cli::parse_from([
            "hcp",
            "delete",
            "tag",
            "prj",
            "my-project",
            "env",
            "--org",
            "my-org",
        ]);
        match cli.command {
            Command::Delete {
                resource:
                    DeleteResource::Tag {
                        resource: tag::DeleteTagResource::Prj(args),
                    },
            } => {
                assert_eq!(args.project, "my-project");
                assert_eq!(args.keys, vec!["env"]);
                assert_eq!(args.org, Some("my-org".to_string()));
            }
            _ => panic!("Expected Delete Tag Prj command"),
        }
    }

    #[test]
    fn test_delete_tag_alias() {
        let cli = Cli::parse_from(["hcp", "delete", "tags", "ws", "ws-abc123", "env"]);
        assert!(matches!(
            cli.command,
            Command::Delete {
                resource: DeleteResource::Tag { .. }
            }
        ));
    }

    #[test]
    fn test_delete_tag_ws_requires_keys() {
        let result = Cli::try_parse_from(["hcp", "delete", "tag", "ws", "ws-abc123"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_tag_prj_requires_keys() {
        let result = Cli::try_parse_from(["hcp", "delete", "tag", "prj", "prj-abc123"]);
        assert!(result.is_err());
    }

    // === Config tests (kubectl-style) ===

    #[test]
    fn test_config_get_contexts() {
        let cli = Cli::parse_from(["hcp", "config", "get-contexts"]);
        assert!(matches!(
            cli.command,
            Command::Config {
                action: ConfigAction::GetContexts
            }
        ));
    }

    #[test]
    fn test_config_set_context_with_host() {
        let cli = Cli::parse_from([
            "hcp",
            "config",
            "set-context",
            "prod",
            "--host",
            "app.terraform.io",
        ]);
        match cli.command {
            Command::Config {
                action: ConfigAction::SetContext(args),
            } => {
                assert_eq!(args.name, "prod");
                assert_eq!(args.host, Some("app.terraform.io".to_string()));
                assert!(args.token.is_none());
                assert!(args.org.is_none());
            }
            _ => panic!("Expected Config SetContext command"),
        }
    }

    #[test]
    fn test_config_set_context_all_flags() {
        let cli = Cli::parse_from([
            "hcp",
            "config",
            "set-context",
            "dev",
            "--host",
            "tfe.dev.com",
            "--token",
            "my-token",
            "--org",
            "my-org",
        ]);
        match cli.command {
            Command::Config {
                action: ConfigAction::SetContext(args),
            } => {
                assert_eq!(args.name, "dev");
                assert_eq!(args.host, Some("tfe.dev.com".to_string()));
                assert_eq!(args.token, Some("my-token".to_string()));
                assert_eq!(args.org, Some("my-org".to_string()));
            }
            _ => panic!("Expected Config SetContext command"),
        }
    }

    #[test]
    fn test_config_use_context() {
        let cli = Cli::parse_from(["hcp", "config", "use-context", "prod"]);
        match cli.command {
            Command::Config {
                action: ConfigAction::UseContext(args),
            } => {
                assert_eq!(args.name, "prod");
            }
            _ => panic!("Expected Config UseContext command"),
        }
    }

    #[test]
    fn test_config_delete_context() {
        let cli = Cli::parse_from(["hcp", "config", "delete-context", "old"]);
        match cli.command {
            Command::Config {
                action: ConfigAction::DeleteContext(args),
            } => {
                assert_eq!(args.name, "old");
            }
            _ => panic!("Expected Config DeleteContext command"),
        }
    }

    #[test]
    fn test_config_current_context() {
        let cli = Cli::parse_from(["hcp", "config", "current-context"]);
        assert!(matches!(
            cli.command,
            Command::Config {
                action: ConfigAction::CurrentContext
            }
        ));
    }

    #[test]
    fn test_config_view() {
        let cli = Cli::parse_from(["hcp", "config", "view"]);
        assert!(matches!(
            cli.command,
            Command::Config {
                action: ConfigAction::View
            }
        ));
    }

    #[test]
    fn test_global_context_flag() {
        let cli = Cli::parse_from(["hcp", "--context", "prod", "get", "org"]);
        assert_eq!(cli.context, Some("prod".to_string()));
    }

    #[test]
    fn test_global_context_short_flag() {
        let cli = Cli::parse_from(["hcp", "-c", "prod", "get", "org"]);
        assert_eq!(cli.context, Some("prod".to_string()));
    }

    #[test]
    fn test_config_set_context_requires_name() {
        let result = Cli::try_parse_from(["hcp", "config", "set-context"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_use_context_requires_name() {
        let result = Cli::try_parse_from(["hcp", "config", "use-context"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_delete_context_requires_name() {
        let result = Cli::try_parse_from(["hcp", "config", "delete-context"]);
        assert!(result.is_err());
    }
}
