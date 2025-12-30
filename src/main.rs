//! HCP CLI - Main entry point

use clap::Parser;
use log::info;
use std::process::ExitCode;

use hcpctl::{
    run_org_command, run_prj_command, run_ws_command, Cli, Command, GetResource, HostResolver,
    TfeClient, TokenResolver,
};

#[tokio::main]
async fn main() -> ExitCode {
    if let Err(e) = run().await {
        eprintln!("\n{}\n", e);
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(&cli.log_level))
        .init();

    info!("Starting HCP CLI v{}", env!("CARGO_PKG_VERSION"));

    // Resolve host with fallback logic (CLI -> env var -> credentials file)
    // In batch mode, error on multiple hosts instead of interactive selection
    let host = HostResolver::resolve(cli.host.as_deref(), cli.batch)?;

    // Resolve token with fallback logic
    let token_resolver = TokenResolver::new(&host);
    let token = token_resolver.resolve(cli.token.as_deref())?;

    // Create TFE client
    let client = TfeClient::new(token, host);

    match &cli.command {
        Command::Get { resource } => match resource {
            GetResource::Org(_) => run_org_command(&client, &cli).await,
            GetResource::Prj(_) => run_prj_command(&client, &cli).await,
            GetResource::Ws(_) => run_ws_command(&client, &cli).await,
        },
    }
}
