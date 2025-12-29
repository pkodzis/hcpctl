//! TFE Workspace Lister
//!
//! A CLI tool to list and explore Terraform Enterprise workspaces.
//!
//! # Features
//!
//! - List workspaces across multiple organizations
//! - Filter workspaces by name
//! - Multiple output formats (table, CSV, JSON)
//! - Parallel fetching for better performance
//! - Automatic pagination handling
//!
//! # Example
//!
//! ```bash
//! # List all workspaces in an organization
//! hcp-cli -o my-org
//!
//! # Filter workspaces
//! hcp-cli -o my-org -f "prod"
//!
//! # Output as JSON
//! hcp-cli -o my-org --format json
//! ```

pub mod cli;
pub mod config;
pub mod error;
pub mod hcp;
pub mod output;

pub use cli::{Cli, OutputFormat, SortField};
pub use error::{Result, TfeError};
pub use hcp::{TfeClient, TokenResolver, Workspace};
pub use output::{output_results, output_results_sorted, SortOptions};
