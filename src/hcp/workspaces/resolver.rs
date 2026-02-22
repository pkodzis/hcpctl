//! Workspace resolution utilities
//!
//! Provides shared functionality for resolving workspaces by ID or name,
//! with optional auto-discovery across organizations.

use futures::stream::{FuturesUnordered, StreamExt};
use log::debug;

use super::Workspace;
use crate::hcp::organizations::resolve_organizations;
use crate::hcp::TfeClient;
use crate::ui::{create_spinner, finish_spinner};

/// Resolved workspace information
#[derive(Debug)]
pub struct ResolvedWorkspace {
    /// The workspace model
    pub workspace: Workspace,
    /// Raw JSON response for extracting relationships
    pub raw: serde_json::Value,
    /// Organization name
    pub org: String,
}

/// Target type for workspace resolution
#[derive(Debug)]
pub enum WorkspaceTarget {
    /// Workspace ID (ws-xxx)
    Id(String),
    /// Workspace name
    Name(String),
}

/// Parse target string to determine workspace type
pub fn parse_workspace_target(target: &str) -> WorkspaceTarget {
    if target.starts_with("ws-") {
        WorkspaceTarget::Id(target.to_string())
    } else {
        WorkspaceTarget::Name(target.to_string())
    }
}

/// Resolve workspace by ID or name
///
/// # Arguments
/// * `client` - TFE API client
/// * `target` - Workspace ID (ws-xxx) or name
/// * `org` - Optional organization name (required for name lookup without auto-discovery)
/// * `batch` - If true, no spinners
///
/// # Returns
/// Resolved workspace info including workspace model, raw JSON, and organization name
pub async fn resolve_workspace(
    client: &TfeClient,
    target: &str,
    org: Option<&str>,
    batch: bool,
) -> Result<ResolvedWorkspace, Box<dyn std::error::Error>> {
    match parse_workspace_target(target) {
        WorkspaceTarget::Id(ws_id) => resolve_by_id(client, &ws_id, batch).await,
        WorkspaceTarget::Name(name) => {
            if let Some(org_name) = org {
                resolve_by_name(client, org_name, &name, batch).await
            } else {
                resolve_across_orgs(client, &name, batch).await
            }
        }
    }
}

/// Resolve workspace by ID
async fn resolve_by_id(
    client: &TfeClient,
    ws_id: &str,
    batch: bool,
) -> Result<ResolvedWorkspace, Box<dyn std::error::Error>> {
    let spinner = create_spinner("Resolving workspace...", batch);

    match client.get_workspace_by_id(ws_id).await? {
        Some((ws, raw)) => {
            finish_spinner(spinner);
            let org = ws.organization_name().unwrap_or("unknown").to_string();
            Ok(ResolvedWorkspace {
                workspace: ws,
                raw,
                org,
            })
        }
        None => {
            finish_spinner(spinner);
            Err(format!("Workspace '{}' not found", ws_id).into())
        }
    }
}

/// Resolve workspace by name in specific organization
async fn resolve_by_name(
    client: &TfeClient,
    org: &str,
    name: &str,
    batch: bool,
) -> Result<ResolvedWorkspace, Box<dyn std::error::Error>> {
    let spinner = create_spinner("Resolving workspace...", batch);

    match client.get_workspace_by_name(org, name).await? {
        Some((ws, raw)) => {
            finish_spinner(spinner);
            Ok(ResolvedWorkspace {
                workspace: ws,
                raw,
                org: org.to_string(),
            })
        }
        None => {
            finish_spinner(spinner);
            Err(format!("Workspace '{}' not found in organization '{}'", name, org).into())
        }
    }
}

