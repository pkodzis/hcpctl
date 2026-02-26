//! Team access command arguments

use clap::Parser;

use super::common::OutputFormat;
use super::enums::TeamAccessSortField;

/// Arguments for 'get team-access' subcommand
#[derive(Parser, Debug)]
pub struct TeamAccessArgs {
    /// Team name or ID — if omitted, lists all teams' bindings
    pub name: Option<String>,

    /// Organization name (required)
    #[arg(long = "org")]
    pub org: Option<String>,

    /// Filter by project (name or ID)
    #[arg(short = 'p', long)]
    pub prj: Option<String>,

    /// Filter results by team name, project name, or access level (substring match)
    #[arg(short, long)]
    pub filter: Option<String>,

    /// Output format
    #[arg(short = 'o', long, value_enum, default_value_t = OutputFormat::Table)]
    pub output: OutputFormat,

    /// Sort results by field
    #[arg(short, long, value_enum, default_value_t = TeamAccessSortField::Team)]
    pub sort: TeamAccessSortField,

    /// Reverse sort order (descending)
    #[arg(short = 'r', long, default_value_t = false)]
    pub reverse: bool,
}
