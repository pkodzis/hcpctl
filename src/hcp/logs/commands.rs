//! Logs command handler
//!
//! Provides `hcpctl logs` command for viewing run logs (plan or apply).
//! Supports multiple target types:
//! - Run ID (run-xxx): directly fetches logs
//! - Workspace ID (ws-xxx): fetches current-run logs
//! - Workspace name: context/organization resolution with auto-discovery
//! - With --runs: atomically download newest workspace run logs

use log::debug;
use std::io;

use super::bulk::run_bulk_download;
use crate::cli::LogsArgs;
use crate::hcp::runs::{fetch_and_print_log, tail_log};
use crate::hcp::workspaces::{extract_current_run_id, resolve_workspace};
use crate::hcp::{RetryPolicy, TfeClient};
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

/// Register handlers eagerly, before the download can create a probe file.
fn interruption() -> io::Result<impl std::future::Future<Output = io::Result<()>>> {
    #[cfg(unix)]
    {
        let mut interrupt =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        Ok(async move {
            tokio::select! {
                _ = interrupt.recv() => Ok(()),
                _ = terminate.recv() => Ok(()),
            }
        })
    }
    #[cfg(not(unix))]
    {
        let mut signal = Box::pin(tokio::signal::ctrl_c());
        // ctrl_c installs its Windows handler on its first poll.
        let waker = futures::task::noop_waker();
        let mut context = std::task::Context::from_waker(&waker);
        let initial = std::future::Future::poll(signal.as_mut(), &mut context);
        Ok(async move {
            match initial {
                std::task::Poll::Ready(result) => result,
                std::task::Poll::Pending => signal.await,
            }
        })
    }
}

fn process_reporter(stderr: bool) -> io::Result<Box<dyn tokio::io::AsyncWrite + Unpin>> {
    #[cfg(unix)]
    match crate::ui::report::ProcessReporter::new(stderr) {
        Ok(reporter) => return Ok(Box::new(reporter)),
        // epoll rejects regular files. Pipe/terminal registration failures
        // must not fall back to an unabortable blocking-pool pipe write.
        Err(error) if error.raw_os_error() == Some(rustix::io::Errno::PERM.raw_os_error()) => {}
        Err(error) => return Err(error),
    }
    if stderr {
        Ok(Box::new(tokio::io::stderr()))
    } else {
        Ok(Box::new(tokio::io::stdout()))
    }
}

