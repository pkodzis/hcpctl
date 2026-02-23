//! Logs command handler
//!
//! Provides `hcpctl logs` command for viewing run logs (plan or apply).
//! Supports multiple target types:
//! - Run ID (run-xxx): directly fetches logs
//! - Workspace ID (ws-xxx): fetches current-run logs
//! - Workspace name: fetches current-run logs (requires --org)

use log::debug;

use crate::cli::LogsArgs;
use crate::hcp::runs::{fetch_and_print_log, tail_log};
use crate::hcp::workspaces::{extract_current_run_id, resolve_workspace};
use crate::hcp::TfeClient;
use crate::Cli;

/// Target type for logs command
enum LogTarget {
    /// Direct run ID (run-xxx)
    Run(String),
    /// Workspace ID or name (ws-xxx or name)
    Workspace(String),
}

/// Parse target string to determine type
fn parse_target(target: &str) -> LogTarget {
    if target.starts_with("run-") {
        LogTarget::Run(target.to_string())
    } else {
        LogTarget::Workspace(target.to_string())
    }
}

/// Run the logs command
pub async fn run_logs_command(
    client: &TfeClient,
    cli: &Cli,
    args: &LogsArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    // Resolve run ID from target
    let run_id = resolve_run_id(client, cli, args).await?;

    debug!("Resolved run ID: {}", run_id);

    // Fetch and display logs
    if args.follow {
        tail_log(client, cli.batch, &run_id, args.apply, args.raw).await
    } else {
        fetch_and_print_log(client, &run_id, args.apply, args.raw).await
    }
}

/// Resolve target to run ID
///
/// - Run ID: return as-is
/// - Workspace ID/name: use shared resolver, extract current-run ID
async fn resolve_run_id(
    client: &TfeClient,
    cli: &Cli,
    args: &LogsArgs,
) -> Result<String, Box<dyn std::error::Error>> {
    match parse_target(&args.target) {
        LogTarget::Run(run_id) => Ok(run_id),

        LogTarget::Workspace(target) => {
            // Use shared workspace resolver
            let effective_org = client.effective_org(args.org.as_ref());
            let resolved =
                resolve_workspace(client, &target, effective_org.as_deref(), cli.batch).await?;
            extract_current_run_id(&resolved.raw)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_target_run_id() {
        match parse_target("run-abc123") {
            LogTarget::Run(id) => assert_eq!(id, "run-abc123"),
            _ => panic!("Expected Run variant"),
        }
    }

    #[test]
    fn test_parse_target_workspace_id() {
        // ws-xxx is now parsed as Workspace (unified handling)
        match parse_target("ws-abc123") {
            LogTarget::Workspace(id) => assert_eq!(id, "ws-abc123"),
            _ => panic!("Expected Workspace variant"),
        }
    }

    #[test]
    fn test_parse_target_workspace_name() {
        match parse_target("my-workspace") {
            LogTarget::Workspace(name) => assert_eq!(name, "my-workspace"),
            _ => panic!("Expected Workspace variant"),
        }
    }

    #[test]
    fn test_parse_target_workspace_name_with_numbers() {
        match parse_target("prod-workspace-01") {
            LogTarget::Workspace(name) => assert_eq!(name, "prod-workspace-01"),
            _ => panic!("Expected Workspace variant"),
        }
    }

    #[test]
    fn test_extract_current_run_id_success() {
        let raw = serde_json::json!({
            "data": {
                "relationships": {
                    "current-run": {
                        "data": {
                            "id": "run-xyz789",
                            "type": "runs"
                        }
                    }
                }
            }
        });

        let result = extract_current_run_id(&raw);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "run-xyz789");
    }

    #[test]
    fn test_extract_current_run_id_no_run() {
        let raw = serde_json::json!({
            "data": {
                "relationships": {
                    "current-run": {
                        "data": null
                    }
                }
            }
        });

        let result = extract_current_run_id(&raw);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no current run"));
    }

    #[test]
    fn test_extract_current_run_id_missing_relationship() {
        let raw = serde_json::json!({
            "data": {
                "relationships": {}
            }
        });

        let result = extract_current_run_id(&raw);
        assert!(result.is_err());
    }

    // Note: resolve_workspace tests moved to workspace_resolver module tests
}
