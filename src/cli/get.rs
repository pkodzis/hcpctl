//! Get command resource definitions and arguments

use clap::{builder::ArgPredicate, Parser, Subcommand};

use super::common::OutputFormat;
use super::enums::{PrjSortField, RunSortField, RunSubresource, WsSortField, WsSubresource};

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

    /// Get teams in an organization
    #[command(visible_alias = "teams")]
    Team(TeamArgs),

    /// Get organization members
    #[command(
        visible_alias = "org-members",
        visible_alias = "orgmember",
        visible_alias = "orgmembers"
    )]
    OrgMember(OrgMemberArgs),

    /// Get tags (org-level, workspace, or project)
    #[command(visible_alias = "tags")]
    Tag(super::tag::GetTagArgs),
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

/// Arguments for 'get team' subcommand
#[derive(Parser, Debug)]
pub struct TeamArgs {
    /// Team name or ID (if specified, shows details for that team)
    pub name: Option<String>,

    /// Organization name (required)
    #[arg(long = "org")]
    pub org: Option<String>,

    /// Filter teams by name (substring match)
    #[arg(short, long)]
    pub filter: Option<String>,

    /// Output format
    #[arg(short = 'o', long, value_enum, default_value_t = OutputFormat::Table)]
    pub output: OutputFormat,
}

/// Arguments for 'get org-member' subcommand
#[derive(Parser, Debug)]
pub struct OrgMemberArgs {
    /// Membership ID (ou-xxx) - if specified, shows details for that membership
    pub id: Option<String>,

    /// Organization name (if not specified, lists members from all organizations)
    #[arg(long = "org")]
    pub org: Option<String>,

    /// Filter by email (substring match)
    #[arg(short, long)]
    pub filter: Option<String>,

    /// Filter by status (active, invited)
    #[arg(long)]
    pub status: Option<String>,

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

    /// Only show workspaces that have runs in 'pending' status (queued behind another active run). Adds a "Pending Runs" count column
    #[arg(long, default_value_t = false)]
    pub has_pending_runs: bool,

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
