//! HCPctl - Explore HashiCorp Cloud Platform and Terraform Enterprise
//!
//! A CLI tool to list and explore TFE/HCP resources.
//!
//! # Features
//!
//! - List organizations, projects, and workspaces
//! - Filter and search resources
//! - Multiple output formats (table, CSV, JSON)
//! - Parallel fetching for better performance
//! - Automatic pagination handling
//!
//! # Example
//!
//! ```bash
//! # List all organizations
//! hcpctl get org
//!
//! # List projects in an organization
//! hcpctl get prj --org my-org
//!
//! # List workspaces
//! hcpctl get ws --org my-org
//!
//! # Filter workspaces by name
//! hcpctl get ws --org my-org -f "prod"
//!
//! # Output as JSON
//! hcpctl get ws --org my-org -o json
//! ```

pub mod cli;
pub mod config;
pub mod error;
pub mod hcp;
pub mod output;
pub mod ui;
pub mod update;

pub use cli::{
    Cli, Command, DeleteOrgMemberArgs, DeleteResource, DownloadConfigArgs, DownloadResource,
    GetResource, InviteArgs, LogsArgs, OcArgs, OrgArgs, OrgMemberArgs, OutputFormat, PrjArgs,
    PrjSortField, PurgeResource, PurgeRunArgs, PurgeStateArgs, RunArgs, RunSortField,
    RunSubresource, TeamArgs, WatchResource, WatchWsArgs, WsArgs, WsSortField, WsSubresource,
};
pub use error::{Result, TfeError};
pub use hcp::{
    run_delete_org_member_command, run_download_config_command, run_invite_command,
    run_logs_command, run_oc_command, run_org_command, run_org_member_command, run_prj_command,
    run_purge_run_command, run_purge_state_command, run_runs_command, run_team_command,
    run_watch_ws_command, run_ws_command, HostResolver, OAuthClient, Organization, Project, Run,
    Team, TfeClient, TfeResource, TokenResolver, Workspace,
};
pub use output::{
    output_oauth_clients, output_organizations, output_projects, output_results_sorted,
    output_runs, WorkspaceRow,
};
pub use update::{run_update, UpdateChecker, UpdateHandle};
