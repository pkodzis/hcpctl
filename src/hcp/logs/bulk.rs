//! Bulk workspace log download with bounded concurrency and atomic replacement.

use futures::stream::{self, StreamExt};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use tempfile::NamedTempFile;
use tokio::io::{AsyncWrite, AsyncWriteExt};

use crate::cli::LogsArgs;
use crate::hcp::runs::{format_human_readable_log, Run};
use crate::hcp::traits::TfeResource;
use crate::hcp::workspaces::resolver::{
    resolve_workspace_with_options, WorkspaceResolutionOptions,
};
use crate::hcp::{RetryPolicy, TfeClient};
use crate::Cli;

pub(crate) fn validate_output_dir(dir: &Path) -> io::Result<PathBuf> {
    if !dir.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Output directory must exist and be a directory",
        ));
    }
    let dir = dir.canonicalize()?;
    // Probe before any HTTP request. Dropping removes the probe immediately.
    NamedTempFile::new_in(&dir)?;
    Ok(dir)
}

fn validate_workspace_name(workspace: &str) -> io::Result<()> {
    let valid_name = !workspace.is_empty()
        && workspace != "."
        && workspace != ".."
        && !workspace
            .chars()
            .any(|c| c.is_control() || c == '/' || c == '\\');
    if !valid_name {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Unsafe workspace name",
        ));
    }
    Ok(())
}

fn filename(workspace: &str, number: usize, run: &str, apply: bool) -> io::Result<String> {
    validate_workspace_name(workspace)?;
    let valid_run = run
        .strip_prefix("run-")
        .is_some_and(|id| !id.is_empty() && id.bytes().all(|c| c.is_ascii_alphanumeric()));
    let name = format!(
        "{}-{:02}-{}-{}.txt",
        workspace,
        number,
        run,
        if apply { "apply" } else { "plan" }
    );
    let mut components = Path::new(&name).components();
    if !valid_run
        || name.len() > 255
        || !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Unsafe or overlong log filename",
        ));
    }
    Ok(name)
}

fn write_atomic_with(
    dir: &Path,
    name: &str,
    write_step: impl FnOnce(&mut NamedTempFile) -> io::Result<()>,
) -> io::Result<PathBuf> {
    let mut temp = NamedTempFile::new_in(dir)?;
    write_step(&mut temp)?;
    temp.flush()?;
    temp.as_file().sync_all()?;
    let path = dir.join(name);
    // persist replaces atomically; a failure retains the temp in PersistError,
    // which is dropped here and removes it without touching the final file.
    temp.persist(&path)
        .map_err(|err| io::Error::new(err.error.kind(), err.error.to_string()))?;
    Ok(path)
}

struct Outcome {
    run: String,
    apply: bool,
    result: Result<PathBuf, String>,
}

async fn report(
    writer: &mut (dyn AsyncWrite + Unpin),
    policy: &RetryPolicy,
    message: String,
) -> io::Result<()> {
    tokio::select! {
        biased;
        _ = policy.cancelled() => Err(io::Error::new(io::ErrorKind::Interrupted, "Operation cancelled by user")),
        result = writer.write_all(message.as_bytes()) => result,
    }
}

pub(crate) async fn run_bulk_download(
    client: &TfeClient,
    cli: &Cli,
    args: &LogsArgs,
    policy: RetryPolicy,
    out: &mut (dyn AsyncWrite + Unpin),
    err: &mut (dyn AsyncWrite + Unpin),
) -> Result<(), Box<dyn std::error::Error>> {
    let result = run_bulk_download_inner(client, cli, args, policy.clone(), out, err).await;
    // Include preflight/fallback diagnostics even when history or resolution
    // fails before workers exist. Report failure never unwinds active writes.
    for diagnostic in policy.take_diagnostics() {
        report(err, &policy, format!("{}\n", diagnostic)).await?;
    }
    result
}

