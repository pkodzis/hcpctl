//! Workspace command handlers

use log::debug;

use crate::cli::OutputFormat;
use crate::hcp::helpers::{collect_org_results, fetch_from_organizations, log_completion};
use crate::hcp::organizations::resolve_organizations;
use crate::hcp::TfeClient;
use crate::output::{output_raw, output_results_sorted};
use crate::ui::{create_spinner, finish_spinner, finish_spinner_with_status};
use crate::{Cli, Command, GetResource, Workspace};

/// Run the workspace list command
pub async fn run_ws_command(
    client: &TfeClient,
    cli: &Cli,
) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Get {
        resource: GetResource::Ws(args),
    } = &cli.command
    else {
        unreachable!()
    };

    // If NAME is specified, get single workspace
    if let Some(name) = &args.name {
        return get_single_workspace(client, cli, name, args.org.as_ref()).await;
    }

    // Otherwise list all workspaces
    let organizations = resolve_organizations(client, args.org.as_ref()).await?;

    debug!(
        "Processing {} organizations: {:?}",
        organizations.len(),
        organizations
    );

    // Resolve project filter if specified
    let project_id = if let Some(prj_input) = &args.prj {
        if let Some(org) = &args.org {
            // Use appropriate method based on whether it's an ID or name
            let project = if prj_input.starts_with("prj-") {
                client.get_project_by_id(prj_input).await?
            } else {
                client.get_project_by_name(org, prj_input).await?
            };
            match project {
                Some((p, _raw)) => Some(p.id),
                None => {
                    return Err(format!(
                        "Project '{}' not found in organization '{}'",
                        prj_input, org
                    )
                    .into());
                }
            }
        } else {
            return Err("Project filter requires an organization to be specified".into());
        }
    } else {
        None
    };

    let spinner = create_spinner(
        &format!(
            "Fetching workspaces from {} organization(s)...",
            organizations.len()
        ),
        cli.batch,
    );

    // Fetch workspaces from all orgs in parallel
    let filter = args.filter.as_deref();
    let project_id_ref = project_id.as_deref();

    let results = fetch_from_organizations(organizations, |org| async move {
        let workspaces = if let Some(prj_id) = project_id_ref {
            client.get_workspaces_by_project(&org, prj_id, filter).await
        } else {
            client.get_workspaces_filtered(&org, filter).await
        };

        match workspaces {
            Ok(ws) => {
                debug!(
                    "Found {} workspaces for org '{}' (after filtering)",
                    ws.len(),
                    org
                );
                Ok((org, ws))
            }
            Err(e) => {
                debug!("Error fetching workspaces for org '{}': {}", org, e);
                Err((org, e))
            }
        }
    })
    .await;

    let (all_workspaces, had_errors): (Vec<(String, Vec<Workspace>)>, bool) =
        collect_org_results(results, &spinner, "workspaces");

    finish_spinner_with_status(spinner, &all_workspaces, had_errors);

    if !all_workspaces.is_empty() {
        output_results_sorted(all_workspaces, cli);
    }

    log_completion(had_errors);
    Ok(())
}

/// Get a single workspace by name or ID
async fn get_single_workspace(
    client: &TfeClient,
    cli: &Cli,
    name: &str,
    org: Option<&String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Get {
        resource: GetResource::Ws(args),
    } = &cli.command
    else {
        unreachable!()
    };

    // If it's an ID (ws-...), we can fetch directly without knowing the org
    if name.starts_with("ws-") {
        let spinner = create_spinner(&format!("Fetching workspace '{}'...", name), cli.batch);

        match client.get_workspace_by_id(name).await {
            Ok(Some((workspace, raw))) => {
                finish_spinner(spinner, "Found");

                // For JSON/YAML, return raw API response
                if matches!(args.output, OutputFormat::Json | OutputFormat::Yaml) {
                    output_raw(&raw, &args.output);
                    return Ok(());
                }

                let org_name = workspace
                    .organization_name()
                    .unwrap_or("unknown")
                    .to_string();
                let all_workspaces = vec![(org_name, vec![workspace])];
                output_results_sorted(all_workspaces, cli);
                return Ok(());
            }
            Ok(None) => {
                finish_spinner(spinner, "Not found");
                return Err(format!("Workspace '{}' not found", name).into());
            }
            Err(e) => {
                finish_spinner(spinner, "Error");
                return Err(e.into());
            }
        }
    }

    // For name-based lookup, we need to search organizations
    let organizations = resolve_organizations(client, org).await?;

    let spinner = create_spinner(
        &format!(
            "Searching for workspace '{}' in {} organization(s)...",
            name,
            organizations.len()
        ),
        cli.batch,
    );

    // Search in all organizations IN PARALLEL with early termination
    use futures::stream::{FuturesUnordered, StreamExt};

    let name_owned = name.to_string();
    let mut futures: FuturesUnordered<_> = organizations
        .iter()
        .map(|org_name| {
            let org = org_name.clone();
            let ws_name = name_owned.clone();
            async move {
                let result = client.get_workspace_by_name(&org, &ws_name).await;
                (org, result)
            }
        })
        .collect();

    // Process results as they complete, stop on first match
    while let Some((org_name, result)) = futures.next().await {
        if let Ok(Some((workspace, raw))) = result {
            finish_spinner(spinner, "Found");

            // For JSON/YAML, return raw API response
            if matches!(args.output, OutputFormat::Json | OutputFormat::Yaml) {
                output_raw(&raw, &args.output);
                return Ok(());
            }

            let all_workspaces = vec![(org_name, vec![workspace])];
            output_results_sorted(all_workspaces, cli);
            return Ok(());
        }
    }

    finish_spinner(spinner, "Not found");

    let searched = if organizations.len() == 1 {
        format!("organization '{}'", organizations[0])
    } else {
        format!("{} organizations", organizations.len())
    };

    Err(format!("Workspace '{}' not found in {}", name, searched).into())
}
