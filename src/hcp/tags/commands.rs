//! Tag binding command handlers

use log::debug;

use crate::cli::{
    classify_tags, parse_tags, Cli, Command, DeleteResource, DeleteTagResource, GetResource,
    GetTagResource, SetResource, SetTagResource,
};
use crate::hcp::projects::resolve_project;
use crate::hcp::tags::{TagTarget, TagTargetKind};
use crate::hcp::traits::TfeResource;
use crate::hcp::workspaces::resolve_workspace;
use crate::hcp::workspaces::WorkspaceQuery;
use crate::hcp::TfeClient;
use crate::output::{
    output_org_tags, output_org_tags_with_workspaces, output_tag_bindings,
    output_workspace_all_tags,
};
use crate::ui::{confirm_action, create_spinner, finish_spinner};

/// Run the set tag command (add/update tags)
pub async fn run_set_tag_command(
    client: &TfeClient,
    cli: &Cli,
) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Set {
        resource: SetResource::Tag { resource },
    } = &cli.command
    else {
        unreachable!()
    };

    match resource {
        SetTagResource::Ws(args) => {
            debug!("Setting tags on workspace '{}'", args.workspace);

            // Classify tags into flat string tags and key=value bindings
            let classified = classify_tags(&args.tags)?;

            // Resolve workspace
            let resolved =
                resolve_workspace(client, &args.workspace, args.org.as_deref(), cli.batch).await?;

            let ws_name = resolved.workspace.name().to_string();
            let ws_id = resolved.workspace.id.clone();

            // Build display string
            let mut tags_display: Vec<String> = classified.flat_tags.to_vec();
            tags_display.extend(
                classified
                    .bindings
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v)),
            );

            // Confirm
            let prompt = format!(
                "Set tag(s) [{}] on workspace '{}' ({})?",
                tags_display.join(", "),
                ws_name,
                ws_id
            );

            if !confirm_action(&prompt, args.yes || cli.batch)? {
                println!("Cancelled");
                return Ok(());
            }

            let spinner = create_spinner(
                &format!("Setting tags on workspace '{}'...", ws_name),
                cli.batch,
            );

            let mut flat_count = 0;
            let mut binding_count = 0;

            // Add flat string tags if any
            if !classified.flat_tags.is_empty() {
                client
                    .add_workspace_tags(&ws_id, &classified.flat_tags)
                    .await?;
                flat_count = classified.flat_tags.len();
            }

            // Add key=value tag bindings if any
            if !classified.bindings.is_empty() {
                let target = TagTarget {
                    kind: TagTargetKind::Workspace,
                    id: ws_id.clone(),
                    display_name: ws_name.clone(),
                };
                let result = client
                    .add_tag_bindings(&target, &classified.bindings)
                    .await?;
                binding_count = result.len();
            }

            finish_spinner(spinner);

            let total = flat_count + binding_count;
            println!(
                "✓ Set {} tag(s) on workspace '{}' ({})",
                total, ws_name, ws_id
            );
        }
        SetTagResource::Prj(args) => {
            debug!("Setting tags on project '{}'", args.project);

            // Validate: projects only support key=value tag bindings
            let flat_tags: Vec<&String> = args.tags.iter().filter(|t| !t.contains('=')).collect();
            if !flat_tags.is_empty() {
                return Err(format!(
                    "Projects only support key=value tag bindings. Invalid tag(s): {}\n\
                     Hint: use key=value format, e.g. 'env=prod'",
                    flat_tags
                        .iter()
                        .map(|t| format!("'{}'", t))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
                .into());
            }

            // Parse tags
            let tags = parse_tags(&args.tags)?;

            // Resolve org - need it for project name resolution
            let org = args
                .org
                .as_deref()
                .ok_or("Organization (--org) is required for project tag operations")?;

            // Resolve project
            let resolved = resolve_project(client, &args.project, org, cli.batch).await?;

            let prj_name = resolved.project.name().to_string();
            let prj_id = resolved.project.id.clone();

            let tags_display: Vec<String> =
                tags.iter().map(|(k, v)| format!("{}={}", k, v)).collect();

            // Confirm
            let prompt = format!(
                "Set tag(s) [{}] on project '{}' ({})?",
                tags_display.join(", "),
                prj_name,
                prj_id
            );

            if !confirm_action(&prompt, args.yes || cli.batch)? {
                println!("Cancelled");
                return Ok(());
            }

            let target = TagTarget {
                kind: TagTargetKind::Project,
                id: prj_id.clone(),
                display_name: prj_name.clone(),
            };

            let spinner = create_spinner(
                &format!("Setting tags on project '{}'...", prj_name),
                cli.batch,
            );
            let result = client.add_tag_bindings(&target, &tags).await?;
            finish_spinner(spinner);

            println!(
                "✓ Set {} tag(s) on project '{}' ({})",
                result.len(),
                prj_name,
                prj_id
            );
        }
    }

    Ok(())
}

