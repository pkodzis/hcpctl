//! Download command resource definitions and arguments

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Resource types for the 'download' command
#[derive(Subcommand, Debug)]
pub enum DownloadResource {
    /// Download workspace configuration files (tar.gz)
    ///
    /// Downloads the Terraform configuration files associated with a workspace's
    /// current or specified configuration version.
    ///
    /// PROCEDURE:
    ///   1. Resolves workspace by name or ID (auto-discovers organization)
    ///   2. Fetches configuration version details (current or specified)
    ///   3. Downloads the configuration archive (tar.gz)
    ///   4. Saves to specified output file or default name
    ///
    /// OUTPUT:
    ///   By default, saves to: configuration-{cv_id}.tar.gz
    ///   Use --output to specify a custom path.
    ///
    /// EXAMPLES:
    ///   hcpctl download config my-workspace --org my-org
    ///   hcpctl download config ws-abc123
    ///   hcpctl download config my-ws --output ./config.tar.gz
    ///   hcpctl download config my-ws --cv-id cv-xyz789
    #[command(verbatim_doc_comment, visible_alias = "cfg")]
    Config(DownloadConfigArgs),
}

/// Arguments for 'download config' subcommand
#[derive(Parser, Debug)]
pub struct DownloadConfigArgs {
    /// Workspace name or ID (ws-xxx) to download configuration from
    ///
    /// Can be either:
    /// - Workspace name (e.g., "my-workspace") - requires --org or auto-discovery
    /// - Workspace ID (e.g., "ws-abc123") - organization auto-detected
    #[arg(verbatim_doc_comment)]
    pub workspace: String,

    /// Organization name (auto-detected if not provided)
    #[arg(short, long)]
    pub org: Option<String>,

    /// Specific configuration version ID (default: current/latest)
    ///
    /// If not specified, downloads the most recent uploaded configuration version.
    #[arg(long, verbatim_doc_comment)]
    pub cv_id: Option<String>,

    /// Output file path (default: configuration-{cv_id}.tar.gz)
    #[arg(long)]
    pub output: Option<PathBuf>,
}
