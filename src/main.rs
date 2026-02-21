//! HCPCTL - Main entry point

use clap::Parser;
use log::info;
use std::process::ExitCode;

use hcpctl::{
    run_delete_org_member_command, run_delete_tag_command, run_download_config_command,
    run_get_tag_command, run_invite_command, run_logs_command, run_oc_command, run_org_command,
    run_org_member_command, run_prj_command, run_purge_run_command, run_purge_state_command,
    run_runs_command, run_set_tag_command, run_set_ws_command, run_team_command, run_update,
    run_watch_ws_command, run_ws_command, Cli, Command, DeleteResource, DownloadResource,
    GetResource, HostResolver, PurgeResource, SetResource, TfeClient, TokenResolver, UpdateChecker,
    WatchResource,
};

#[tokio::main]
async fn main() -> ExitCode {
    // Handle markdown help early (before clap parsing requires subcommand)
    if std::env::args().any(|arg| arg == "--markdown-help") {
        clap_markdown::print_help_markdown::<Cli>();
        return ExitCode::SUCCESS;
    }

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

    // Handle update command early (doesn't require TFE credentials)
    if matches!(cli.command, Command::Update) {
        return run_update().await;
    }

    // Start background update check (non-blocking, only in interactive mode)
    let update_handle = if !cli.batch {
        UpdateChecker::new().check_async()
    } else {
        None
    };

    // Resolve host with fallback logic (CLI -> env var -> credentials file)
    // In batch mode, error on multiple hosts instead of interactive selection
    let host = HostResolver::resolve(cli.host.as_deref(), cli.batch)?;

    // Resolve token with fallback logic
    let token_resolver = TokenResolver::new(&host);
    let token = token_resolver.resolve(cli.token.as_deref())?;

    // Create TFE client with batch mode setting
    let mut client = TfeClient::new(token, host);
    client.set_batch_mode(cli.batch);

    let result = match &cli.command {
        Command::Get { resource } => match resource {
            GetResource::Org(_) => run_org_command(&client, &cli).await,
            GetResource::Prj(_) => run_prj_command(&client, &cli).await,
            GetResource::Ws(_) => run_ws_command(&client, &cli).await,
            GetResource::Oc(_) => run_oc_command(&client, &cli).await,
            GetResource::Run(_) => run_runs_command(&client, &cli).await,
            GetResource::Team(_) => run_team_command(&client, &cli).await,
            GetResource::OrgMember(_) => run_org_member_command(&client, &cli).await,
            GetResource::Tag(_) => run_get_tag_command(&client, &cli).await,
        },
        Command::Delete { resource } => match resource {
            DeleteResource::OrgMember(args) => {
                run_delete_org_member_command(&client, &cli, args).await
            }
            DeleteResource::Tag { .. } => run_delete_tag_command(&client, &cli).await,
        },
        Command::Purge { resource } => match resource {
            PurgeResource::State(_) => run_purge_state_command(&client, &cli).await,
            PurgeResource::Run(_) => run_purge_run_command(&client, &cli).await,
        },
        Command::Logs(args) => run_logs_command(&client, &cli, args).await,
        Command::Watch { resource } => match resource {
            WatchResource::Ws(args) => run_watch_ws_command(&client, &cli, args).await,
        },
        Command::Download { resource } => match resource {
            DownloadResource::Config(_) => run_download_config_command(&client, &cli).await,
        },
        Command::Invite(args) => run_invite_command(&client, &cli, args).await,
        Command::Set { resource } => match resource {
            SetResource::Ws(_) => run_set_ws_command(&client, &cli).await,
            SetResource::Tag { .. } => run_set_tag_command(&client, &cli).await,
        },
        Command::Update => unreachable!(), // Handled above
    };

    // Show update notification if available (non-blocking check completed)
    if let Some(handle) = update_handle {
        if let Some(msg) = handle.get() {
            eprintln!("{}", msg);
        }
    }

    result
}
