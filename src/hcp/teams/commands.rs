//! Team command handlers

use log::debug;

use crate::cli::OutputFormat;
use crate::hcp::TfeClient;
use crate::output::{output_raw, output_teams};
use crate::ui::{create_spinner, finish_spinner};
use crate::{Cli, Command, GetResource};

/// Run the team list/get command
pub async fn run_team_command(
    client: &TfeClient,
    cli: &Cli,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let Command::Get {
        resource: GetResource::Team(args),
    } = &cli.command
    else {
        unreachable!()
    };

    let effective_org = client.effective_org(args.org.as_ref());

    let org = effective_org
        .as_ref()
        .ok_or("Organization is required (--org)")?;

    debug!("Fetching teams for organization: {}", org);

    let spinner = create_spinner(&format!("Fetching teams for '{}'...", org), cli.batch);

    // If NAME is specified and output is JSON/YAML, use direct API call for raw output
    if let Some(name) = &args.name {
        // Try by ID first if it looks like an ID
        let result = if name.starts_with("team-") {
            client.get_team(name).await?
        } else {
            client.get_team_by_name(org, name).await?
        };

        match result {
            Some((team, raw)) => {
                finish_spinner(spinner);
                if matches!(args.output, OutputFormat::Json | OutputFormat::Yaml) {
                    output_raw(&raw, &args.output);
                } else {
                    output_teams(&[team], cli);
                }
                return Ok(());
            }
            None => {
                finish_spinner(spinner);
                return Err(format!("Team '{}' not found in organization '{}'", name, org).into());
            }
        }
    }

    // List all teams
    let mut teams = client.get_teams(org).await?;

    finish_spinner(spinner);

    // Apply filter if specified
    if let Some(filter) = &args.filter {
        let filter_lower = filter.to_lowercase();
        teams.retain(|team| team.name().to_lowercase().contains(&filter_lower));
        debug!("Filtered to {} teams matching '{}'", teams.len(), filter);
    }

    if teams.is_empty() {
        if args.filter.is_some() {
            eprintln!("No teams found matching filter");
        } else {
            eprintln!("No teams found in organization '{}'", org);
        }
        return Ok(());
    }

    output_teams(&teams, cli);
    Ok(())
}
