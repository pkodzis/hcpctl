//! Project command handlers

use crate::cli::OutputFormat;
use crate::hcp::helpers::{collect_org_results, fetch_from_organizations, log_completion};
use crate::hcp::organizations::resolve_organizations;
use crate::hcp::projects::models::ProjectWorkspaces;
use crate::hcp::traits::TfeResource;
use crate::hcp::TfeClient;
use crate::output::{output_projects, output_raw};
use crate::ui::{create_spinner, finish_spinner, finish_spinner_with_status};
use crate::{Cli, Command, GetResource, PrjSortField, Project};

/// Project row for output: (org_name, project, workspace_info)
pub type ProjectRow = (String, Project, ProjectWorkspaces);

/// Run the project list command
pub async fn run_prj_command(
    client: &TfeClient,
    cli: &Cli,
) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Get {
        resource: GetResource::Prj(args),
    } = &cli.command
    else {
        unreachable!()
    };

    // Determine if we need workspace info (any of the flags)
    let need_ws_info =
        args.with_ws || args.with_ws_names || args.with_ws_ids || args.with_ws_details;

    // If NAME is specified, get single project
    if let Some(name) = &args.name {
        return get_single_project(client, cli, name, args.org.as_ref(), need_ws_info).await;
    }

    // Otherwise list all projects
    let organizations = resolve_organizations(client, args.org.as_ref()).await?;

    let spinner = create_spinner(
        &format!(
            "Fetching projects from {} organization(s)...",
            organizations.len()
        ),
        cli.batch,
    );

    // Fetch projects from all orgs in parallel
    let results = fetch_from_organizations(organizations, |org| async move {
        if need_ws_info {
            // Fetch projects and workspaces IN PARALLEL
            let (projects_result, workspaces_result) =
                tokio::join!(client.get_projects(&org), client.get_workspaces(&org));

            match (projects_result, workspaces_result) {
                (Ok(projects), Ok(workspaces)) => {
                    // Build workspace info per project
                    let results: Vec<ProjectRow> = projects
                        .into_iter()
                        .map(|project| {
                            let ws_list: Vec<_> = workspaces
                                .iter()
                                .filter(|ws| ws.project_id() == Some(&project.id))
                                .cloned()
                                .collect();
                            (
                                org.clone(),
                                project,
                                ProjectWorkspaces::from_workspaces(ws_list),
                            )
                        })
                        .collect();
                    Ok(results)
                }
                (Err(e), _) => Err((org, e)),
                (_, Err(e)) => Err((org, e)),
            }
        } else {
            // Just fetch projects, no workspace info
            match client.get_projects(&org).await {
                Ok(projects) => {
                    let results: Vec<ProjectRow> = projects
                        .into_iter()
                        .map(|project| (org.clone(), project, ProjectWorkspaces::new()))
                        .collect();
                    Ok(results)
                }
                Err(e) => Err((org, e)),
            }
        }
    })
    .await;

    let (project_batches, had_errors) = collect_org_results(results, &spinner, "projects");
    let mut all_projects: Vec<ProjectRow> = project_batches.into_iter().flatten().collect();

    finish_spinner_with_status(spinner, &all_projects, had_errors);

    // Apply filter if specified
    if let Some(filter) = &args.filter {
        let filter_lower = filter.to_lowercase();
        all_projects.retain(|(_, prj, _)| prj.name().to_lowercase().contains(&filter_lower));
    }

    // Sort projects
    let group_by_org = args.org.is_none() && !args.no_group_org;
    all_projects.sort_by(|a, b| {
        if group_by_org {
            let org_cmp = a.0.cmp(&b.0);
            if org_cmp != std::cmp::Ordering::Equal {
                return org_cmp;
            }
        }
        match args.sort {
            PrjSortField::Name => a.1.name().cmp(b.1.name()),
            PrjSortField::Workspaces => a.2.count().cmp(&b.2.count()),
        }
    });

    if args.reverse {
        all_projects.reverse();
    }

    if !all_projects.is_empty() {
        output_projects(&all_projects, cli);
    }

    log_completion(had_errors);
    Ok(())
}

/// Get a single project by name or ID
async fn get_single_project(
    client: &TfeClient,
    cli: &Cli,
    name: &str,
    org: Option<&String>,
    need_ws_info: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Get {
        resource: GetResource::Prj(args),
    } = &cli.command
    else {
        unreachable!()
    };

    // If it's an ID (prj-...), we can fetch directly without knowing the org
    if name.starts_with("prj-") {
        let spinner = create_spinner(&format!("Fetching project '{}'...", name), cli.batch);

        match client.get_project_by_id(name).await {
            Ok(Some((project, raw))) => {
                finish_spinner(spinner, "Found");

                // For JSON/YAML, return raw API response
                if matches!(args.output, OutputFormat::Json | OutputFormat::Yaml) {
                    output_raw(&raw, &args.output);
                    return Ok(());
                }

                // Extract org name from raw JSON response
                let org_name = raw["data"]["relationships"]["organization"]["data"]["id"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string();

                // For ID-based lookup, we can now get workspace info if org is known
                let ws_info = if need_ws_info && org_name != "unknown" {
                    let workspaces = client.get_workspaces(&org_name).await.unwrap_or_default();
                    let ws_list: Vec<_> = workspaces
                        .into_iter()
                        .filter(|ws| ws.project_id() == Some(&project.id))
                        .collect();
                    ProjectWorkspaces::from_workspaces(ws_list)
                } else {
                    ProjectWorkspaces::new()
                };

                let all_projects = vec![(org_name, project, ws_info)];
                output_projects(&all_projects, cli);
                return Ok(());
            }
            Ok(None) => {
                finish_spinner(spinner, "Not found");
                return Err(format!("Project '{}' not found", name).into());
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
            "Searching for project '{}' in {} organization(s)...",
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
            let prj_name = name_owned.clone();
            async move {
                let result = client.get_project_by_name(&org, &prj_name).await;
                (org, result)
            }
        })
        .collect();

    // Process results as they complete, stop on first match
    while let Some((org_name, result)) = futures.next().await {
        if let Ok(Some((project, raw))) = result {
            finish_spinner(spinner, "Found");

            // For JSON/YAML, return raw API response
            if matches!(args.output, OutputFormat::Json | OutputFormat::Yaml) {
                output_raw(&raw, &args.output);
                return Ok(());
            }

            // Get workspace info if requested
            let ws_info = if need_ws_info {
                let workspaces = client.get_workspaces(&org_name).await.unwrap_or_default();
                let ws_list: Vec<_> = workspaces
                    .into_iter()
                    .filter(|ws| ws.project_id() == Some(&project.id))
                    .collect();
                ProjectWorkspaces::from_workspaces(ws_list)
            } else {
                ProjectWorkspaces::new()
            };

            let all_projects = vec![(org_name, project, ws_info)];
            output_projects(&all_projects, cli);
            return Ok(());
        }
    }

    finish_spinner(spinner, "Not found");

    let searched = if organizations.len() == 1 {
        format!("organization '{}'", organizations[0])
    } else {
        format!("{} organizations", organizations.len())
    };

    Err(format!("Project '{}' not found in {}", name, searched).into())
}
