//! Logs command arguments

use clap::Parser;

/// Arguments for 'logs' command
#[derive(Parser, Debug)]
pub struct LogsArgs {
    /// Run ID (run-xxx), workspace ID (ws-xxx), or workspace name
    ///
    ///   run-xxx  directly fetches logs for that run
    ///   ws-xxx   fetches logs for workspace's current run
    ///   name     workspace name, fetches current run (requires --org)
    #[arg(verbatim_doc_comment)]
    pub target: String,

    /// Organization name (required when target is a workspace name)
    #[arg(short = 'O', long)]
    pub org: Option<String>,

    /// Show apply log instead of plan log (default: plan)
    #[arg(short = 'a', long, default_value_t = false)]
    pub apply: bool,

    /// Follow log output in real-time until completion (like tail -f)
    #[arg(short = 'f', long, default_value_t = false)]
    pub follow: bool,

    /// Output raw log without parsing (default: extract @message from JSON lines)
    #[arg(long, default_value_t = false)]
    pub raw: bool,
}
