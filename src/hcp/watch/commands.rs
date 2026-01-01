//! Watch command handlers
//!
//! Provides `hcpctl watch ws` command for continuously monitoring workspaces
//! and streaming logs for new runs as they appear.

use std::collections::HashSet;
use std::time::Duration;

use log::debug;
use tokio::time::sleep;

use crate::cli::WatchWsArgs;
use crate::hcp::runs::{print_log_with_prefix, Run};
use crate::hcp::traits::TfeResource;
use crate::hcp::workspaces::{extract_current_run_id, resolve_workspace};
use crate::hcp::TfeClient;
use crate::Cli;

/// Run the watch ws command
///
/// Continuously monitors a workspace for new runs and streams their logs.
pub async fn run_watch_ws_command(
    client: &TfeClient,
    cli: &Cli,
    args: &WatchWsArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    // Resolve workspace using shared resolver
    let resolved = resolve_workspace(client, &args.target, args.org.as_deref(), cli.batch).await?;

    println!(
        "Watching workspace '{}' in organization '{}' ({})",
        resolved.workspace.name(),
        resolved.org,
        resolved.workspace.id
    );
    println!("Press Ctrl+C to stop\n");

    // Track watched run IDs to avoid re-watching
    let mut watched_run_ids: HashSet<String> = HashSet::new();

    let poll_interval = Duration::from_secs(args.interval);

    loop {
        // Check for current run using shared utility
        match get_current_run(client, &resolved.workspace.id).await? {
            Some(run) => {
                let run_id = run.id.clone();

                // Only watch if not already watched
                if !watched_run_ids.contains(&run_id) {
                    debug!("New run detected: {}", run_id);

                    // Watch this run's logs
                    watch_run_logs(client, args, &run_id).await?;

                    // Mark as watched
                    watched_run_ids.insert(run_id.clone());

                    debug!("Run {} completed, returning to watch loop", run_id);
                }
            }
            None => {
                debug!("No current run, waiting...");
            }
        }

        // Wait before next poll
        sleep(poll_interval).await;
    }
}

/// Get current run from workspace (if any)
///
/// Uses shared `extract_current_run_id` to parse workspace response.
async fn get_current_run(
    client: &TfeClient,
    ws_id: &str,
) -> Result<Option<Run>, Box<dyn std::error::Error>> {
    // Fetch workspace to get current-run relationship
    match client.get_workspace_by_id(ws_id).await? {
        Some((_ws, raw)) => {
            // Use shared utility to extract run ID
            match extract_current_run_id(&raw) {
                Ok(run_id) => {
                    // Fetch the run to check its status
                    match client.get_run_by_id(&run_id).await? {
                        Some((run, _raw)) => Ok(Some(run)),
                        None => Ok(None),
                    }
                }
                Err(_) => Ok(None), // No current run
            }
        }
        None => Err(format!("Workspace '{}' not found", ws_id).into()),
    }
}

/// Watch a single run's logs until completion
async fn watch_run_logs(
    client: &TfeClient,
    args: &WatchWsArgs,
    run_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let poll_interval = Duration::from_secs(args.interval);
    let resource_name = if args.apply { "apply" } else { "plan" };

    // Print run start message
    let prefix = if args.no_prefix {
        println!("--- Run {} started ({}) ---", run_id, resource_name);
        None
    } else {
        println!("[{}] --- Run started ({}) ---", run_id, resource_name);
        Some(run_id)
    };

    let mut last_log_len = 0;

    loop {
        // Fetch log URL and final state based on resource type
        let (log_url, is_final) = if args.apply {
            let apply = client.get_run_apply(run_id).await?;
            (apply.attributes.log_read_url.clone(), apply.is_final())
        } else {
            let plan = client.get_run_plan(run_id).await?;
            (plan.attributes.log_read_url.clone(), plan.is_final())
        };

        // Fetch and display new log content
        if let Some(url) = &log_url {
            if let Ok(content) = client.get_log_content(url).await {
                if content.len() > last_log_len {
                    // Print new content using shared utility
                    let new_content = &content[last_log_len..];
                    print_log_with_prefix(new_content, prefix, args.raw);
                    last_log_len = content.len();
                }
            }
        }

        // Check if resource has reached final state
        if is_final {
            break;
        }

        sleep(poll_interval).await;
    }

    // Print run end message
    if args.no_prefix {
        println!("--- Run {} completed ---\n", run_id);
    } else {
        println!("[{}] --- Run completed ---\n", run_id);
    }

    Ok(())
}