async fn run_bulk_download_inner(
    client: &TfeClient,
    cli: &Cli,
    args: &LogsArgs,
    policy: RetryPolicy,
    out: &mut (dyn AsyncWrite + Unpin),
    err: &mut (dyn AsyncWrite + Unpin),
) -> Result<(), Box<dyn std::error::Error>> {
    let count = args.runs.ok_or("--runs is required")?;
    if count == 0 || args.follow || (args.all_phases && args.apply) {
        return Err("Invalid bulk log flag combination".into());
    }
    let dir = validate_output_dir(
        args.output_dir
            .as_deref()
            .ok_or("--output-dir is required")?,
    )?;
    let client = client.with_request_policy(policy.clone());
    let ambiguous = args.target.starts_with("run-");
    let org = client.effective_org(args.org.as_ref());
    // Scoped run-like names win a collision; unscoped targets retain run-first
    // classification. A genuine run ID is still rejected when no name matches.
    let scoped_workspace = if ambiguous {
        if let Some(org) = org.as_deref() {
            client.get_workspace_by_name(org, &args.target).await?
        } else {
            None
        }
    } else {
        None
    };
    if ambiguous
        && scoped_workspace.is_none()
        && client.get_run_by_id(&args.target).await?.is_some()
    {
        return Err(format!(
            "'{}' is a run ID; --runs needs a workspace name or ID",
            args.target
        )
        .into());
    }
    let resolved = if let Some((workspace, raw)) = scoped_workspace {
        crate::hcp::workspaces::ResolvedWorkspace {
            workspace,
            raw,
            org: org.clone().unwrap_or_default(),
        }
    } else {
        resolve_workspace_with_options(&client, &args.target, org.as_deref(), cli.batch, WorkspaceResolutionOptions {
            all_organization_pages: true,
            concurrency: Some(4),
            propagate_errors: true,
            id_name_fallback: true,
        }).await
        .map_err(|error| if ambiguous && error.to_string().contains("not found") {
            format!("'{}' was not found as a workspace. If it is a run ID, it may not exist or you may lack access; --runs requires a workspace name or ID.", args.target)
        } else { error.to_string() })?
    };
    let name = resolved.workspace.name();
    validate_workspace_name(name)?;
    let requested_phases: &[bool] = if args.all_phases {
        &[false, true]
    } else if args.apply {
        &[true]
    } else {
        &[false]
    };
    let phases = if args.all_phases {
        "plan,apply"
    } else if args.apply {
        "apply"
    } else {
        "plan"
    };
    let (runs, included, acquired) = client
        .get_bulk_run_history(resolved.workspace.id(), count, phases, &policy)
        .await?;
    if runs.is_empty() {
        return Err(format!("Workspace '{}' has no runs", name).into());
    }
    // Validate every final name before starting any download.
    for (index, run) in runs.iter().enumerate() {
        for &apply in requested_phases {
            filename(name, index + 1, &run.id, apply)?;
        }
    }
    let total = runs.len() * if args.all_phases { 2 } else { 1 };
    let job = |(index, run): (usize, Run)| {
        let client = &client;
        let policy = &policy;
        let included = &included;
        let dir = &dir;
        async move {
            let mut outcomes = Vec::new();
            for &apply in requested_phases {
                let result = async {
                    policy.check_cancelled().map_err(|e| e.to_string())?;
                    let content = tokio::select! {
                        _ = policy.cancelled() => return Err("Operation cancelled by user".to_string()),
                        content = client.fetch_bulk_phase(&run, apply, included, acquired, policy) => content.map_err(|e| e.to_string())?,
                    };
                    let bytes = if args.raw {
                        content
                    } else {
                        format_human_readable_log(&content)
                    };
                    let file =
                        filename(name, index + 1, &run.id, apply).map_err(|e| e.to_string())?;
                    let dir = dir.clone();
                    let policy = policy.clone();
                    tokio::task::spawn_blocking(move || {
                        policy.check_cancelled().map_err(|e| io::Error::other(e.to_string()))?;
                        write_atomic_with(&dir, &file, |temp| {
                            temp.write_all(bytes.as_bytes())?;
                            policy.check_cancelled().map_err(|e| io::Error::other(e.to_string()))
                        })
                    })
                    .await
                    .map_err(|e| e.to_string())?
                    .map_err(|e| e.to_string())
                }
                .await;
                outcomes.push(Outcome {
                    run: run.id.clone(),
                    apply,
                    result,
                });
            }
            outcomes
        }
    };
    let mut results = stream::iter(runs.into_iter().enumerate())
        .map(job)
        .buffered(4);
    let mut failures = 0;
    let mut report_error = None;
    while let Some(outcomes) = results.next().await {
        for diagnostic in policy.take_diagnostics() {
            if let Err(error) = report(err, &policy, format!("{}\n", diagnostic)).await {
                report_error = Some(error);
            }
        }
        for outcome in outcomes {
            let report = match outcome.result {
                Ok(path) => report(out, &policy, format!("Saved {}\n", path.display())).await,
                Err(reason) => {
                    failures += 1;
                    report(
                        err,
                        &policy,
                        format!(
                            "Error: run {} {}: {}\n",
                            outcome.run,
                            if outcome.apply { "apply" } else { "plan" },
                            reason
                        ),
                    )
                    .await
                }
            };
            // Drain all workers even if reporting fails; no abandoned writes.
            if let Err(error) = report {
                report_error = Some(error);
            }
        }
    }
    policy.check_cancelled()?;
    if let Some(error) = report_error {
        return Err(error.into());
    }
    if failures > 0 {
        return Err(format!("{} of {} log(s) could not be saved", failures, total).into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Command;
    use clap::Parser;
    use serde_json::{json, Value};
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn verification_ws_prefixed_workspace_name_downloads_by_name() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/organizations/org/workspaces/ws-production"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data":{
                "id":"ws-abc", "attributes":{"name":"ws-production"}
            }})))
            .mount(&server)
            .await;
        history(&server, 1, true, "plan").await;
        Mock::given(method("GET"))
            .and(path("/blob/0/plan"))
            .respond_with(ResponseTemplate::new(200).set_body_string("log"))
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let cli = arguments("ws-production", dir.path(), &["--org", "org"]);
        let (result, out, err) = execute(&TfeClient::test_client(&server.uri()), &cli).await;
        result.unwrap();
        assert!(err.is_empty());
        assert!(out.contains("ws-production-01-run-R0-plan.txt"));
        assert_eq!(
            fs::read_to_string(dir.path().join("ws-production-01-run-R0-plan.txt")).unwrap(),
            "log\n"
        );
    }

    #[tokio::test]
    async fn verification_explicit_org_workspace_name_wins_run_id_collision() {
        run_id_collision_with_scope(false).await;
    }

    #[tokio::test]
    async fn context_org_workspace_name_wins_run_id_collision() {
        run_id_collision_with_scope(true).await;
    }

    async fn run_id_collision_with_scope(context: bool) {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/runs/run-abc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data":{
                "id":"run-abc", "attributes":{"status":"applied"}
            }})))
            .expect(0)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/organizations/org/workspaces/run-abc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data":{
                "id":"ws-abc", "attributes":{"name":"run-abc"}
            }})))
            .expect(1)
            .mount(&server)
            .await;
        history(&server, 1, true, "plan").await;
        Mock::given(method("GET"))
            .and(path("/blob/0/plan"))
            .respond_with(ResponseTemplate::new(200).set_body_string("log"))
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let cli = arguments(
            "run-abc",
            dir.path(),
            if context { &[] } else { &["--org", "org"] },
        );
        let mut client = TfeClient::test_client(&server.uri());
        if context {
            client.set_context_org(Some("org".into()));
        }
        let (result, out, err) = execute(&client, &cli).await;
        result.unwrap();
        assert!(err.is_empty());
        assert!(out.contains("run-abc-01-run-R0-plan.txt"));
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn unscoped_run_id_collision_is_rejected_before_workspace_discovery() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/runs/run-abc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data":{
                "id":"run-abc", "attributes":{"status":"applied"}
            }})))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/organizations"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data":[{"id":"org","attributes":{"name":"org"}}]
            })))
            .expect(0)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/organizations/org/workspaces/run-abc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data":{
                "id":"ws-abc", "attributes":{"name":"run-abc"}
            }})))
            .expect(0)
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let cli = arguments("run-abc", dir.path(), &[]);
        let error = execute(&TfeClient::test_client(&server.uri()), &cli)
            .await
            .0
            .unwrap_err();
        assert!(error.contains("is a run ID"));
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    fn arguments(target: &str, dir: &Path, flags: &[&str]) -> Cli {
        let mut words = vec![
            "hcpctl",
            "--batch",
            "logs",
            target,
            "--runs",
            "105",
            "--output-dir",
            dir.to_str().unwrap(),
        ];
        words.extend(flags);
        Cli::parse_from(words)
    }

    fn run(id: usize) -> Value {
        json!({"id": format!("run-R{id}"), "attributes": {
            "status": (["applied", "errored", "canceled", "discarded", "planned"][id % 5]),
            "created-at": format!("2026-01-01T00:{:02}:{:02}Z", id / 60, id % 60)
        }, "relationships": {
            "plan": {"data": {"id": format!("plan-{id}")}},
            "apply": {"data": {"id": format!("apply-{id}")}}
        }})
    }

    async fn workspace(server: &MockServer, target: &str, auto: bool) {
        if auto {
            Mock::given(method("GET"))
                .and(path("/organizations"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(
                        json!({"data":[{"id":"org", "attributes": {"name":"org"}}]}),
                    ),
                )
                .expect(1)
                .mount(server)
                .await;
        }
        let endpoint = if target.starts_with("ws-") {
            format!("/workspaces/{target}")
        } else {
            format!("/organizations/org/workspaces/{target}")
        };
        Mock::given(method("GET")).and(path(endpoint))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": {
                "id":"ws-abc", "attributes":{"name":"actual-name"},
                "relationships":{"organization":{"data":{"id":"org"}},"current-run":{"data":{"id":"run-old"}}}
            }}))).expect(1).mount(server).await;
    }

    async fn history(server: &MockServer, size: usize, included: bool, phases: &str) {
        let pages = size.div_ceil(100).max(1);
        for page in 1..=pages {
            // Reverse chronology and place the newest run on the last page.
            let ids: Vec<usize> = ((page - 1) * 100..(page * 100).min(size)).rev().collect();
            let related: Vec<Value> = if included {
                ids.iter()
                    .flat_map(|id| {
                        [false, true].into_iter().map(move |apply| {
                    let phase = if apply {"apply"} else {"plan"};
                    json!({"type":if apply {"applies"} else {"plans"}, "id":format!("{phase}-{id}"),
                        "attributes":{"log-read-url":format!("{}/blob/{id}/{phase}",server.uri())}})
                })
                    })
                    .collect()
            } else {
                vec![]
            };
            Mock::given(method("GET")).and(path("/workspaces/ws-abc/runs"))
                .and(query_param("page[number]",page.to_string()))
                .and(query_param("filter[operation]", "plan_and_apply,plan_only,save_plan,refresh_only,destroy,empty_apply,action_only"))
                .and(query_param("include",phases))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "data": ids.into_iter().map(run).collect::<Vec<_>>(), "included":related,
                    "meta":{"pagination":{"current-page":page,"total-pages":pages,"total-count":size}}
                }))).expect(1).mount(server).await;
        }
    }

    async fn execute(client: &TfeClient, cli: &Cli) -> (Result<(), String>, String, String) {
        let Command::Logs(args) = &cli.command else {
            panic!()
        };
        let mut out = Vec::new();
        let mut err = Vec::new();
        let result = run_bulk_download(
            client,
            cli,
            args,
            RetryPolicy::instant(),
            &mut out,
            &mut err,
        )
        .await
        .map_err(|e| e.to_string());
        (
            result,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    #[tokio::test]
    async fn bulk_105_runs_both_phases_pagination_numbering_and_order() {
        let server = MockServer::start().await;
        workspace(&server, "ws-abc", false).await;
        history(&server, 105, true, "plan,apply").await;
        Mock::given(method("GET"))
            .and(path("/runs/run-R0/plan"))
            .respond_with(ResponseTemplate::new(500))
            .expect(0)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .respond_with(|request: &wiremock::Request| {
                assert!(request.url.path().starts_with("/blob/"));
                assert!(!request.headers.contains_key("authorization"));
                ResponseTemplate::new(200)
                    .set_body_string("{\"@message\":\"hello\"}\n{\"skip\":true}\nplain\n")
            })
            .expect(210)
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let cli = arguments("ws-abc", dir.path(), &["--all-phases"]);
        let (result, out, err) = execute(&TfeClient::test_client(&server.uri()), &cli).await;
        result.unwrap();
        assert!(err.is_empty());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 210);
        for id in 0..105 {
            for apply in [false, true] {
                let file = filename("actual-name", id + 1, &format!("run-R{id}"), apply).unwrap();
                assert_eq!(
                    fs::read_to_string(dir.path().join(file)).unwrap(),
                    "hello\nplain\n"
                );
            }
        }
        assert!(out
            .lines()
            .next()
            .unwrap()
            .contains("actual-name-01-run-R0-plan.txt"));
        assert!(out
            .lines()
            .last()
            .unwrap()
            .contains("actual-name-105-run-R104-apply.txt"));
        #[cfg(unix)]
        {
            assert_eq!(
                fs::metadata(dir.path().join("actual-name-100-run-R99-apply.txt"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }

    #[tokio::test]
    async fn names_context_auto_discovery_plan_apply_raw_and_newest_not_current() {
        for (target, flags, auto) in [
            ("workspace", vec!["--org", "org"], false),
            ("workspace", vec!["--apply", "--raw"], false),
            ("workspace", vec![], true),
        ] {
            let server = MockServer::start().await;
            workspace(&server, target, auto).await;
            let apply = flags.contains(&"--apply");
            let phase = if apply { "apply" } else { "plan" };
            history(&server, 3, true, phase).await;
            Mock::given(method("GET"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_string("{\"@message\":\"hello\"}\n"),
                )
                .expect(1)
                .mount(&server)
                .await;
            let dir = tempfile::tempdir().unwrap();
            let mut cli = arguments(target, dir.path(), &flags);
            let Command::Logs(args) = &mut cli.command else {
                panic!()
            };
            args.runs = Some(1);
            let mut client = TfeClient::test_client(&server.uri());
            if !auto {
                client.set_context_org(Some("org".into()));
            }
            let (result, _, _) = execute(&client, &cli).await;
            result.unwrap();
            let text = fs::read_to_string(
                dir.path()
                    .join(filename("actual-name", 1, "run-R2", apply).unwrap()),
            )
            .unwrap();
            assert_eq!(
                text,
                if flags.contains(&"--raw") {
                    "{\"@message\":\"hello\"}\n"
                } else {
                    "hello\n"
                }
            );
        }
    }

    #[tokio::test]
    async fn missing_and_failed_logs_continue_and_preserve_existing_files() {
        for status in [200, 404, 500] {
            let server = MockServer::start().await;
            workspace(&server, "ws-abc", false).await;
            history(&server, 2, true, "plan,apply").await;
            Mock::given(method("GET"))
                .and(path("/blob/0/plan"))
                .respond_with(ResponseTemplate::new(status))
                .expect(1)
                .mount(&server)
                .await;
            Mock::given(method("GET"))
                .respond_with(ResponseTemplate::new(200).set_body_string("good"))
                .expect(3)
                .mount(&server)
                .await;
            let dir = tempfile::tempdir().unwrap();
            let existing = dir.path().join("actual-name-01-run-R0-plan.txt");
            fs::write(&existing, "original").unwrap();
            // A directory at one destination forces persist failure, not a partial file.
            let blocked = dir.path().join("actual-name-02-run-R1-apply.txt");
            fs::create_dir(&blocked).unwrap();
            let cli = arguments("ws-abc", dir.path(), &["--all-phases"]);
            let (result, out, err) = execute(&TfeClient::test_client(&server.uri()), &cli).await;
            assert!(result.unwrap_err().contains("2 of 4"));
            assert!(err.contains("run run-R0 plan"));
            assert!(err.contains("run run-R1 apply"));
            assert_eq!(out.lines().count(), 2);
            assert_eq!(fs::read_to_string(existing).unwrap(), "original");
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 4);
        }
    }

    #[tokio::test]
    async fn empty_history_is_explicit_error_without_downloads() {
        let server = MockServer::start().await;
        workspace(&server, "ws-abc", false).await;
        history(&server, 0, true, "plan").await;
        let dir = tempfile::tempdir().unwrap();
        let cli = arguments("ws-abc", dir.path(), &[]);
        let (result, out, err) = execute(&TfeClient::test_client(&server.uri()), &cli).await;
        assert!(result.unwrap_err().contains("has no runs"));
        assert!(out.is_empty() && err.is_empty());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn run_like_targets_classification_and_ambiguous_workspace_name() {
        for status in [200, 404, 403, 500] {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/runs/run-abc"))
                .respond_with(ResponseTemplate::new(status).set_body_json(json!({"data":run(0)})))
                .expect(1)
                .mount(&server)
                .await;
            Mock::given(method("GET"))
                .and(path("/organizations/org/workspaces/run-abc"))
                .respond_with(ResponseTemplate::new(404))
                .mount(&server)
                .await;
            let dir = tempfile::tempdir().unwrap();
            let cli = arguments("run-abc", dir.path(), &["--org", "org"]);
            let message = execute(&TfeClient::test_client(&server.uri()), &cli)
                .await
                .0
                .unwrap_err();
            if status == 200 {
                assert!(message.contains("is a run ID"));
            } else if status == 404 {
                assert!(message.contains("was not found as a workspace"));
            } else {
                assert!(message.contains(&status.to_string()));
            }
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
        }
        let server = MockServer::start().await;
        let dir = tempfile::tempdir().unwrap();
        let cli = arguments("run-absent", dir.path(), &["--org", "org"]);
        let message = execute(&TfeClient::test_client(&server.uri()), &cli)
            .await
            .0
            .unwrap_err();
        assert!(message.contains("may lack access"));
    }

    #[tokio::test]
    async fn concurrent_downloads_overlap_are_bounded_and_reports_stay_ordered() {
        let server = MockServer::start().await;
        workspace(&server, "ws-abc", false).await;
        history(&server, 20, true, "plan").await;
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let active_response = active.clone();
        let peak_response = peak.clone();
        Mock::given(method("GET"))
            .respond_with(move |_: &wiremock::Request| {
                let count = active_response.fetch_add(1, Ordering::SeqCst) + 1;
                peak_response.fetch_max(count, Ordering::SeqCst);
                let active = active_response.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(80)).await;
                    active.fetch_sub(1, Ordering::SeqCst);
                });
                ResponseTemplate::new(200)
                    .set_body_string("log")
                    .set_delay(Duration::from_millis(100))
            })
            .expect(20)
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let cli = arguments("ws-abc", dir.path(), &[]);
        let (result, out, err) = execute(&TfeClient::test_client(&server.uri()), &cli).await;
        result.unwrap();
        assert!(err.is_empty());
        assert!((2..=4).contains(&peak.load(Ordering::SeqCst)));
        for (id, line) in out.lines().enumerate() {
            assert!(line.ends_with(
                &filename("actual-name", id + 1, &format!("run-R{id}"), false).unwrap()
            ));
        }
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 20);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn read_only_directory_has_no_http_requests() {
        let server = MockServer::start().await;
        let dir = tempfile::tempdir().unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o500)).unwrap();
        // Root can write regardless of mode bits; avoid a misleading assertion.
        if NamedTempFile::new_in(dir.path()).is_err() {
            let cli = arguments("run-abc", dir.path(), &[]);
            assert!(execute(&TfeClient::test_client(&server.uri()), &cli)
                .await
                .0
                .is_err());
            assert!(server.received_requests().await.unwrap().is_empty());
        }
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    }

    #[tokio::test]
    async fn invalid_directory_precedes_even_run_classification() {
        let server = MockServer::start().await;
        let dir = tempfile::tempdir().unwrap();
        for path in [dir.path().join("missing"), dir.path().join("file")] {
            if path.ends_with("file") {
                fs::write(&path, "data").unwrap();
            }
            let cli = arguments("run-abc", &path, &[]);
            assert!(execute(&TfeClient::test_client(&server.uri()), &cli)
                .await
                .0
                .is_err());
        }
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[test]
    fn filenames_and_atomic_failure_cleanup_and_overwrite() {
        for name in [
            "",
            ".",
            "..",
            "a/b",
            "a\\b",
            "a\0b",
            "a\nb",
            &"x".repeat(250),
        ] {
            assert!(filename(name, 1, "run-abc", false).is_err());
        }
        for id in ["run-", "run-a/b", "../run-abc", "run-a.b"] {
            assert!(filename("ws", 1, id, false).is_err());
        }
        for n in [1, 99, 100, 105] {
            assert!(filename("ws", n, "run-abc", false)
                .unwrap()
                .contains(&format!("-{:02}-", n)));
        }
        let dir = tempfile::tempdir().unwrap();
        let final_path = dir.path().join("log.txt");
        fs::write(&final_path, "original").unwrap();
        let result = write_atomic_with(dir.path(), "log.txt", |temp| {
            temp.write_all(b"partial")?;
            Err(io::Error::other("injected write/flush failure"))
        });
        assert!(result.is_err());
        assert_eq!(fs::read_to_string(&final_path).unwrap(), "original");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        write_atomic_with(dir.path(), "log.txt", |temp| temp.write_all(b"replacement")).unwrap();
        assert_eq!(fs::read_to_string(final_path).unwrap(), "replacement");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn bulk_preflight_rejects_unsafe_workspace_and_run_before_blobs() {
        for unsafe_workspace in [true, false] {
            let server = MockServer::start().await;
            Mock::given(method("GET")).and(path("/workspaces/ws-abc"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data":{
                    "id":"ws-abc","attributes":{"name":if unsafe_workspace {"../escape"} else {"actual-name"}}
                }}))).expect(1).mount(&server).await;
            if !unsafe_workspace {
                Mock::given(method("GET")).and(path("/workspaces/ws-abc/runs"))
                    .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                        "data":[run(0),{"id":"run-../escape","attributes":{"status":"errored","created-at":"2026-01-02T00:00:00Z"}}],
                        "meta":{"pagination":{"current-page":1,"total-pages":1,"total-count":2}}
                    }))).expect(1).mount(&server).await;
            }
            let dir = tempfile::tempdir().unwrap();
            let cli = arguments("ws-abc", dir.path(), &[]);
            let (result, out, err) = execute(&TfeClient::test_client(&server.uri()), &cli).await;
            assert!(result.unwrap_err().contains("Unsafe"));
            assert!(out.is_empty() && err.is_empty());
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
            assert_eq!(
                server.received_requests().await.unwrap().len(),
                if unsafe_workspace { 1 } else { 2 }
            );
        }
    }

    #[tokio::test]
    async fn bulk_newest_three_of_twenty_renumber_from_one() {
        let server = MockServer::start().await;
        workspace(&server, "ws-abc", false).await;
        history(&server, 20, true, "plan").await;
        for id in 17..20 {
            Mock::given(method("GET"))
                .and(path(format!("/blob/{id}/plan")))
                .respond_with(ResponseTemplate::new(200).set_body_string(format!("log {id}")))
                .expect(1)
                .mount(&server)
                .await;
        }
        let dir = tempfile::tempdir().unwrap();
        let mut cli = arguments("ws-abc", dir.path(), &[]);
        let Command::Logs(args) = &mut cli.command else {
            panic!()
        };
        args.runs = Some(3);
        let (result, out, err) = execute(&TfeClient::test_client(&server.uri()), &cli).await;
        result.unwrap();
        assert!(err.is_empty());
        let reports: Vec<_> = out.lines().collect();
        assert_eq!(reports.len(), 3);
        for (index, id) in (17..20).enumerate() {
            let file = dir
                .path()
                .join(format!("actual-name-{:02}-run-R{id}-plan.txt", index + 1));
            assert_eq!(reports[index], format!("Saved {}", file.display()));
            assert_eq!(fs::read_to_string(file).unwrap(), format!("log {id}\n"));
        }
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 3);
    }

    #[tokio::test]
    async fn verification_round_one_filename_uses_selected_count_not_requested_count() {
        download_with_fitting_plan_filename(220, usize::MAX, "run-R0").await;
    }

    #[tokio::test]
    async fn verification_round_one_plan_only_filename_does_not_require_apply_to_fit() {
        download_with_fitting_plan_filename(214, 1, &format!("run-{}", "R".repeat(24))).await;
    }

    async fn download_with_fitting_plan_filename(name_length: usize, count: usize, run_id: &str) {
        let server = MockServer::start().await;
        let name = "w".repeat(name_length);
        let expected_file = filename(&name, 1, run_id, false).unwrap();
        assert!(expected_file.len() <= 255);
        // Synthetic API data exercises accepted filename boundaries, not live
        // service identifier limits. Verify the temp filesystem supports this
        // component length before asserting a successful filesystem write.
        let dir = tempfile::tempdir().unwrap();
        let probe = dir.path().join(&expected_file);
        if let Err(error) = fs::write(&probe, "capability probe") {
            eprintln!(
                "filesystem cannot support {}-byte fixture: {error}",
                expected_file.len()
            );
            return;
        }
        fs::remove_file(probe).unwrap();
        Mock::given(method("GET"))
            .and(path("/workspaces/ws-abc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data":{
                "id":"ws-abc", "attributes":{"name":name}
            }})))
            .expect(1)
            .mount(&server)
            .await;
        let mut selected_run = run(0);
        selected_run["id"] = json!(run_id);
        Mock::given(method("GET"))
            .and(path("/workspaces/ws-abc/runs"))
            .and(query_param("include", "plan"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data":[selected_run],
                "included":[{"type":"plans", "id":"plan-0", "attributes":{
                    "log-read-url":format!("{}/blob/0/plan", server.uri())
                }}],
                "meta":{"pagination":{"current-page":1,"total-pages":1,"total-count":1}}
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/blob/0/plan"))
            .respond_with(ResponseTemplate::new(200).set_body_string("complete"))
            .mount(&server)
            .await;
        let mut cli = arguments("ws-abc", dir.path(), &[]);
        let Command::Logs(args) = &mut cli.command else {
            panic!()
        };
        args.runs = Some(count);
        let (result, out, err) = execute(&TfeClient::test_client(&server.uri()), &cli).await;
        assert!(
            result.is_ok(),
            "fitting selected filename rejected: {result:?}"
        );
        assert!(err.is_empty());
        let saved = dir.path().join(expected_file);
        assert_eq!(out.trim(), format!("Saved {}", saved.display()));
        assert_eq!(fs::read_to_string(saved).unwrap(), "complete\n");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn actual_requested_overlong_filenames_rejected_before_any_blob() {
        for flags in [vec!["--apply"], vec!["--all-phases"], vec![]] {
            let server = MockServer::start().await;
            // At 214 bytes plan fits exactly, apply is 256 bytes. At 215
            // bytes even plan is overlong. No filesystem limit assumption.
            let name = "w".repeat(if flags.is_empty() { 215 } else { 214 });
            let id = format!("run-{}", "R".repeat(24));
            Mock::given(method("GET"))
                .and(path("/workspaces/ws-abc"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data":{
                    "id":"ws-abc","attributes":{"name":name}
                }})))
                .expect(1)
                .mount(&server)
                .await;
            let mut selected = run(0);
            selected["id"] = json!(id);
            Mock::given(method("GET")).and(path("/workspaces/ws-abc/runs"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "data":[selected],"meta":{"pagination":{"current-page":1,"total-pages":1,"total-count":1}}
                }))).expect(1).mount(&server).await;
            let dir = tempfile::tempdir().unwrap();
            let cli = arguments("ws-abc", dir.path(), &flags);
            let (result, out, err) = execute(&TfeClient::test_client(&server.uri()), &cli).await;
            assert!(result.unwrap_err().contains("overlong"));
            assert!(out.is_empty() && err.is_empty());
            assert_eq!(server.received_requests().await.unwrap().len(), 2);
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
        }
    }

    #[test]
    fn exact_phase_filename_boundaries_and_utf8_byte_lengths() {
        let id = format!("run-{}", "R".repeat(24));
        assert_eq!(
            filename(&"w".repeat(214), 1, &id, false).unwrap().len(),
            255
        );
        assert!(filename(&"w".repeat(214), 1, &id, true).is_err());
        assert_eq!(filename(&"w".repeat(213), 1, &id, true).unwrap().len(), 255);
        assert!(filename(&"w".repeat(215), 1, &id, false).is_err());
        assert!(filename(&"é".repeat(120), 1, &id, false).is_err());
    }

    struct StalledReporter {
        policy: RetryPolicy,
    }

    impl AsyncWrite for StalledReporter {
        fn poll_write(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
            _: &[u8],
        ) -> std::task::Poll<io::Result<usize>> {
            // Model a full unread pipe. Cancellation arrives while write_all
            // is pending, rather than returning an immediate broken-pipe error.
            self.policy.cancel();
            std::task::Poll::Pending
        }
        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<io::Result<()>> {
            std::task::Poll::Pending
        }
        fn poll_shutdown(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<io::Result<()>> {
            std::task::Poll::Pending
        }
    }

    #[tokio::test]
    async fn stalled_stdout_or_stderr_cancellation_drains_bulk_workers() {
        for stderr in [false, true] {
            let server = MockServer::start().await;
            workspace(&server, "ws-abc", false).await;
            history(&server, 8, true, "plan").await;
            Mock::given(method("GET"))
                .and(path("/blob/0/plan"))
                .respond_with(
                    ResponseTemplate::new(if stderr { 404 } else { 200 }).set_body_string("saved"),
                )
                .expect(1)
                .mount(&server)
                .await;
            Mock::given(method("GET"))
                .respond_with(ResponseTemplate::new(200).set_body_string("saved"))
                .mount(&server)
                .await;
            let dir = tempfile::tempdir().unwrap();
            let original = dir.path().join("actual-name-08-run-R7-plan.txt");
            fs::write(&original, "original").unwrap();
            let cli = arguments("ws-abc", dir.path(), &[]);
            let Command::Logs(args) = &cli.command else {
                panic!()
            };
            let policy = RetryPolicy::instant();
            let mut stalled = StalledReporter {
                policy: policy.clone(),
            };
            let mut good = Vec::new();
            let (out, err): (&mut (dyn AsyncWrite + Unpin), &mut (dyn AsyncWrite + Unpin)) =
                if stderr {
                    (&mut good, &mut stalled)
                } else {
                    (&mut stalled, &mut good)
                };
            let client = TfeClient::test_client(&server.uri());
            let result = tokio::time::timeout(
                Duration::from_secs(2),
                run_bulk_download(&client, &cli, args, policy, out, err),
            )
            .await
            .unwrap();
            assert!(result.unwrap_err().to_string().contains("cancelled"));
            assert_eq!(fs::read_to_string(original).unwrap(), "original");
            for entry in fs::read_dir(dir.path()).unwrap() {
                assert_eq!(entry.unwrap().path().extension().unwrap(), "txt");
            }
            assert!(
                server.received_requests().await.unwrap().len() <= 6,
                "no later run jobs start HTTP after cancellation"
            );
        }
    }

    #[tokio::test]
    async fn bulk_identical_timestamps_select_and_number_by_run_id() {
        let server = MockServer::start().await;
        workspace(&server, "ws-abc", false).await;
        let data: Vec<_> = [2, 0, 1]
            .into_iter()
            .map(|id| {
                let mut value = run(id);
                value["attributes"]["created-at"] = json!("2026-01-01T00:00:00Z");
                value
            })
            .collect();
        Mock::given(method("GET"))
            .and(path("/workspaces/ws-abc/runs"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data":data,"meta":{"pagination":{"current-page":1,"total-pages":1,"total-count":3}}
            })))
            .expect(1)
            .mount(&server)
            .await;
        for id in [1, 2] {
            Mock::given(method("GET")).and(path(format!("/runs/run-R{id}/plan")))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data":{
                    "id":format!("plan-{id}"),"attributes":{"status":"finished","log-read-url":format!("{}/blob/{id}",server.uri())}
                }}))).expect(1).mount(&server).await;
            Mock::given(method("GET"))
                .and(path(format!("/blob/{id}")))
                .respond_with(ResponseTemplate::new(200).set_body_string("log"))
                .expect(1)
                .mount(&server)
                .await;
        }
        let dir = tempfile::tempdir().unwrap();
        let mut cli = arguments("ws-abc", dir.path(), &[]);
        let Command::Logs(args) = &mut cli.command else {
            panic!()
        };
        args.runs = Some(2);
        let (result, out, err) = execute(&TfeClient::test_client(&server.uri()), &cli).await;
        result.unwrap();
        assert!(err.is_empty());
        assert_eq!(out.lines().count(), 2);
        assert!(out
            .lines()
            .next()
            .unwrap()
            .ends_with("actual-name-01-run-R1-plan.txt"));
        assert!(out
            .lines()
            .last()
            .unwrap()
            .ends_with("actual-name-02-run-R2-plan.txt"));
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[tokio::test]
    async fn bulk_missing_plan_metadata_still_saves_apply_and_overwrites() {
        for status in [404, 401, 500] {
            let server = MockServer::start().await;
            workspace(&server, "ws-abc", false).await;
            history(&server, 1, false, "plan,apply").await;
            Mock::given(method("GET"))
                .and(path("/runs/run-R0/plan"))
                .respond_with(ResponseTemplate::new(status))
                .expect(1)
                .mount(&server)
                .await;
            Mock::given(method("GET")).and(path("/runs/run-R0/apply"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data":{
                    "id":"apply-0","attributes":{"status":"finished","log-read-url":format!("{}/blob",server.uri())}
                }}))).expect(1).mount(&server).await;
            Mock::given(method("GET"))
                .and(path("/blob"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_string("{\"@message\":\"replacement\"}\n"),
                )
                .expect(1)
                .mount(&server)
                .await;
            let dir = tempfile::tempdir().unwrap();
            let plan = dir.path().join("actual-name-01-run-R0-plan.txt");
            let apply = dir.path().join("actual-name-01-run-R0-apply.txt");
            fs::write(&plan, "original plan").unwrap();
            fs::write(&apply, "original apply").unwrap();
            let cli = arguments("ws-abc", dir.path(), &["--all-phases"]);
            let (result, out, err) = execute(&TfeClient::test_client(&server.uri()), &cli).await;
            assert!(result.unwrap_err().contains("1 of 2"));
            assert!(err.contains("run run-R0 plan"));
            assert!(!err.contains("run run-R0 apply"));
            assert_eq!(out.trim(), format!("Saved {}", apply.display()));
            assert_eq!(fs::read_to_string(plan).unwrap(), "original plan");
            assert_eq!(fs::read_to_string(apply).unwrap(), "replacement\n");
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
        }
    }

    struct BrokenReporter;

    #[tokio::test]
    async fn broken_stderr_blob_rate_retry_refreshes_url_and_drains_workers() {
        let server = MockServer::start().await;
        workspace(&server, "ws-abc", false).await;
        history(&server, 2, true, "plan").await;
        let attempts = AtomicUsize::new(0);
        Mock::given(method("GET"))
            .and(path("/blob/0/plan"))
            .respond_with(move |_: &wiremock::Request| {
                ResponseTemplate::new(if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                    429
                } else {
                    200
                })
                .set_body_string("saved")
            })
            .expect(2)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/runs/run-R0/plan"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data":{
                "id":"plan-0", "attributes":{"status":"finished",
                    "log-read-url":format!("{}/blob/0/plan", server.uri())}
            }})))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/blob/1/plan"))
            .respond_with(ResponseTemplate::new(200).set_body_string("saved"))
            .expect(1)
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let cli = arguments("ws-abc", dir.path(), &[]);
        let Command::Logs(args) = &cli.command else {
            panic!()
        };
        let mut out = Vec::new();
        let error = run_bulk_download(
            &TfeClient::test_client(&server.uri()),
            &cli,
            args,
            RetryPolicy::instant(),
            &mut out,
            &mut BrokenReporter,
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("reporting failure"));
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
        assert_eq!(String::from_utf8(out).unwrap().lines().count(), 2);
    }

    #[tokio::test]
    async fn broken_stderr_retry_and_include_fallback_still_save_every_log() {
        for status in [429, 503, 400, 422] {
            let server = MockServer::start().await;
            workspace(&server, "ws-abc", false).await;
            let attempts = AtomicUsize::new(0);
            let blob = format!("{}/blob", server.uri());
            Mock::given(method("GET"))
                .and(path("/workspaces/ws-abc/runs"))
                .respond_with(move |_: &wiremock::Request| {
                    if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                        return ResponseTemplate::new(status);
                    }
                    ResponseTemplate::new(200).set_body_json(json!({
                        "data":[run(0), run(1)],
                        "included":[
                            {"type":"plans","id":"plan-0","attributes":{"log-read-url":blob}},
                            {"type":"plans","id":"plan-1","attributes":{"log-read-url":blob}}
                        ],
                        "meta":{"pagination":{"current-page":1,"total-pages":1,"total-count":2}}
                    }))
                })
                .expect(2)
                .mount(&server)
                .await;
            Mock::given(method("GET"))
                .and(path("/blob"))
                .respond_with(ResponseTemplate::new(200).set_body_string("saved"))
                .expect(2)
                .mount(&server)
                .await;
            let dir = tempfile::tempdir().unwrap();
            let cli = arguments("ws-abc", dir.path(), &[]);
            let Command::Logs(args) = &cli.command else {
                panic!()
            };
            let mut out = Vec::new();
            let error = run_bulk_download(
                &TfeClient::test_client(&server.uri()),
                &cli,
                args,
                RetryPolicy::instant(),
                &mut out,
                &mut BrokenReporter,
            )
            .await
            .unwrap_err();
            assert!(error.to_string().contains("reporting failure"));
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
            assert_eq!(String::from_utf8(out).unwrap().lines().count(), 2);
        }
    }

    #[tokio::test]
    async fn cancellation_during_active_write_joins_and_removes_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("log.txt");
        fs::write(&destination, "original").unwrap();
        let policy = RetryPolicy::instant();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let writer = tokio::task::spawn_blocking({
            let dir = dir.path().to_path_buf();
            let policy = policy.clone();
            move || {
                write_atomic_with(&dir, "log.txt", |temp| {
                    temp.write_all(b"partial")?;
                    started_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    policy
                        .check_cancelled()
                        .map_err(|error| io::Error::other(error.to_string()))
                })
            }
        });
        started_rx.await.unwrap();
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
        policy.cancel();
        release_tx.send(()).unwrap();
        assert!(writer
            .await
            .unwrap()
            .unwrap_err()
            .to_string()
            .contains("cancelled"));
        assert_eq!(fs::read_to_string(destination).unwrap(), "original");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn cancellation_during_blob_fetch_stops_workers_without_partial_files() {
        let server = MockServer::start().await;
        workspace(&server, "ws-abc", false).await;
        history(&server, 8, true, "plan,apply").await;
        let policy = RetryPolicy::instant();
        Mock::given(method("GET"))
            .respond_with({
                let policy = policy.clone();
                move |_: &wiremock::Request| {
                    policy.cancel();
                    ResponseTemplate::new(200).set_body_string("not saved")
                }
            })
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let cli = arguments("ws-abc", dir.path(), &["--all-phases"]);
        let Command::Logs(args) = &cli.command else {
            panic!()
        };
        let mut out = Vec::new();
        let mut err = Vec::new();
        let error = run_bulk_download(
            &TfeClient::test_client(&server.uri()),
            &cli,
            args,
            policy,
            &mut out,
            &mut err,
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("cancelled"));
        assert!(out.is_empty());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    impl AsyncWrite for BrokenReporter {
        fn poll_write(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
            _: &[u8],
        ) -> std::task::Poll<io::Result<usize>> {
            std::task::Poll::Ready(Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "injected reporting failure",
            )))
        }

        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<io::Result<()>> {
            std::task::Poll::Ready(Ok(()))
        }

        fn poll_shutdown(
            self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<io::Result<()>> {
            self.poll_flush(cx)
        }
    }

    #[tokio::test]
    async fn bulk_reporting_failure_drains_workers_and_leaves_no_temps() {
        for fail_stderr in [false, true] {
            let server = MockServer::start().await;
            workspace(&server, "ws-abc", false).await;
            history(&server, 8, true, "plan").await;
            if fail_stderr {
                Mock::given(method("GET"))
                    .and(path("/blob/0/plan"))
                    .respond_with(ResponseTemplate::new(404))
                    .expect(1)
                    .mount(&server)
                    .await;
            }
            Mock::given(method("GET"))
                .respond_with(ResponseTemplate::new(200).set_body_string("saved"))
                .expect(if fail_stderr { 7 } else { 8 })
                .mount(&server)
                .await;
            let dir = tempfile::tempdir().unwrap();
            let cli = arguments("ws-abc", dir.path(), &[]);
            let Command::Logs(args) = &cli.command else {
                panic!()
            };
            let mut good = Vec::new();
            let mut broken = BrokenReporter;
            let (out, err): (&mut (dyn AsyncWrite + Unpin), &mut (dyn AsyncWrite + Unpin)) =
                if fail_stderr {
                    (&mut good, &mut broken)
                } else {
                    (&mut broken, &mut good)
                };
            let error = run_bulk_download(
                &TfeClient::test_client(&server.uri()),
                &cli,
                args,
                RetryPolicy::instant(),
                out,
                err,
            )
            .await
            .unwrap_err();
            assert!(error.to_string().contains("reporting failure"));
            let paths: Vec<_> = fs::read_dir(dir.path())
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .collect();
            assert_eq!(paths.len(), if fail_stderr { 7 } else { 8 });
            for path in paths {
                assert_eq!(path.extension().unwrap(), "txt");
                assert_eq!(fs::read_to_string(path).unwrap(), "saved\n");
            }
        }
    }

    #[tokio::test]
    async fn bulk_handler_rejects_invalid_programmatic_arguments_without_http() {
        for case in 0..5 {
            let server = MockServer::start().await;
            let dir = tempfile::tempdir().unwrap();
            let mut cli = arguments("ws-abc", dir.path(), &[]);
            let Command::Logs(args) = &mut cli.command else {
                panic!()
            };
            match case {
                0 => args.runs = None,
                1 => args.runs = Some(0),
                2 => args.follow = true,
                3 => {
                    args.apply = true;
                    args.all_phases = true;
                }
                _ => args.output_dir = None,
            }
            assert!(execute(&TfeClient::test_client(&server.uri()), &cli)
                .await
                .0
                .is_err());
            assert!(server.received_requests().await.unwrap().is_empty());
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
        }
    }

    #[cfg(unix)]
    #[test]
    fn atomic_overwrite_replaces_symlink_without_touching_external_target() {
        let dir = tempfile::tempdir().unwrap();
        let external = tempfile::NamedTempFile::new().unwrap();
        fs::write(external.path(), "external original").unwrap();
        std::os::unix::fs::symlink(external.path(), dir.path().join("log.txt")).unwrap();
        write_atomic_with(dir.path(), "log.txt", |temp| temp.write_all(b"replacement")).unwrap();
        assert_eq!(
            fs::read_to_string(external.path()).unwrap(),
            "external original"
        );
        assert!(!fs::symlink_metadata(dir.path().join("log.txt"))
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(
            fs::read_to_string(dir.path().join("log.txt")).unwrap(),
            "replacement"
        );
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
