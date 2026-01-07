//! Watch command resource definitions and arguments

use clap::{Parser, Subcommand};

/// Resource types for the 'watch' command
#[derive(Subcommand, Debug)]
pub enum WatchResource {
    /// Watch a workspace for new runs and stream their logs
    ///
    /// Continuously monitors a workspace for new runs. When a new run starts,
    /// automatically streams its logs until completion, then watches for the
    /// next run. Logs are prefixed with [run-xxx] by default.
    #[command(visible_alias = "workspace", verbatim_doc_comment)]
    Ws(WatchWsArgs),
}

/// Arguments for 'watch ws' subcommand
#[derive(Parser, Debug)]
pub struct WatchWsArgs {
    /// Workspace ID (ws-xxx) or workspace name
    ///
    ///   ws-xxx   Workspace ID - watches directly
    ///   name     Workspace name - requires --org or auto-discovery
    #[arg(verbatim_doc_comment)]
    pub target: String,

    /// Organization name (optional - will search all orgs if not specified)
    #[arg(short = 'O', long)]
    pub org: Option<String>,

    /// Show apply logs instead of plan logs (default: plan)
    #[arg(short = 'a', long, default_value_t = false)]
    pub apply: bool,

    /// Disable [run-xxx] prefix on log output (default: prefix enabled)
    #[arg(long = "no-prefix", default_value_t = false)]
    pub no_prefix: bool,

    /// Poll interval in seconds (default: 3)
    #[arg(short = 'i', long, default_value_t = 3)]
    pub interval: u64,

    /// Output raw log without parsing (default: extract @message from JSON lines)
    #[arg(long, default_value_t = false)]
    pub raw: bool,
}
