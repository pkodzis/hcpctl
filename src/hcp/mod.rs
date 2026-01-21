//! TFE API client module
//!
//! This module provides functionality to interact with Terraform Enterprise API.

mod client;
pub mod configuration_versions;
mod credentials;
pub mod helpers;
mod host;
pub mod logs;
pub mod oauth_clients;
pub mod org_memberships;
pub mod organizations;
pub mod projects;
pub mod runs;
pub mod state;
pub mod teams;
pub mod traits;
pub mod watch;
pub mod workspaces;

use serde::Deserialize;

pub use client::TfeClient;
pub use configuration_versions::run_download_config_command;
pub use credentials::TokenResolver;
pub use helpers::{collect_org_results, fetch_from_organizations, log_completion};
pub use host::HostResolver;
pub use logs::run_logs_command;
pub use oauth_clients::{run_oc_command, OAuthClient, OAuthClientAttributes, OAuthToken};
pub use org_memberships::{
    run_delete_org_member_command, run_invite_command, run_org_member_command,
    OrganizationMembership, OrganizationMembershipAttributes,
};
pub use organizations::{
    resolve_organizations, run_org_command, Organization, OrganizationAttributes,
    OrganizationWithTokens,
};
pub use projects::{run_prj_command, Project, ProjectAttributes, ProjectWorkspaces};
pub use runs::{run_purge_run_command, run_runs_command, Run, RunAttributes};
pub use state::run_purge_state_command;
pub use teams::{run_team_command, Team, TeamAttributes};
pub use traits::{PaginatedResponse, TfeResource};
pub use watch::run_watch_ws_command;
pub use workspaces::{
    extract_current_run_id, resolve_workspace, run_ws_command, ResolvedWorkspace, Workspace,
    WorkspaceAttributes, WorkspaceTarget,
};

/// Pagination metadata from TFE API (shared across resources)
#[derive(Deserialize, Debug, Default, Clone)]
pub struct PaginationMeta {
    pub pagination: Option<Pagination>,
}

/// Pagination details
#[derive(Deserialize, Debug, Clone)]
pub struct Pagination {
    #[serde(rename = "current-page")]
    pub current_page: u32,
    #[serde(rename = "total-pages")]
    pub total_pages: u32,
    #[serde(rename = "total-count")]
    pub total_count: u32,
}
