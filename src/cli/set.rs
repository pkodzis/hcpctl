//! Set command resource definitions and arguments

use clap::{Parser, Subcommand};

/// Resource types for the 'set' command
#[derive(Subcommand, Debug)]
pub enum SetResource {
    /// Modify workspace settings (project assignment, terraform version, etc.)
    #[command(visible_alias = "workspace", visible_alias = "workspaces")]
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
#[command(group = clap::ArgGroup::new("settings").required(true).multiple(true).args(["project", "terraform_version"]))]
pub struct SetWsArgs {
    /// Workspace name or ID (ws-xxx)
    pub workspace: String,

    /// Target project name or ID (prj-xxx)
    #[arg(long = "prj", short = 'p')]
    pub project: Option<String>,

    /// Terraform version to set (e.g. 1.5.0)
    #[arg(long = "terraform-version", visible_alias = "tf-version")]
    pub terraform_version: Option<String>,

    /// Organization name (auto-discovered when using workspace ID)
    #[arg(long = "org")]
    pub org: Option<String>,

    /// Skip confirmation prompt
    #[arg(short = 'y', long, default_value_t = false)]
    pub yes: bool,
}
