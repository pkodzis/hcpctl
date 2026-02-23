//! Set workspace command handlers

use log::debug;

use crate::cli::{Cli, Command, SetResource};
use crate::hcp::projects::resolve_project;
use crate::hcp::traits::TfeResource;
use crate::hcp::workspaces::resolve_workspace;
use crate::hcp::TfeClient;
use crate::ui::{confirm_action, create_spinner, finish_spinner};

/// Run the set ws command (assign workspace to project)
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
        "Set workspace '{}' to project '{}'",
        args.workspace, args.project
    );

    let effective_org = client.effective_org(args.org.as_ref());

    // 1. Resolve workspace
    let resolved_ws =
        resolve_workspace(client, &args.workspace, effective_org.as_deref(), cli.batch).await?;

    let ws_id = &resolved_ws.workspace.id;
    let ws_name = resolved_ws.workspace.name().to_string();
    let org = &resolved_ws.org;

    // 2. Resolve project
    let resolved_prj = resolve_project(client, &args.project, org, cli.batch).await?;
    let project_id = &resolved_prj.project.id;
    let project_name = resolved_prj.project.name().to_string();

    // 3. Check if already assigned to this project + resolve current project name
    let current_project_display = if let Some(current_prj_id) = resolved_ws.workspace.project_id() {
        if current_prj_id == project_id {
            println!(
                "Workspace '{}' is already assigned to project '{}'",
                ws_name, project_name
            );
            return Ok(());
        }

        // Resolve current project name for display
        let current_name = match client.get_project_by_id(current_prj_id).await {
            Ok(Some((prj, _))) => prj.name().to_string(),
            _ => "unknown".to_string(),
        };
        Some(format!("'{}' ({})", current_name, current_prj_id))
    } else {
        None
    };

    // 4. Confirm
    let current_info = match &current_project_display {
        Some(display) => format!(" (current project: {})", display),
        None => String::new(),
    };

    let prompt = format!(
        "Assign workspace '{}' ({}) to project '{}' ({}){}?",
        ws_name, ws_id, project_name, project_id, current_info
    );

    if !confirm_action(&prompt, args.yes || cli.batch)? {
        println!("Cancelled");
        return Ok(());
    }

    // 5. Execute
    let spinner = create_spinner(
        &format!(
            "Assigning workspace '{}' to project '{}'...",
            ws_name, project_name
        ),
        cli.batch,
    );
    client
        .assign_workspace_to_project(ws_id, project_id)
        .await?;
    finish_spinner(spinner);

    println!(
        "✓ Workspace '{}' assigned to project '{}' ({})",
        ws_name, project_name, org
    );

    Ok(())
}
