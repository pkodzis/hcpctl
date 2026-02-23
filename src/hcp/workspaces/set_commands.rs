//! Set workspace command handlers

use log::debug;

use crate::cli::{Cli, Command, SetResource};
use crate::hcp::projects::resolve_project;
use crate::hcp::traits::TfeResource;
use crate::hcp::workspaces::resolve_workspace;
use crate::hcp::TfeClient;
use crate::ui::{confirm_action, create_spinner, finish_spinner};

/// Run the set ws command (modify workspace settings)
pub async fn run_set_ws_command(
    client: &TfeClient,
    cli: &Cli,
) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Set {
        resource: SetResource::Ws(args),
    } = &cli.command
    else {
        unreachable!()
    };

    debug!(
        "Set workspace '{}' (project={:?}, terraform_version={:?})",
        args.workspace, args.project, args.terraform_version
    );

    // Validate terraform_version is not empty/whitespace if provided
    if let Some(ref tf_ver) = args.terraform_version {
        if tf_ver.trim().is_empty() {
            return Err("--terraform-version cannot be empty".into());
        }
    }

    let effective_org = client.effective_org(args.org.as_ref());

    // 1. Resolve workspace
    let resolved_ws =
        resolve_workspace(client, &args.workspace, effective_org.as_deref(), cli.batch).await?;

    let ws_id = &resolved_ws.workspace.id;
    let ws_name = resolved_ws.workspace.name().to_string();
    let org = &resolved_ws.org;

    // 2. Check "already current" for terraform version
    let current_tf_version = resolved_ws.workspace.terraform_version().to_string();
    let tf_version_to_set = if let Some(ref requested_version) = args.terraform_version {
        if current_tf_version == *requested_version {
            println!(
                "Workspace '{}' ({}) already has terraform version '{}'",
                ws_name, ws_id, requested_version
            );
            None
        } else {
            Some(requested_version.clone())
        }
    } else {
        None
    };

    // 3. Resolve project and check "already current" (only if --prj provided)
    let project_to_set = if let Some(ref requested_project) = args.project {
        let resolved_prj = resolve_project(client, requested_project, org, cli.batch).await?;
        let project_id = resolved_prj.project.id.clone();
        let project_name = resolved_prj.project.name().to_string();

        if let Some(current_prj_id) = resolved_ws.workspace.project_id() {
            if current_prj_id == project_id {
                println!(
                    "Workspace '{}' ({}) is already assigned to project '{}' ({})",
                    ws_name, ws_id, project_name, project_id
                );
                None
            } else {
                // Resolve current project name for display
                let current_display = match client.get_project_by_id(current_prj_id).await {
                    Ok(Some((prj, _))) => format!("'{}' ({})", prj.name(), current_prj_id),
                    _ => format!("unknown ({})", current_prj_id),
                };
                Some((project_id, project_name, current_display))
            }
        } else {
            Some((project_id, project_name, "none".to_string()))
        }
    } else {
        None
    };

    // 4. If everything is already current, return early
    if tf_version_to_set.is_none() && project_to_set.is_none() {
        return Ok(());
    }

    // 5. Build combined confirmation prompt
    let mut changes = Vec::new();
    if let Some(ref tf_ver) = tf_version_to_set {
        changes.push(format!(
            "terraform version: '{}' → '{}'",
            current_tf_version, tf_ver
        ));
    }
    if let Some((ref prj_id, ref prj_name, ref current_display)) = project_to_set {
        changes.push(format!(
            "project: {} → '{}' ({})",
            current_display, prj_name, prj_id
        ));
    }

    let prompt = format!(
        "Update workspace '{}' ({}):\n  {}\nContinue?",
        ws_name,
        ws_id,
        changes.join("\n  ")
    );

    if !confirm_action(&prompt, args.yes || cli.batch)? {
        println!("Cancelled");
        return Ok(());
    }

    // 6. Execute single PATCH via update_workspace
    let spinner = create_spinner(&format!("Updating workspace '{}'...", ws_name), cli.batch);
    client
        .update_workspace(
            ws_id,
            tf_version_to_set.as_deref(),
            project_to_set.as_ref().map(|(id, _, _)| id.as_str()),
        )
        .await?;
    finish_spinner(spinner);

    // 7. Print success messages
    if let Some(ref tf_ver) = tf_version_to_set {
        println!(
            "✓ Workspace '{}' ({}) terraform version set to '{}' ({})",
            ws_name, ws_id, tf_ver, org
        );
    }
    if let Some((ref prj_id, ref prj_name, _)) = project_to_set {
        println!(
            "✓ Workspace '{}' ({}) assigned to project '{}' ({}) ({})",
            ws_name, ws_id, prj_name, prj_id, org
        );
    }

    Ok(())
}
