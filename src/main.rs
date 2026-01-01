//! HCPCTL - Main entry point

use clap::Parser;
use log::info;
use std::process::ExitCode;

use hcpctl::{
    run_logs_command, run_oc_command, run_org_command, run_prj_command, run_runs_command,
    run_watch_ws_command, run_ws_command, Cli, Command, GetResource, HostResolver, TfeClient,
    TokenResolver, WatchResource,
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
            GetResource::Oc(_) => run_oc_command(&client, &cli).await,
            GetResource::Run(_) => run_runs_command(&client, &cli).await,
        },
        Command::Logs(args) => run_logs_command(&client, &cli, args).await,
        Command::Watch { resource } => match resource {
            WatchResource::Ws(args) => run_watch_ws_command(&client, &cli, args).await,
        },
    }
}
