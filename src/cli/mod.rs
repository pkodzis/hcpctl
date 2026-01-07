//! CLI argument parsing with kubectl-style subcommands
//!
//! Command structure:
//! - hcpctl get org [NAME]           - list organizations or get one
//! - hcpctl get prj [NAME] -o ORG    - list projects or get one
//! - hcpctl get ws [NAME] -o ORG     - list workspaces or get one

mod common;
mod delete;
mod enums;
mod get;
mod invite;
mod logs;
mod watch;

use clap::{Parser, Subcommand};

use crate::config::defaults;

// Re-export all types for public API
pub use common::OutputFormat;
pub use delete::{DeleteOrgMemberArgs, DeleteResource};
pub use enums::{PrjSortField, RunSortField, RunSubresource, WsSortField, WsSubresource};
pub use get::{GetResource, OcArgs, OrgArgs, OrgMemberArgs, PrjArgs, RunArgs, TeamArgs, WsArgs};
pub use invite::InviteArgs;
pub use logs::LogsArgs;
pub use watch::{WatchResource, WatchWsArgs};

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

    /// Delete resources
    Delete {
        #[command(subcommand)]
        resource: DeleteResource,
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
}
