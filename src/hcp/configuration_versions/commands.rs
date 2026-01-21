//! Download configuration command handler

use std::path::PathBuf;

use log::debug;

use crate::cli::{Cli, Command, DownloadResource};
use crate::error::TfeError;
use crate::hcp::workspaces::resolve_workspace;
use crate::hcp::TfeClient;
use crate::ui::{create_spinner, finish_spinner, finish_spinner_with_message};

/// Run the download config command
pub async fn run_download_config_command(
    client: &TfeClient,
    cli: &Cli,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let Command::Download {
        resource: DownloadResource::Config(args),
    } = &cli.command
    else {
        unreachable!()
    };

    // Resolve workspace
    let resolved =
        resolve_workspace(client, &args.workspace, args.org.as_deref(), cli.batch).await?;
    let workspace_id = &resolved.workspace.id;

    debug!(
        "Resolved workspace: {} ({})",
        resolved.workspace.attributes.name, workspace_id
    );

    // Get configuration version
    let cv = if let Some(cv_id) = &args.cv_id {
        // Specific CV requested
        let sp = create_spinner(
            &format!("Fetching configuration version {}...", cv_id),
            cli.batch,
        );
        let cv = client.get_configuration_version(cv_id).await?;
        finish_spinner(sp);

        if !cv.is_downloadable() {
            return Err(Box::new(TfeError::Api {
                status: 400,
                message: format!(
                    "Configuration version '{}' is not downloadable (status: {})",
                    cv_id, cv.attributes.status
                ),
            }));
        }
        cv
    } else {
        // Find latest downloadable CV
        let sp = create_spinner("Finding latest configuration version...", cli.batch);
        let cv = client
            .get_latest_configuration_version(workspace_id)
            .await?
            .ok_or_else(|| TfeError::Api {
                status: 404,
                message: format!(
                    "No downloadable configuration version found for workspace '{}'",
                    resolved.workspace.attributes.name
                ),
            })?;
        finish_spinner(sp);
        cv
    };

    // Determine output path
    let output_path = args
        .output
        .clone()
        .unwrap_or_else(|| PathBuf::from(format!("configuration-{}.tar.gz", cv.id)));

    // Download configuration
    let sp = create_spinner(
        &format!("Downloading configuration to {}...", output_path.display()),
        cli.batch,
    );

    let size = client
        .download_configuration(&cv.id, &output_path)
        .await
        .inspect_err(|_e| {
            finish_spinner_with_message(sp.clone(), "Download failed");
        })?;

    finish_spinner_with_message(
        sp,
        &format!("Downloaded {} bytes to {}", size, output_path.display()),
    );

    println!();
    println!("Configuration downloaded successfully:");
    println!(
        "  Workspace: {} ({})",
        resolved.workspace.attributes.name, workspace_id
    );
    println!("  Configuration Version: {}", cv.id);
    println!("  Source: {}", cv.source());
    println!("  Status: {}", cv.attributes.status);
    println!("  Output: {}", output_path.display());
    println!("  Size: {} bytes", size);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_output_path() {
        let cv_id = "cv-abc123";
        let path = PathBuf::from(format!("configuration-{}.tar.gz", cv_id));
        assert_eq!(path.to_str().unwrap(), "configuration-cv-abc123.tar.gz");
    }

    #[test]
    fn test_custom_output_path() {
        let custom = PathBuf::from("/tmp/my-config.tar.gz");
        assert_eq!(custom.to_str().unwrap(), "/tmp/my-config.tar.gz");
    }
}