/// Resolve workspace by searching across all organizations
async fn resolve_across_orgs(
    client: &TfeClient,
    name: &str,
    batch: bool,
) -> Result<ResolvedWorkspace, Box<dyn std::error::Error>> {
    let organizations = resolve_organizations(client, None).await?;

    debug!(
        "Searching for workspace '{}' across {} organization(s)",
        name,
        organizations.len()
    );

    let spinner = create_spinner(
        &format!(
            "Searching for workspace '{}' in {} organization(s)...",
            name,
            organizations.len()
        ),
        batch,
    );

    // Search in all organizations in parallel with early termination
    let name_owned = name.to_string();
    let mut futures: FuturesUnordered<_> = organizations
        .iter()
        .map(|org_name| {
            let org = org_name.clone();
            let ws_name = name_owned.clone();
            async move {
                let result = client.get_workspace_by_name(&org, &ws_name).await;
                (org, result)
            }
        })
        .collect();

    // Process results as they complete, stop on first match
    while let Some((org_name, result)) = futures.next().await {
        if let Ok(Some((ws, raw))) = result {
            finish_spinner(spinner);
            return Ok(ResolvedWorkspace {
                workspace: ws,
                raw,
                org: org_name,
            });
        }
    }

    finish_spinner(spinner);

    let searched = if organizations.len() == 1 {
        format!("organization '{}'", organizations[0])
    } else {
        format!("{} organizations", organizations.len())
    };

    Err(format!("Workspace '{}' not found in {}", name, searched).into())
}

/// Extract current-run ID from workspace raw JSON
pub fn extract_current_run_id(
    ws_raw: &serde_json::Value,
) -> Result<String, Box<dyn std::error::Error>> {
    ws_raw["data"]["relationships"]["current-run"]["data"]["id"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Workspace has no current run".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_workspace_target_id() {
        match parse_workspace_target("ws-abc123") {
            WorkspaceTarget::Id(id) => assert_eq!(id, "ws-abc123"),
            _ => panic!("Expected Id variant"),
        }
    }

    #[test]
    fn test_parse_workspace_target_name() {
        match parse_workspace_target("my-workspace") {
            WorkspaceTarget::Name(name) => assert_eq!(name, "my-workspace"),
            _ => panic!("Expected Name variant"),
        }
    }

    #[test]
    fn test_parse_workspace_target_name_with_numbers() {
        match parse_workspace_target("prod-workspace-01") {
            WorkspaceTarget::Name(name) => assert_eq!(name, "prod-workspace-01"),
            _ => panic!("Expected Name variant"),
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

    // Wiremock-based API tests
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn workspace_response(ws_id: &str, ws_name: &str, org: &str) -> serde_json::Value {
        serde_json::json!({
            "data": {
                "id": ws_id,
                "type": "workspaces",
                "attributes": {
                    "name": ws_name
                },
                "relationships": {
                    "organization": {
                        "data": {
                            "id": format!("org-{}", org),
                            "type": "organizations"
                        }
                    }
                }
            }
        })
    }

    #[tokio::test]
    async fn test_resolve_by_id_success() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-abc123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(workspace_response(
                "ws-abc123",
                "my-workspace",
                "my-org",
            )))
            .mount(&mock_server)
            .await;

        let result = resolve_by_id(&client, "ws-abc123", true).await;

        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert_eq!(resolved.workspace.id, "ws-abc123");
    }

    #[tokio::test]
    async fn test_resolve_by_id_not_found() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-notfound"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let result = resolve_by_id(&client, "ws-notfound", true).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_resolve_by_name_success() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/workspaces/my-workspace"))
            .respond_with(ResponseTemplate::new(200).set_body_json(workspace_response(
                "ws-abc123",
                "my-workspace",
                "my-org",
            )))
            .mount(&mock_server)
            .await;

        let result = resolve_by_name(&client, "my-org", "my-workspace", true).await;

        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert_eq!(resolved.workspace.id, "ws-abc123");
        assert_eq!(resolved.org, "my-org");
    }

    #[tokio::test]
    async fn test_resolve_by_name_not_found() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/workspaces/unknown"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let result = resolve_by_name(&client, "my-org", "unknown", true).await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"));
        assert!(err.contains("my-org"));
    }

    #[tokio::test]
    async fn test_resolve_workspace_with_id() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/workspaces/ws-abc123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(workspace_response(
                "ws-abc123",
                "my-workspace",
                "my-org",
            )))
            .mount(&mock_server)
            .await;

        let result = resolve_workspace(&client, "ws-abc123", None, true).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_resolve_workspace_with_name_and_org() {
        let mock_server = MockServer::start().await;
        let client = TfeClient::test_client(&mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/organizations/my-org/workspaces/my-workspace"))
            .respond_with(ResponseTemplate::new(200).set_body_json(workspace_response(
                "ws-abc123",
                "my-workspace",
                "my-org",
            )))
            .mount(&mock_server)
            .await;

        let result = resolve_workspace(&client, "my-workspace", Some("my-org"), true).await;

        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert_eq!(resolved.org, "my-org");
    }
}
