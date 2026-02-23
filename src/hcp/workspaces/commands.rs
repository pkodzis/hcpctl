//! Workspace command handlers

use std::collections::HashMap;

use log::debug;

use crate::cli::{OutputFormat, WsSortField, WsSubresource};
use crate::hcp::helpers::{
    aggregate_pagination_info, collect_org_results, fetch_from_organizations, log_completion,
};
use crate::hcp::organizations::resolve_organizations;
use crate::hcp::projects::resolve_project;
use crate::hcp::runs::{count_runs_by_workspace, RunQuery};
use crate::hcp::workspaces::WorkspaceQuery;
use crate::hcp::TfeClient;
use crate::output::{output_raw, output_results_sorted};
use crate::ui::{
    confirm_large_pagination, create_spinner, finish_spinner, finish_spinner_with_status,
    LargePaginationInfo,
};
use crate::{Cli, Command, GetResource, TfeError, Workspace};

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

    // Validate: --subresource requires a workspace name
    if args.subresource.is_some() && args.name.is_none() {
        return Err("--subresource requires a workspace name or ID".into());
    }

    // Validate: --sort pending-runs requires --has-pending-runs
    if args.sort == WsSortField::PendingRuns && !args.has_pending_runs {
        return Err("--sort pending-runs requires --has-pending-runs".into());
    }

    let effective_org = client.effective_org(args.org.as_ref());

    // If NAME is specified, get single workspace
    if let Some(name) = &args.name {
        return get_single_workspace(client, cli, name, effective_org.as_ref()).await;
    }

    // Otherwise list all workspaces
    let organizations = resolve_organizations(client, effective_org.as_ref()).await?;

    debug!(
        "Processing {} organizations: {:?}",
        organizations.len(),
        organizations
    );

    // Resolve project filter if specified
    let project_id = if let Some(prj_input) = &args.prj {
        if let Some(org) = &effective_org {
            let resolved = resolve_project(client, prj_input, org, cli.batch).await?;
            Some(resolved.project.id)
        } else {
            return Err("Project filter requires an organization to be specified".into());
        }
    } else {
        None
    };

    let filter = args.filter.as_deref();
    let project_id_ref = project_id.as_deref();

    // Phase 1: Prefetch pagination info from all orgs to check scale
    let prefetch_spinner = create_spinner(
        &format!(
            "Checking scale across {} organization(s)...",
            organizations.len()
        ),
        cli.batch,
    );

    let pagination_results = fetch_from_organizations(organizations.clone(), |org| async move {
        let query = WorkspaceQuery {
            search: filter,
            project_id: project_id_ref,
            ..Default::default()
        };
        match client
            .prefetch_workspaces_pagination_info(&org, query)
            .await
        {
            Ok(info) => Ok(info),
            Err(e) => Err((org, e)),
        }
    })
    .await;

    // Collect pagination info (ignoring errors - they'll be caught in main fetch)
    let pagination_infos: Vec<_> = pagination_results
        .into_iter()
        .filter_map(|r| r.ok())
        .collect();

    finish_spinner(prefetch_spinner);

    // Aggregate and check threshold
    let aggregated = aggregate_pagination_info(pagination_infos);

    if aggregated.total_count > 0 {
        let info = LargePaginationInfo::from_aggregated(&aggregated, "workspaces");

        if info.exceeds_threshold() && !confirm_large_pagination(&info, cli.batch) {
            return Err(Box::new(TfeError::UserCancelled));
        }
    }

    // Phase 2: Fetch all workspaces (user confirmed or under threshold)
    let spinner = create_spinner(
        &format!(
            "Fetching workspaces from {} organization(s)...",
            organizations.len()
        ),
        cli.batch,
    );

    let results = fetch_from_organizations(organizations, |org| async move {
        let query = WorkspaceQuery {
            search: filter,
            project_id: project_id_ref,
            ..Default::default()
        };
        let workspaces = client.get_workspaces(&org, query).await;

        match workspaces {
            Ok(ws) => {
                debug!("Found {} workspaces for org '{}'", ws.len(), org);
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
        if args.has_pending_runs {
            // Phase 3: Fetch pending runs per org in parallel
            let org_names: Vec<String> =
                all_workspaces.iter().map(|(org, _)| org.clone()).collect();

            let pending_spinner = create_spinner(
                &format!(
                    "Fetching pending runs from {} organization(s)...",
                    org_names.len()
                ),
                cli.batch,
            );

            let pending_results = fetch_from_organizations(org_names, |org| async move {
                match client
                    .get_runs_for_organization(&org, RunQuery::pending(), None)
                    .await
                {
                    Ok(runs) => Ok(runs),
                    Err(e) => Err((org, e)),
                }
            })
            .await;

            // Collect all pending runs into a single counts map
            let mut counts: HashMap<String, usize> = HashMap::new();
            for result in pending_results {
                match result {
                    Ok(runs) => {
                        for (ws_id, count) in count_runs_by_workspace(&runs) {
                            *counts.entry(ws_id).or_insert(0) += count;
                        }
                    }
                    Err((org, e)) => {
                        eprintln!(
                            "Warning: failed to fetch pending runs for org '{}': {}",
                            org, e
                        );
                    }
                }
            }

            finish_spinner(pending_spinner);

            // Filter out workspaces with zero pending runs
            let filtered: Vec<(String, Vec<Workspace>)> = all_workspaces
                .into_iter()
                .filter_map(|(org, workspaces)| {
                    let filtered_ws: Vec<Workspace> = workspaces
                        .into_iter()
                        .filter(|ws| counts.contains_key(&ws.id))
                        .collect();
                    if filtered_ws.is_empty() {
                        None
                    } else {
                        Some((org, filtered_ws))
                    }
                })
                .collect();

            if filtered.is_empty() {
                println!("No workspaces with pending runs found.");
            } else {
                output_results_sorted(filtered, cli, Some(&counts));
            }
        } else {
            output_results_sorted(all_workspaces, cli, None);
        }
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

    // Validate subresource usage
    if args.subresource.is_some() && !matches!(args.output, OutputFormat::Json | OutputFormat::Yaml)
    {
        return Err(
            "--subresource requires JSON or YAML output format (-o json or -o yaml)".into(),
        );
    }

    // If it's an ID (ws-...), we can fetch directly without knowing the org
    if name.starts_with("ws-") {
        let spinner = create_spinner(&format!("Fetching workspace '{}'...", name), cli.batch);

        match client.get_workspace_by_id(name).await {
            Ok(Some((_workspace, raw))) => {
                finish_spinner(spinner);

                // Handle subresource if requested
                if let Some(subresource) = &args.subresource {
                    return fetch_and_output_subresource(client, cli, &raw, subresource).await;
                }

                // For JSON/YAML, return raw API response
                if matches!(args.output, OutputFormat::Json | OutputFormat::Yaml) {
                    output_raw(&raw, &args.output);
                    return Ok(());
                }

                let workspace: Workspace = serde_json::from_value(raw["data"].clone())
                    .map_err(|e| format!("Failed to parse workspace: {}", e))?;
                let org_name = workspace
                    .organization_name()
                    .unwrap_or("unknown")
                    .to_string();

                let pending_counts = fetch_pending_counts_for_workspace(
                    client,
                    &workspace.id,
                    name,
                    args.has_pending_runs,
                )
                .await?;
                if args.has_pending_runs && pending_counts.is_none() {
                    return Ok(());
                }

                let all_workspaces = vec![(org_name, vec![workspace])];
                output_results_sorted(all_workspaces, cli, pending_counts.as_ref());
                return Ok(());
            }
            Ok(None) => {
                finish_spinner(spinner);
                return Err(format!("Workspace '{}' not found", name).into());
            }
            Err(e) => {
                finish_spinner(spinner);
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
    let name_owned = name.to_string();
    let found = crate::hcp::helpers::search_first_in_orgs(&organizations, |org| {
        let ws_name = name_owned.clone();
        async move {
            match client.get_workspace_by_name(&org, &ws_name).await {
                Ok(Some(result)) => (org, Some(result)),
                _ => (org, None),
            }
        }
    })
    .await;

    if let Some((org_name, (_workspace, raw))) = found {
        finish_spinner(spinner);

        // Handle subresource if requested
        if let Some(subresource) = &args.subresource {
            return fetch_and_output_subresource(client, cli, &raw, subresource).await;
        }

        // For JSON/YAML, return raw API response
        if matches!(args.output, OutputFormat::Json | OutputFormat::Yaml) {
            output_raw(&raw, &args.output);
            return Ok(());
        }

        let workspace: Workspace = serde_json::from_value(raw["data"].clone())
            .map_err(|e| format!("Failed to parse workspace: {}", e))?;

        let pending_counts =
            fetch_pending_counts_for_workspace(client, &workspace.id, name, args.has_pending_runs)
                .await?;
        if args.has_pending_runs && pending_counts.is_none() {
            return Ok(());
        }

        let all_workspaces = vec![(org_name, vec![workspace])];
        output_results_sorted(all_workspaces, cli, pending_counts.as_ref());
        return Ok(());
    }

    finish_spinner(spinner);
    Err(crate::hcp::helpers::not_found_in_orgs_error("Workspace", name, &organizations).into())
}

/// Fetch pending run counts for a single workspace.
/// Returns `Some(counts)` if pending runs exist, `None` (with a printed message) if none found,
/// or `None` if `has_pending_runs` is false.
async fn fetch_pending_counts_for_workspace(
    client: &TfeClient,
    ws_id: &str,
    ws_name: &str,
    has_pending_runs: bool,
) -> Result<Option<HashMap<String, usize>>, Box<dyn std::error::Error>> {
    if !has_pending_runs {
        return Ok(None);
    }
    let runs = client
        .get_runs_for_workspace(ws_id, RunQuery::pending(), None)
        .await?;
    let counts = count_runs_by_workspace(&runs);
    if !counts.contains_key(ws_id) {
        println!("\nNo pending runs found for workspace '{}'", ws_name);
        return Ok(None);
    }
    Ok(Some(counts))
}

/// Fetch and output a workspace subresource
async fn fetch_and_output_subresource(
    client: &TfeClient,
    cli: &Cli,
    workspace_raw: &serde_json::Value,
    subresource: &WsSubresource,
) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Get {
        resource: GetResource::Ws(args),
    } = &cli.command
    else {
        unreachable!()
    };

    // Map subresource enum to relationship key
    let relationship_key = match subresource {
        WsSubresource::Run => "current-run",
        WsSubresource::State => "current-state-version",
        WsSubresource::Config => "current-configuration-version",
        WsSubresource::Assessment => "current-assessment-result",
    };

    // Get the related link from relationships
    let url = workspace_raw["data"]["relationships"][relationship_key]["links"]["related"]
        .as_str()
        .ok_or_else(|| {
            format!(
                "No '{}' relationship found for this workspace",
                relationship_key
            )
        })?;

    let spinner = create_spinner(&format!("Fetching {}...", relationship_key), cli.batch);

    match client.get_subresource(url).await {
        Ok(raw) => {
            finish_spinner(spinner);
            output_raw(&raw, &args.output);
            Ok(())
        }
        Err(e) => {
            finish_spinner(spinner);
            Err(e.into())
        }
    }
}
