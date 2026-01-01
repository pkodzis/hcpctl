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

pub use cli::{
    Cli, Command, GetResource, LogsArgs, OcArgs, OrgArgs, OutputFormat, PrjArgs, PrjSortField,
    RunArgs, RunSortField, RunSubresource, WsArgs, WsSortField, WsSubresource,
};
pub use error::{Result, TfeError};
pub use hcp::{
    run_logs_command, run_oc_command, run_org_command, run_prj_command, run_runs_command,
    run_ws_command, HostResolver, OAuthClient, Organization, Project, Run, TfeClient, TfeResource,
    TokenResolver, Workspace,
};
pub use output::{
    output_oauth_clients, output_organizations, output_projects, output_results_sorted,
    output_runs, WorkspaceRow,
};