/// Run the get tag command (list tags)
pub async fn run_get_tag_command(
    client: &TfeClient,
    cli: &Cli,
) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Get {
        resource: GetResource::Tag(ref tag_args),
    } = &cli.command
    else {
        unreachable!()
    };

    match &tag_args.resource {
        Some(GetTagResource::Ws(args)) => {
            debug!("Getting tags for workspace '{}'", args.workspace);

            // Resolve workspace
            let resolved =
                resolve_workspace(client, &args.workspace, tag_args.org.as_deref(), cli.batch)
                    .await?;

            let ws_name = resolved.workspace.name().to_string();
            let ws_id = resolved.workspace.id.clone();

            let target = TagTarget {
                kind: TagTargetKind::Workspace,
                id: ws_id.clone(),
                display_name: ws_name.clone(),
            };

            let spinner = create_spinner(
                &format!("Fetching tags for workspace '{}'...", ws_name),
                cli.batch,
            );
            let tag_bindings = client.get_tag_bindings(&target).await?;
            let workspace_tags = client.get_workspace_tags(&ws_id).await?;
            finish_spinner(spinner);

            if tag_bindings.is_empty() && workspace_tags.is_empty() {
                println!("No tags found on workspace '{}'", ws_name);
            } else {
                output_workspace_all_tags(
                    &workspace_tags,
                    &tag_bindings,
                    &tag_args.output,
                    cli.no_header,
                );
            }
        }
        Some(GetTagResource::Prj(args)) => {
            debug!("Getting tags for project '{}'", args.project);

            let org = tag_args
                .org
                .as_deref()
                .ok_or("Organization (--org) is required for project tag operations")?;

            // Resolve project
            let resolved = resolve_project(client, &args.project, org, cli.batch).await?;

            let prj_name = resolved.project.name().to_string();
            let prj_id = resolved.project.id.clone();

            let target = TagTarget {
                kind: TagTargetKind::Project,
                id: prj_id,
                display_name: prj_name.clone(),
            };

            let spinner = create_spinner(
                &format!("Fetching tags for project '{}'...", prj_name),
                cli.batch,
            );
            let tags = client.get_tag_bindings(&target).await?;
            finish_spinner(spinner);

            if tags.is_empty() {
                println!("No tags found on project '{}'", prj_name);
            } else {
                output_tag_bindings(&tags, &tag_args.output, cli.no_header);
            }
        }
        None => {
            // Org-level tag listing
            let org = tag_args
                .org
                .as_deref()
                .ok_or("Organization (--org) is required to list organization tags")?;

            // Use positional name or --filter as search query
            let search = tag_args.name.as_deref().or(tag_args.filter.as_deref());

            debug!("Listing all tags for organization '{}'", org);

            let spinner = create_spinner(
                &format!("Fetching tags for organization '{}'...", org),
                cli.batch,
            );
            let tags = client.get_org_tags(org, search).await?;
            finish_spinner(spinner);

            if tags.is_empty() {
                println!("No tags found in organization '{}'", org);
            } else if tag_args.name.is_some() {
                // Specific tag lookup — also fetch associated workspaces
                let tag_name = tag_args.name.as_deref().unwrap();

                let ws_spinner = create_spinner(
                    &format!("Fetching workspaces with tag '{}'...", tag_name),
                    cli.batch,
                );
                let query = WorkspaceQuery {
                    search_tags: Some(tag_name),
                    ..Default::default()
                };
                let workspaces = client.get_workspaces(org, query).await?;
                finish_spinner(ws_spinner);

                output_org_tags_with_workspaces(
                    &tags,
                    &workspaces,
                    &tag_args.output,
                    cli.no_header,
                );
            } else {
                output_org_tags(&tags, &tag_args.output, cli.no_header);
            }
        }
    }

    Ok(())
}

