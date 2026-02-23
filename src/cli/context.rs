//! Config management CLI arguments (kubectl-style)

use clap::{Parser, Subcommand};

/// Config subcommands for managing connection contexts
#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Set a context entry in the config file
    #[command(name = "set-context")]
    SetContext(SetContextArgs),

    /// Set the current-context in the config file
    #[command(name = "use-context")]
    UseContext(UseContextArgs),

    /// Describe one or many contexts
    #[command(name = "get-contexts")]
    GetContexts,

    /// Display the current-context
    #[command(name = "current-context")]
    CurrentContext,

    /// Delete the specified context from the config file
    #[command(name = "delete-context")]
    DeleteContext(DeleteContextArgs),

    /// Display config file contents
    View,
}

/// Arguments for 'config set-context' subcommand
#[derive(Parser, Debug)]
#[command(after_help = "EXAMPLES:\n  \
        hcpctl config set-context prod --host app.terraform.io --org my-org\n  \
        hcpctl config set-context dev --host tfe.corp.com --token <TOKEN>\n  \
        hcpctl config set-context prod --org new-org   # update existing context")]
pub struct SetContextArgs {
    /// Context name
    pub name: String,
    /// TFE/HCP host URL
    #[arg(long)]
    pub host: Option<String>,
    /// API token (stored in config file)
    #[arg(long)]
    pub token: Option<String>,
    /// Default organization
    #[arg(long)]
    pub org: Option<String>,
}

/// Arguments for 'config use-context' subcommand
#[derive(Parser, Debug)]
pub struct UseContextArgs {
    /// Context name to activate
    pub name: String,
}

/// Arguments for 'config delete-context' subcommand
#[derive(Parser, Debug)]
pub struct DeleteContextArgs {
    /// Context name to delete
    pub name: String,
}
