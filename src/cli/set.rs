//! Set command resource definitions and arguments

use clap::{Parser, Subcommand};

/// Resource types for the 'set' command
#[derive(Subcommand, Debug)]
pub enum SetResource {
    /// Assign workspace to a project
    #[command(
        visible_alias = "workspace",
        visible_alias = "workspaces",
        override_usage = "hcpctl set ws [OPTIONS] <WORKSPACE> --prj <PROJECT>"
    )]
    Ws(SetWsArgs),

    /// Set tag bindings on a workspace or project
    #[command(visible_alias = "tags")]
    Tag {
        #[command(subcommand)]
        resource: super::tag::SetTagResource,
    },
}

/// Arguments for 'set ws' subcommand
#[derive(Parser, Debug)]
pub struct SetWsArgs {
    /// Workspace name or ID (ws-xxx)
    pub workspace: String,

    /// Target project name or ID (prj-xxx)
    #[arg(long = "prj", short = 'p')]
    pub project: String,

    /// Organization name (auto-discovered when using workspace ID)
    #[arg(long = "org")]
    pub org: Option<String>,

    /// Skip confirmation prompt
    #[arg(short = 'y', long, default_value_t = false)]
    pub yes: bool,
}