// Log parsing functions moved to runs/log_utils.rs for reuse

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hcp::runs::extract_log_message;

    // Test extract_log_message via shared module
    #[test]
    fn test_extract_log_message_json_with_message() {
        let line = r#"{"@level":"info","@message":"Plan: 1 to add","type":"planned_change"}"#;
        assert_eq!(extract_log_message(line), "Plan: 1 to add");
    }

    #[test]
    fn test_extract_log_message_json_without_message() {
        let line = r#"{"@level":"info","type":"something"}"#;
        assert_eq!(extract_log_message(line), "");
    }

    #[test]
    fn test_extract_log_message_plain_text() {
        let line = "Terraform v1.12.2";
        assert_eq!(extract_log_message(line), "Terraform v1.12.2");
    }

    #[test]
    fn test_extract_log_message_invalid_json() {
        let line = "{invalid json}";
        assert_eq!(extract_log_message(line), "{invalid json}");
    }

    #[test]
    fn test_print_log_with_prefix() {
        // Uses shared function - just verify no panic
        print_log_with_prefix("test line\n", Some("run-abc"), false);
    }

    #[test]
    fn test_print_log_without_prefix() {
        print_log_with_prefix("test line\n", None, false);
    }

    #[test]
    fn test_print_log_raw_mode() {
        print_log_with_prefix(r#"{"@message":"test"}"#, Some("run-abc"), true);
    }

    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn create_test_client(base_url: &str) -> TfeClient {
        TfeClient::with_base_url(
            "test-token".to_string(),
            "test.example.com".to_string(),
            base_url.to_string(),
        )
    }

    fn workspace_response_with_current_run(ws_id: &str, run_id: &str) -> serde_json::Value {
        serde_json::json!({
            "data": {
                "id": ws_id,
                "type": "workspaces",
                "attributes": {
                    "name": "test-workspace"
                },
                "relationships": {
                    "current-run": {
                        "data": {
                            "id": run_id,
                            "type": "runs"
                        }
                    },
                    "organization": {
                        "data": {
                            "id": "org-test",
                            "type": "organizations"
                        }
                    }
                }
            }
        })
    }

    fn workspace_response_without_current_run(ws_id: &str) -> serde_json::Value {
        serde_json::json!({
            "data": {
                "id": ws_id,
                "type": "workspaces",
                "attributes": {
                    "name": "test-workspace"
                },
                "relationships": {
                    "current-run": {
                        "data": null
                    },
                    "organization": {
                        "data": {
                            "id": "org-test",
                            "type": "organizations"
                        }
                    }
                }
            }
        })
    }

    fn run_response(run_id: &str, status: &str) -> serde_json::Value {
        serde_json::json!({
            "data": {
                "id": run_id,
                "type": "runs",
                "attributes": {
                    "status": status,
                    "message": "Test run"
                }
            }
        })
    }

    #[tokio::test]
    async fn test_get_current_run_with_run() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-abc123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                workspace_response_with_current_run("ws-abc123", "run-xyz789"),
            ))
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/runs/run-xyz789"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(run_response("run-xyz789", "planning")),
            )
            .mount(&mock_server)
            .await;

        let result = get_current_run(&client, "ws-abc123").await;

        assert!(result.is_ok());
        let run = result.unwrap();
        assert!(run.is_some());
        assert_eq!(run.unwrap().id, "run-xyz789");
    }

    #[tokio::test]
    async fn test_get_current_run_no_run() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-abc123"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(workspace_response_without_current_run("ws-abc123")),
            )
            .mount(&mock_server)
            .await;

        let result = get_current_run(&client, "ws-abc123").await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_get_current_run_workspace_not_found() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-notfound"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let result = get_current_run(&client, "ws-notfound").await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }
}
