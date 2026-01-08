//! State purge command handler

use std::io::{self, Write};

use log::debug;

use crate::error::Result;
use crate::hcp::state::models::EmptyTerraformState;
use crate::hcp::workspaces::{parse_workspace_target, WorkspaceTarget};
use crate::hcp::TfeClient;
use crate::ui::{create_spinner, finish_spinner, finish_spinner_with_message};
use crate::{Cli, Command, PurgeResource};

/// Warning message displayed before purging state
const PURGE_WARNING: &str = r#"
╔══════════════════════════════════════════════════════════════════════════════╗
║                               CRITICAL WARNING !!!                           ║
╠══════════════════════════════════════════════════════════════════════════════╣
║  You are about to PURGE all resources from this workspace's Terraform state. ║
║                                                                              ║
║  This operation will:                                                        ║
║    • Remove ALL resources from the state file                                ║
║    • NOT destroy actual infrastructure (resources will become orphaned)      ║
║    • Make Terraform "forget" about managed resources                         ║
║    • Be IRREVERSIBLE without manual state recovery                           ║
║                                                                              ║
║  The actual cloud resources will continue to exist but will no longer be     ║
║  tracked by Terraform. You may need to manually import or destroy them.      ║
╚══════════════════════════════════════════════════════════════════════════════╝
"#;

/// Workspace statistics
struct WorkspaceStats {
    name: String,
    resource_count: u32,
    serial: u64,
    resources_processed: bool,
}

/// Fetch workspace statistics from API
async fn fetch_workspace_stats(client: &TfeClient, workspace_id: &str) -> Result<WorkspaceStats> {
    // Get workspace for resource count and name
    let (workspace, _) = client
        .get_workspace_by_id(workspace_id)
        .await?
        .ok_or_else(|| crate::error::TfeError::Api {
            status: 404,
            message: format!("Workspace '{}' not found", workspace_id),
        })?;

    // Get current state version for serial and resources_processed
    let state_version = client.get_current_state_version(workspace_id).await?;

    Ok(WorkspaceStats {
        name: workspace.attributes.name.clone(),
        resource_count: workspace.resource_count(),
        serial: state_version.data.attributes.serial,
        resources_processed: state_version
            .data
            .attributes
            .resources_processed
            .unwrap_or(false),
    })
}

/// Display workspace statistics
fn print_workspace_stats(stats: &WorkspaceStats, workspace_id: &str, label: &str) {
    println!("    {}:", label);
    println!("      Workspace: {} ({})", stats.name, workspace_id);
    println!("      Resource count: {}", stats.resource_count);
    println!("      State serial: {}", stats.serial);
    println!(
        "      Resources processed: {}",
        if stats.resources_processed {
            "yes"
        } else {
            "no"
        }
    );
}

