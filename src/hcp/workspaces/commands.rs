//! Workspace command handlers

use log::debug;

use crate::cli::{OutputFormat, WsSubresource};
use crate::hcp::helpers::{
    aggregate_pagination_info, collect_org_results, fetch_from_organizations, log_completion,
};
use crate::hcp::organizations::resolve_organizations;
use crate::hcp::projects::resolve_project;
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
                let all_workspaces = vec![(org_name, vec![workspace])];
                output_results_sorted(all_workspaces, cli);
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
        let all_workspaces = vec![(org_name, vec![workspace])];
        output_results_sorted(all_workspaces, cli);
        return Ok(());
    }

    finish_spinner(spinner);
    Err(crate::hcp::helpers::not_found_in_orgs_error("Workspace", name, &organizations).into())
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