/// Run the logs command
pub async fn run_logs_command(
    client: &TfeClient,
    cli: &Cli,
    args: &LogsArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    if args.runs.is_some() {
        let signal = interruption()?;
        let policy = RetryPolicy::bulk();
        let mut out = process_reporter(false)?;
        let mut err = process_reporter(true)?;
        let download = run_bulk_download(client, cli, args, policy.clone(), &mut *out, &mut *err);
        tokio::pin!(download);
        // Do not drop the download future: blocking filesystem writes must be
        // joined before the runtime exits so NamedTempFile cleanup can run.
        return tokio::select! {
            result = &mut download => result,
            signal = signal => {
                policy.cancel();
                let _ = download.await;
                signal?;
                Err(crate::error::TfeError::UserCancelled.into())
            }
        };
    }
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
    use crate::Command;
    use clap::Parser;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[cfg(unix)]
    #[tokio::test]
    async fn signal_registered_before_probe_and_before_listener_first_poll() {
        // Signals are process-wide: isolate this regression so concurrently
        // running handler tests cannot consume our deliberately sent signal.
        const CHILD: &str = "HCPCTL_SIGNAL_PROBE_REGRESSION_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "hcp::logs::commands::tests::signal_registered_before_probe_and_before_listener_first_poll", "--nocapture"])
                .env(CHILD, "1")
                .output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        for signal in ["-INT", "-TERM"] {
            let listener = interruption().unwrap();
            let dir = tempfile::tempdir().unwrap();
            let probe = tempfile::NamedTempFile::new_in(dir.path()).unwrap();
            assert!(std::process::Command::new("kill")
                .args([signal, &std::process::id().to_string()])
                .status()
                .unwrap()
                .success());
            tokio::time::timeout(std::time::Duration::from_secs(2), listener)
                .await
                .unwrap()
                .unwrap();
            drop(probe);
            assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        }
    }

    #[tokio::test]
    async fn single_logs_handler_uses_current_run_and_preserves_phase_raw_follow() {
        for target in ["ws-abc", "workspace", "run-current"] {
            for flags in [
                vec![],
                vec!["--apply"],
                vec!["--raw"],
                vec!["--follow"],
                vec!["--apply", "--raw", "--follow"],
            ] {
                let server = MockServer::start().await;
                if target != "run-current" {
                    let endpoint = if target == "ws-abc" {
                        "/workspaces/ws-abc"
                    } else {
                        "/organizations/org/workspaces/workspace"
                    };
                    Mock::given(method("GET"))
                        .and(path(endpoint))
                        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data":{
                            "id":"ws-abc","attributes":{"name":"workspace"},
                            "relationships":{"current-run":{"data":{"id":"run-current"}}}
                        }})))
                        .expect(1)
                        .mount(&server)
                        .await;
                }
                let phase = if flags.contains(&"--apply") {
                    "apply"
                } else {
                    "plan"
                };
                Mock::given(method("GET")).and(path(format!("/runs/run-current/{phase}")))
                    .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data":{
                        "id":format!("{phase}-abc"),"attributes":{"status":"finished","log-read-url":format!("{}/blob",server.uri())}
                    }}))).expect(1).mount(&server).await;
                Mock::given(method("GET"))
                    .and(path("/blob"))
                    .respond_with(|request: &wiremock::Request| {
                        assert!(!request.headers.contains_key("authorization"));
                        ResponseTemplate::new(200).set_body_string("{\"@message\":\"hello\"}\n")
                    })
                    .expect(1)
                    .mount(&server)
                    .await;
                let mut words = vec!["hcpctl", "--batch", "logs", target];
                words.extend(flags);
                let cli = Cli::parse_from(words);
                let Command::Logs(args) = &cli.command else {
                    panic!()
                };
                let mut client = TfeClient::test_client(&server.uri());
                client.set_context_org(Some("org".into()));
                run_logs_command(&client, &cli, args).await.unwrap();
                assert_eq!(
                    server.received_requests().await.unwrap().len(),
                    if target == "run-current" { 2 } else { 3 }
                );
            }
        }
    }

    #[tokio::test]
    async fn single_logs_workspace_without_current_run_does_not_fetch_history() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/workspaces/ws-abc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data":{
                "id":"ws-abc","attributes":{"name":"workspace"},
                "relationships":{"current-run":{"data":null}}
            }})))
            .expect(1)
            .mount(&server)
            .await;
        let cli = Cli::parse_from(["hcpctl", "--batch", "logs", "ws-abc"]);
        let Command::Logs(args) = &cli.command else {
            panic!()
        };
        let error = run_logs_command(&TfeClient::test_client(&server.uri()), &cli, args)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("no current run"));
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn logs_handler_dispatches_bulk_instead_of_current_run() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/workspaces/ws-abc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data":{
                "id":"ws-abc","attributes":{"name":"workspace"},
                "relationships":{"current-run":{"data":{"id":"run-current"}}}
            }})))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/workspaces/ws-abc/runs"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data":[],"meta":{"pagination":{"current-page":1,"total-pages":1,"total-count":0}}
            })))
            .expect(1)
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let cli = Cli::parse_from([
            "hcpctl",
            "--batch",
            "logs",
            "ws-abc",
            "--runs",
            "1",
            "--output-dir",
            dir.path().to_str().unwrap(),
        ]);
        let Command::Logs(args) = &cli.command else {
            panic!()
        };
        let error = run_logs_command(&TfeClient::test_client(&server.uri()), &cli, args)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("has no runs"));
        assert_eq!(server.received_requests().await.unwrap().len(), 2);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
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