/// Run the delete tag command (remove tags)
pub async fn run_delete_tag_command(
    client: &TfeClient,
    cli: &Cli,
) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Delete {
        resource: DeleteResource::Tag { resource },
    } = &cli.command
    else {
        unreachable!()
    };

    match resource {
        DeleteTagResource::Ws(args) => {
            debug!(
                "Deleting tags {:?} from workspace '{}'",
                args.keys, args.workspace
            );

            // Resolve workspace
            let resolved =
                resolve_workspace(client, &args.workspace, args.org.as_deref(), cli.batch).await?;

            let ws_name = resolved.workspace.name().to_string();
            let ws_id = resolved.workspace.id.clone();

            // Confirm
            let prompt = format!(
                "Remove tag(s) [{}] from workspace '{}' ({})?",
                args.keys.join(", "),
                ws_name,
                ws_id
            );

            if !confirm_action(&prompt, args.yes || cli.batch)? {
                println!("Cancelled");
                return Ok(());
            }

            let target = TagTarget {
                kind: TagTargetKind::Workspace,
                id: ws_id.clone(),
                display_name: ws_name.clone(),
            };

            let spinner = create_spinner(
                &format!("Removing tags from workspace '{}'...", ws_name),
                cli.batch,
            );

            // Get current tag bindings to check which keys exist as bindings
            let current_bindings = client.get_tag_bindings(&target).await?;
            let binding_keys: Vec<String> = args
                .keys
                .iter()
                .filter(|k| current_bindings.iter().any(|t| &t.attributes.key == *k))
                .cloned()
                .collect();
            let flat_tag_names: Vec<String> = args
                .keys
                .iter()
                .filter(|k| !current_bindings.iter().any(|t| &t.attributes.key == *k))
                .cloned()
                .collect();

            // Remove key-value tag bindings
            if !binding_keys.is_empty() {
                client.remove_tag_bindings(&target, &binding_keys).await?;
            }

            // Remove flat string tags (DELETE is idempotent, safe even if not present)
            if !flat_tag_names.is_empty() {
                client
                    .remove_workspace_tags(&ws_id, &flat_tag_names)
                    .await?;
            }

            finish_spinner(spinner);

            println!(
                "✓ Removed {} tag(s) from workspace '{}' ({})",
                args.keys.len(),
                ws_name,
                ws_id
            );
        }
        DeleteTagResource::Prj(args) => {
            debug!(
                "Deleting tags {:?} from project '{}'",
                args.keys, args.project
            );

            let org = args
                .org
                .as_deref()
                .ok_or("Organization (--org) is required for project tag operations")?;

            // Resolve project
            let resolved = resolve_project(client, &args.project, org, cli.batch).await?;

            let prj_name = resolved.project.name().to_string();
            let prj_id = resolved.project.id.clone();

            // Confirm
            let prompt = format!(
                "Remove tag(s) [{}] from project '{}' ({})?",
                args.keys.join(", "),
                prj_name,
                prj_id
            );

            if !confirm_action(&prompt, args.yes || cli.batch)? {
                println!("Cancelled");
                return Ok(());
            }

            let target = TagTarget {
                kind: TagTargetKind::Project,
                id: prj_id.clone(),
                display_name: prj_name.clone(),
            };

            let spinner = create_spinner(
                &format!("Removing tags from project '{}'...", prj_name),
                cli.batch,
            );
            client.remove_tag_bindings(&target, &args.keys).await?;
            finish_spinner(spinner);

            println!(
                "✓ Removed {} tag(s) from project '{}' ({})",
                args.keys.len(),
                prj_name,
                prj_id
            );
        }
    }

    Ok(())
}
