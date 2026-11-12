//! Workspace resolution utilities
//!
//! Provides shared functionality for resolving workspaces by ID or name,
//! with optional auto-discovery across organizations.

use futures::stream::{self, StreamExt};
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

/// Reusable discovery controls; legacy callers retain their existing defaults.
#[derive(Clone, Copy, Default)]
pub(crate) struct WorkspaceResolutionOptions {
    pub all_organization_pages: bool,
    pub concurrency: Option<usize>,
    pub propagate_errors: bool,
    pub id_name_fallback: bool,
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
    resolve_workspace_with_options(
        client,
        target,
        org,
        batch,
        WorkspaceResolutionOptions::default(),
    )
    .await
}

pub(crate) async fn resolve_workspace_with_options(
    client: &TfeClient,
    target: &str,
    org: Option<&str>,
    batch: bool,
    options: WorkspaceResolutionOptions,
) -> Result<ResolvedWorkspace, Box<dyn std::error::Error>> {
    if target.starts_with("ws-") && options.id_name_fallback {
        // Prefer a real workspace ID, but a 404 is not proof this is not a name.
        // Authentication/server errors are never converted into a name fallback.
        if let Some((workspace, raw)) = client.get_workspace_by_id(target).await? {
            let org = workspace
                .organization_name()
                .unwrap_or("unknown")
                .to_string();
            return Ok(ResolvedWorkspace {
                workspace,
                raw,
                org,
            });
        }
        return if let Some(org) = org {
            resolve_by_name(client, org, target, batch).await
        } else {
            resolve_across_orgs(client, target, batch, options).await
        };
    }
    match parse_workspace_target(target) {
        WorkspaceTarget::Id(ws_id) => resolve_by_id(client, &ws_id, batch).await,
        WorkspaceTarget::Name(name) => {
            if let Some(org_name) = org {
                resolve_by_name(client, org_name, &name, batch).await
            } else {
                resolve_across_orgs(client, &name, batch, options).await
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
    options: WorkspaceResolutionOptions,
) -> Result<ResolvedWorkspace, Box<dyn std::error::Error>> {
    let organizations = if options.all_organization_pages {
        client.get_all_organizations().await?
    } else {
        resolve_organizations(client, None).await?
    };

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
    let jobs = organizations.iter().map(|org_name| {
        let org = org_name.clone();
        let ws_name = name_owned.clone();
        async move {
            let result = client.get_workspace_by_name(&org, &ws_name).await;
            (org, result)
        }
    });
    let concurrency = options.concurrency.unwrap_or(organizations.len()).max(1);
    let mut futures = stream::iter(jobs).buffer_unordered(concurrency);
    let mut failure = None;

    // Process results as they complete, stop on first match
    while let Some((org_name, result)) = futures.next().await {
        match result {
            Ok(Some((ws, raw))) => {
                finish_spinner(spinner);
                return Ok(ResolvedWorkspace {
                    workspace: ws,
                    raw,
                    org: org_name,
                });
            }
            Err(error) if options.propagate_errors => failure = Some(error),
            _ => {}
        }
    }

    finish_spinner(spinner);

    if let Some(error) = failure {
        return Err(error.into());
    }

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
    use crate::hcp::RetryPolicy;

    #[tokio::test]
    async fn discovery_bulk_propagates_api_errors_while_single_retains_not_found() {
        for bulk in [false, true] {
            for status in [401, 500, 404] {
                let server = MockServer::start().await;
                Mock::given(method("GET"))
                    .and(path("/organizations"))
                    .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                        "data":[{"id":"org","attributes":{"name":"org"}}],
                        "meta":{"pagination":{"current-page":1,"total-pages":1,"total-count":1}}
                    })))
                    .expect(1)
                    .mount(&server)
                    .await;
                Mock::given(method("GET"))
                    .and(path("/organizations/org/workspaces/missing"))
                    .respond_with(ResponseTemplate::new(status))
                    .expect(1)
                    .mount(&server)
                    .await;
                let client = TfeClient::test_client(&server.uri());
                let client = if bulk {
                    client.with_request_policy(RetryPolicy::instant())
                } else {
                    client
                };
                let message = resolve_workspace_with_options(
                    &client,
                    "missing",
                    None,
                    true,
                    WorkspaceResolutionOptions {
                        all_organization_pages: bulk,
                        propagate_errors: bulk,
                        concurrency: bulk.then_some(4),
                        ..Default::default()
                    },
                )
                .await
                .unwrap_err()
                .to_string();
                if bulk && status != 404 {
                    assert!(message.contains(&status.to_string()));
                } else {
                    assert!(message.contains("not found in organization 'org'"));
                }
            }
        }
    }

    #[tokio::test]
    async fn bulk_discovery_finds_workspace_on_second_organization_page() {
        let server = MockServer::start().await;
        for page in 1..=2 {
            Mock::given(method("GET")).and(path("/organizations"))
                .and(wiremock::matchers::query_param("page[number]", page.to_string()))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data":[{"id":format!("org-{page}"),"attributes":{"name":format!("org-{page}")}}],
                    "meta":{"pagination":{"current-page":page,"total-pages":2,"total-count":2}}
                }))).expect(1).mount(&server).await;
        }
        Mock::given(method("GET"))
            .and(path("/organizations/org-1/workspaces/workspace"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/organizations/org-2/workspaces/workspace"))
            .respond_with(ResponseTemplate::new(200).set_body_json(workspace_response(
                "ws-abc",
                "workspace",
                "org-2",
            )))
            .expect(1)
            .mount(&server)
            .await;
        let client =
            TfeClient::test_client(&server.uri()).with_request_policy(RetryPolicy::instant());
        let resolved = resolve_workspace_with_options(
            &client,
            "workspace",
            None,
            true,
            WorkspaceResolutionOptions {
                all_organization_pages: true,
                concurrency: Some(4),
                propagate_errors: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(resolved.workspace.id, "ws-abc");
        assert_eq!(resolved.org, "org-2");
    }

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