/// Run the purge state command
pub async fn run_purge_state_command(
    client: &TfeClient,
    cli: &Cli,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let Command::Purge {
        resource: PurgeResource::State(args),
    } = &cli.command
    else {
        unreachable!()
    };

    let workspace_id = &args.workspace_id;

    // Validate workspace ID format - must be ws-xxx, not workspace name
    match parse_workspace_target(workspace_id) {
        WorkspaceTarget::Id(_) => {} // Valid
        WorkspaceTarget::Name(_) => {
            return Err(format!(
                "Invalid workspace ID '{}'. Must be a workspace ID starting with 'ws-'.\n\
                 Workspace names are not supported for this operation - use the exact workspace ID.",
                workspace_id
            )
            .into());
        }
    }

    // Fetch and display BEFORE stats
    let spinner = create_spinner(
        &format!("Fetching workspace {}...", workspace_id),
        cli.batch,
    );
    let before_stats = fetch_workspace_stats(client, workspace_id).await?;
    finish_spinner_with_message(
        spinner,
        &format!("Workspace '{}' fetched", before_stats.name),
    );

    print_workspace_stats(&before_stats, workspace_id, "Current state (BEFORE)");

    if before_stats.resource_count == 0 {
        println!("\n✓ Workspace has no resources in state. Nothing to purge.");
        return Ok(());
    }

    // Get download URL from state version
    let spinner = create_spinner("Fetching state version...", cli.batch);
    let state_version = client.get_current_state_version(workspace_id).await?;
    let state_version_id = &state_version.data.id;
    finish_spinner_with_message(
        spinner,
        &format!("State version {} retrieved", state_version_id),
    );

    let download_url = state_version
        .data
        .attributes
        .hosted_state_download_url
        .as_ref()
        .ok_or(
            "No state download URL available. The workspace may have no state or use remote state storage.",
        )?;

    // Show critical warning - ALWAYS (--batch is ignored for this command)
    println!("{}", PURGE_WARNING);

    // Confirmation prompt - ALWAYS required (--batch and -y are ignored)
    print!(
        "Type the workspace ID '{}' to confirm purge: ",
        workspace_id
    );
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();

    if input != workspace_id {
        println!(
            "\nAborted. Input '{}' does not match '{}'.",
            input, workspace_id
        );
        return Ok(());
    }

    println!();

    // Step 1: Lock workspace
    let spinner = create_spinner(&format!("Locking workspace {}...", workspace_id), cli.batch);
    if let Err(e) = client.lock_workspace(workspace_id).await {
        finish_spinner(spinner);
        return Err(format!("Failed to lock workspace: {}", e).into());
    }
    finish_spinner_with_message(spinner, &format!("Workspace {} locked", workspace_id));
    debug!("Workspace locked");

    // From here, we need to unlock on any error
    let result =
        purge_state_internal(client, cli, workspace_id, state_version_id, download_url).await;

    // Step 5: Unlock workspace (always, even on error)
    let spinner = create_spinner(
        &format!("Unlocking workspace {}...", workspace_id),
        cli.batch,
    );
    if let Err(e) = client.unlock_workspace(workspace_id).await {
        finish_spinner(spinner);
        eprintln!("⚠️  Warning: Failed to unlock workspace: {}", e);
        eprintln!("   You may need to manually unlock the workspace.");
    } else {
        finish_spinner_with_message(spinner, &format!("Workspace {} unlocked", workspace_id));
        debug!("Workspace unlocked");
    }

    // Step 6: Fetch and display AFTER stats (only on success)
    if result.is_ok() {
        let spinner = create_spinner("Fetching final state...", cli.batch);
        match fetch_workspace_stats(client, workspace_id).await {
            Ok(stats) => {
                finish_spinner_with_message(spinner, "Final state retrieved");
                print_workspace_stats(&stats, workspace_id, "Final state (AFTER)");
            }
            Err(e) => {
                finish_spinner(spinner);
                debug!("Failed to fetch final stats: {}", e);
            }
        }
    }

    result
}

/// Internal function to perform the actual state purge
/// Separated to ensure unlock happens even on error
async fn purge_state_internal(
    client: &TfeClient,
    cli: &Cli,
    workspace_id: &str,
    state_version_id: &str,
    download_url: &str,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Step 2: Download current state
    let spinner = create_spinner(
        &format!("Downloading state {}...", state_version_id),
        cli.batch,
    );
    let current_state = client.download_state(download_url).await?;
    finish_spinner_with_message(
        spinner,
        &format!(
            "State {} downloaded (serial={}, {} resources)",
            state_version_id,
            current_state.serial,
            current_state.resources.len()
        ),
    );
    debug!(
        "Downloaded state: serial={}, lineage={}, resources={}",
        current_state.serial,
        current_state.lineage,
        current_state.resources.len()
    );

    let original_resource_count = current_state.resources.len();

    // Step 3: Create empty state
    let empty_state = EmptyTerraformState::from_current(&current_state);
    debug!(
        "Created empty state: serial={}, lineage={}",
        empty_state.serial, empty_state.lineage
    );

    // Step 4: Upload empty state
    let spinner = create_spinner(
        &format!("Uploading empty state (serial={})...", empty_state.serial),
        cli.batch,
    );
    client
        .upload_state_version(workspace_id, &empty_state)
        .await?;
    finish_spinner_with_message(
        spinner,
        &format!("Empty state uploaded (serial={})", empty_state.serial),
    );

    println!(
        "\n✓ Successfully purged {} resources from workspace '{}'",
        original_resource_count, workspace_id
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_purge_warning_contains_key_info() {
        // Verify warning message contains critical information
        assert!(PURGE_WARNING.contains("CRITICAL WARNING"));
        assert!(PURGE_WARNING.contains("PURGE"));
        assert!(PURGE_WARNING.contains("resources"));
        assert!(PURGE_WARNING.contains("IRREVERSIBLE"));
        assert!(PURGE_WARNING.contains("NOT destroy"));
        assert!(PURGE_WARNING.contains("orphaned"));
    }
}
