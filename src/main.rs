//! TFE Workspace Lister - Main entry point

use clap::Parser;
use futures::future::join_all;
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, info};

use hcp_cli::{output_results_sorted, Cli, SortOptions, TfeClient, TokenResolver, Workspace};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(&cli.log_level))
        .init();

    info!("Starting TFE workspace lister v{}", env!("CARGO_PKG_VERSION"));
    debug!(
        "CLI args: org={:?}, filter={:?}, host={}, format={}, sort={}, no_group={}, reverse={}",
        cli.org, cli.filter, cli.host, cli.format, cli.sort, cli.no_group, cli.reverse
    );

    // Resolve token with fallback logic
    let token_resolver = TokenResolver::new(&cli.host);
    let token = token_resolver.resolve(cli.tfe_token.as_deref())?;

    // Create TFE client
    let client = TfeClient::new(token, cli.host.clone());

    // Get list of organizations
    let organizations: Vec<String> = match &cli.org {
        Some(org) => {
            debug!("Using specified organization: {}", org);
            vec![org.clone()]
        }
        None => {
            debug!("Fetching all organizations from TFE");
            client.get_organizations().await?
        }
    };

    debug!(
        "Processing {} organizations: {:?}",
        organizations.len(),
        organizations
    );

    // Create progress spinner
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner:.blue} {msg}")?,
    );
    spinner.set_message(format!(
        "Fetching workspaces from {} organization(s)...",
        organizations.len()
    ));
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));

    // Process organizations in parallel
    let filter = cli.filter.as_deref();
    let futures = organizations.into_iter().map(|org| {
        let client = &client;
        async move {
            match client.get_workspaces_filtered(&org, filter).await {
                Ok(workspaces) => {
                    debug!(
                        "Found {} workspaces for org '{}' (after filtering)",
                        workspaces.len(),
                        org
                    );
                    Ok((org, workspaces))
                }
                Err(e) => {
                    debug!("Error fetching workspaces for org '{}': {}", org, e);
                    Err((org, e))
                }
            }
        }
    });

    let results = join_all(futures).await;

    // Stop spinner
    spinner.finish_with_message("Workspaces fetched successfully!");

    // Collect successful results and report errors
    let mut all_workspaces: Vec<(String, Vec<Workspace>)> = Vec::new();
    let mut had_errors = false;

    for result in results {
        match result {
            Ok((org, workspaces)) => {
                all_workspaces.push((org, workspaces));
            }
            Err((org, e)) => {
                eprintln!("Error fetching workspaces for org '{}': {}", org, e);
                had_errors = true;
            }
        }
    }

    // Output results with sorting options
    let sort_options = SortOptions {
        field: cli.sort,
        reverse: cli.reverse,
        group_by_org: !cli.no_group,
    };
    output_results_sorted(all_workspaces, &cli.format, &sort_options);

    if had_errors {
        info!("Completed with some errors");
    } else {
        info!("Completed successfully");
    }

    Ok(())
}
