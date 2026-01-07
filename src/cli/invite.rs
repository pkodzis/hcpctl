//! Invite command arguments

use clap::Parser;

use super::common::OutputFormat;

/// Arguments for 'invite' command
#[derive(Parser, Debug)]
pub struct InviteArgs {
    /// Email address of user to invite
    #[arg(long)]
    pub email: String,

    /// Organization name to invite user to
    #[arg(long = "org")]
    pub org: String,

    /// Team ID(s) to add user to (comma-separated, e.g. team-xxx,team-yyy)
    #[arg(long)]
    pub teams: Option<String>,

    /// Output format
    #[arg(short = 'o', long, value_enum, default_value_t = OutputFormat::Table)]
    pub output: OutputFormat,
}
