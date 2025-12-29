//! TFE API client module
//!
//! This module provides functionality to interact with Terraform Enterprise API.

mod client;
mod credentials;
pub mod helpers;
pub mod organizations;
pub mod projects;
pub mod traits;
pub mod workspaces;

use serde::Deserialize;

pub use client::TfeClient;
pub use credentials::TokenResolver;
pub use helpers::{collect_org_results, fetch_from_organizations, log_completion};
pub use organizations::{
    resolve_organizations, run_org_command, Organization, OrganizationAttributes,
};
pub use projects::{run_prj_command, Project, ProjectAttributes, ProjectWorkspaces};
pub use traits::TfeResource;
pub use workspaces::{run_ws_command, Workspace, WorkspaceAttributes};

/// Pagination metadata from TFE API (shared across resources)
#[derive(Deserialize, Debug, Default)]
pub struct PaginationMeta {
    pub pagination: Option<Pagination>,
}

/// Pagination details
#[derive(Deserialize, Debug)]
pub struct Pagination {
    #[serde(rename = "current-page")]
    pub current_page: u32,
    #[serde(rename = "total-pages")]
    pub total_pages: u32,
    #[serde(rename = "total-count")]
    pub total_count: u32,
}
