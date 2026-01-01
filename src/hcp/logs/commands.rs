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
use crate::hcp::TfeClient;
use crate::ui::{create_spinner, finish_spinner};
use crate::Cli;

/// Target type for logs command
enum LogTarget {
    /// Direct run ID (run-xxx)
    Run(String),
    /// Workspace ID (ws-xxx)
    WorkspaceId(String),
    /// Workspace name (requires org)
    WorkspaceName(String),
}

/// Parse target string to determine type
fn parse_target(target: &str) -> LogTarget {
    if target.starts_with("run-") {
        LogTarget::Run(target.to_string())
    } else if target.starts_with("ws-") {
        LogTarget::WorkspaceId(target.to_string())
    } else {
        LogTarget::WorkspaceName(target.to_string())
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
/// - Workspace ID/name: fetch workspace, extract current-run ID
async fn resolve_run_id(
    client: &TfeClient,
    cli: &Cli,
    args: &LogsArgs,
) -> Result<String, Box<dyn std::error::Error>> {
    match parse_target(&args.target) {
        LogTarget::Run(run_id) => Ok(run_id),

        LogTarget::WorkspaceId(ws_id) => {
            let spinner = create_spinner("Resolving workspace...", cli.batch);
            let result = resolve_workspace_by_id(client, &ws_id).await;
            match &result {
                Ok(_) => finish_spinner(spinner, "Found"),
                Err(_) => finish_spinner(spinner, "Not found"),
            }
            result
        }

        LogTarget::WorkspaceName(name) => {
            let org = args
                .org
                .as_ref()
                .ok_or("Organization (--org) is required when target is a workspace name")?;

            let spinner = create_spinner("Resolving workspace...", cli.batch);
            let result = resolve_workspace_by_name(client, org, &name).await;
            match &result {
                Ok(_) => finish_spinner(spinner, "Found"),
                Err(_) => finish_spinner(spinner, "Not found"),
            }
            result
        }
    }
}

/// Resolve workspace by ID and extract current-run ID (testable without UI)
async fn resolve_workspace_by_id(
    client: &TfeClient,
    ws_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    match client.get_workspace_by_id(ws_id).await? {
        Some((_ws, raw)) => extract_current_run_id(&raw),
        None => Err(format!("Workspace '{}' not found", ws_id).into()),
    }
}

/// Resolve workspace by name and extract current-run ID (testable without UI)
async fn resolve_workspace_by_name(
    client: &TfeClient,
    org: &str,
    name: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    match client.get_workspace_by_name(org, name).await? {
        Some((_ws, raw)) => extract_current_run_id(&raw),
        None => Err(format!("Workspace '{}' not found in organization '{}'", name, org).into()),
    }
}

/// Extract current-run ID from workspace raw JSON
fn extract_current_run_id(
    ws_raw: &serde_json::Value,
) -> Result<String, Box<dyn std::error::Error>> {
    // Path: data.relationships.current-run.data.id
    ws_raw["data"]["relationships"]["current-run"]["data"]["id"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Workspace has no current run".into())
}

#[cfg(test)]
mod tests {
    use super::*;
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
                    }
                }
            }
        })
    }

    #[test]
    fn test_parse_target_run_id() {
        match parse_target("run-abc123") {
            LogTarget::Run(id) => assert_eq!(id, "run-abc123"),
            _ => panic!("Expected Run variant"),
        }
    }

    #[test]
    fn test_parse_target_workspace_id() {
        match parse_target("ws-abc123") {
            LogTarget::WorkspaceId(id) => assert_eq!(id, "ws-abc123"),
            _ => panic!("Expected WorkspaceId variant"),
        }
    }

    #[test]
    fn test_parse_target_workspace_name() {
        match parse_target("my-workspace") {
            LogTarget::WorkspaceName(name) => assert_eq!(name, "my-workspace"),
            _ => panic!("Expected WorkspaceName variant"),
        }
    }

    #[test]
    fn test_parse_target_workspace_name_with_numbers() {
        match parse_target("prod-workspace-01") {
            LogTarget::WorkspaceName(name) => assert_eq!(name, "prod-workspace-01"),
            _ => panic!("Expected WorkspaceName variant"),
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

    #[tokio::test]
    async fn test_resolve_workspace_by_id_success() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(&mock_server.uri());

        let response = workspace_response_with_current_run("ws-abc123", "run-xyz789");

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-abc123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response))
            .mount(&mock_server)
            .await;

        let result = resolve_workspace_by_id(&client, "ws-abc123").await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "run-xyz789");
    }

    #[tokio::test]
    async fn test_resolve_workspace_by_id_not_found() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-notfound"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let result = resolve_workspace_by_id(&client, "ws-notfound").await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_resolve_workspace_by_id_no_current_run() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(&mock_server.uri());

        let response = workspace_response_without_current_run("ws-abc123");

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-abc123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response))
            .mount(&mock_server)
            .await;

        let result = resolve_workspace_by_id(&client, "ws-abc123").await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no current run"));
    }

    #[tokio::test]
    async fn test_resolve_workspace_by_name_success() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(&mock_server.uri());

        let response = workspace_response_with_current_run("ws-abc123", "run-xyz789");

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/workspaces/my-workspace"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response))
            .mount(&mock_server)
            .await;

        let result = resolve_workspace_by_name(&client, "my-org", "my-workspace").await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "run-xyz789");
    }

    #[tokio::test]
    async fn test_resolve_workspace_by_name_not_found() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/workspaces/unknown-ws"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let result = resolve_workspace_by_name(&client, "my-org", "unknown-ws").await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("not found"));
        assert!(err_msg.contains("my-org"));
    }

    #[tokio::test]
    async fn test_resolve_workspace_by_name_no_current_run() {
        let mock_server = MockServer::start().await;
        let client = create_test_client(&mock_server.uri());

        let response = workspace_response_without_current_run("ws-abc123");

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/workspaces/my-workspace"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response))
            .mount(&mock_server)
            .await;

        let result = resolve_workspace_by_name(&client, "my-org", "my-workspace").await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no current run"));
    }
}
