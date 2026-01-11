//! Run command handlers

use std::collections::HashSet;
use std::io::{self, Write};
use std::time::Duration;

use chrono::{DateTime, Utc};
use dialoguer::Confirm;
use tokio::time::sleep;

use crate::cli::{OutputFormat, RunSortField, RunSubresource};
use crate::hcp::runs::{Run, RunEventsResponse, RunQuery};
use crate::hcp::traits::TfeResource;
use crate::hcp::workspaces::{extract_current_run_id, resolve_workspace};
use crate::hcp::TfeClient;
use crate::output::{output_apply, output_plan, output_raw, output_run_events, output_runs};
use crate::ui::{create_spinner, finish_spinner};
use crate::{Cli, Command, GetResource};

/// Maximum results before requiring user confirmation
const CONFIRM_THRESHOLD: usize = 100;

/// Run the runs list command
pub async fn run_runs_command(
    client: &TfeClient,
    cli: &Cli,
) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Get {
        resource: GetResource::Run(args),
    } = &cli.command
    else {
        unreachable!()
    };

    // If run ID is specified, get single run
    if let Some(run_id) = &args.name {
        if run_id.starts_with("run-") {
            return get_single_run(client, cli, run_id).await;
        }
        return Err(format!(
            "Invalid run ID '{}'. Run IDs must start with 'run-'",
            run_id
        )
        .into());
    }

    // Validate that org or ws is provided
    if args.org.is_none() && args.ws.is_none() {
        return Err("Either --org or --ws is required to list runs".into());
    }

    // Validate that workspace_names is only used with org
    if args.workspace_names.is_some() && args.ws.is_some() {
        return Err("--workspace-names can only be used with --org, not --ws".into());
    }

    // Build query
    let mut query = build_run_query(args)?;

    // Add workspace names filter if provided (for org endpoint)
    if let Some(ws_names) = &args.workspace_names {
        query.workspace_names = Some(ws_names.split(',').map(|s| s.trim().to_string()).collect());
    }

    // Fetch runs based on whether we have org or ws
    let runs = if let Some(ws_id) = &args.ws {
        fetch_workspace_runs(client, cli, ws_id, query, args.yes).await?
    } else if let Some(org) = &args.org {
        fetch_org_runs(client, cli, org, query, args.yes).await?
    } else {
        unreachable!()
    };

    if runs.is_empty() {
        println!("\nNo runs found matching the criteria.");
        return Ok(());
    }

    // Sort runs
    let sorted_runs = sort_runs(runs, args.sort, args.reverse);

    // Output
    output_runs(&sorted_runs, &args.output, cli.no_header);

    Ok(())
}

/// Get a single run by ID
async fn get_single_run(
    client: &TfeClient,
    cli: &Cli,
    run_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Get {
        resource: GetResource::Run(args),
    } = &cli.command
    else {
        unreachable!()
    };

    let spinner = create_spinner(&format!("Fetching run '{}'...", run_id), cli.batch);

    match client.get_run_by_id(run_id).await {
        Ok(Some((run, raw))) => {
            finish_spinner(spinner);

            // Handle subresource if requested
            if let Some(subresource) = &args.subresource {
                return fetch_and_output_subresource(client, cli, &raw, subresource).await;
            }

            // For single run, output raw JSON/YAML or table
            match args.output {
                OutputFormat::Json | OutputFormat::Yaml => {
                    output_raw(&raw, &args.output);
                }
                _ => {
                    // For table/csv, convert to single-item list
                    output_runs(&[run], &args.output, cli.no_header);
                }
            }
            Ok(())
        }
        Ok(None) => {
            finish_spinner(spinner);
            Err(format!("Run '{}' not found", run_id).into())
        }
        Err(e) => {
            finish_spinner(spinner);
            Err(e.into())
        }
    }
}

/// Build RunQuery from CLI arguments
/// Always uses non_final status group. --status filters within non_final only.
fn build_run_query(args: &crate::cli::RunArgs) -> Result<RunQuery, Box<dyn std::error::Error>> {
    use crate::hcp::runs::RunStatus;

    // If explicit statuses provided, validate they are non-final and use them
    if let Some(status_str) = &args.status {
        let statuses: Result<Vec<RunStatus>, _> =
            status_str.split(',').map(|s| s.trim().parse()).collect();

        match statuses {
            Ok(s) => {
                // Validate all statuses are non-final
                for status in &s {
                    if !status.is_non_final() {
                        return Err(format!(
                            "Status '{}' is a final status. Only non-final statuses are allowed.",
                            status
                        )
                        .into());
                    }
                }
                return Ok(RunQuery::with_statuses(s));
            }
            Err(e) => {
                return Err(format!("Invalid status: {}", e).into());
            }
        }
    }

    // Default: all non_final runs
    Ok(RunQuery::non_final())
}

/// Fetch runs from a workspace
async fn fetch_workspace_runs(
    client: &TfeClient,
    cli: &Cli,
    ws_id: &str,
    query: RunQuery,
    auto_confirm: bool,
) -> Result<Vec<Run>, Box<dyn std::error::Error>> {
    let spinner = create_spinner(
        &format!("Fetching runs for workspace '{}'...", ws_id),
        cli.batch,
    );

    // First fetch with limit to check count
    let initial_runs = client
        .get_runs_for_workspace(ws_id, query.clone(), Some(CONFIRM_THRESHOLD as u32 + 1))
        .await?;

    if initial_runs.len() > CONFIRM_THRESHOLD {
        finish_spinner(spinner);

        if !auto_confirm && !cli.batch {
            let confirm = Confirm::new()
                .with_prompt(format!(
                    "Found more than {} runs. Continue fetching all?",
                    CONFIRM_THRESHOLD
                ))
                .default(false)
                .interact()?;

            if !confirm {
                return Ok(initial_runs.into_iter().take(CONFIRM_THRESHOLD).collect());
            }
        } else if cli.batch && !auto_confirm {
            // In batch mode without --yes, limit to threshold
            return Ok(initial_runs.into_iter().take(CONFIRM_THRESHOLD).collect());
        }

        // Fetch all runs
        let spinner = create_spinner("Fetching all runs...", cli.batch);
        let all_runs = client.get_runs_for_workspace(ws_id, query, None).await?;
        finish_spinner(spinner);
        return Ok(all_runs);
    }

    finish_spinner(spinner);
    Ok(initial_runs)
}

/// Fetch runs from an organization
async fn fetch_org_runs(
    client: &TfeClient,
    cli: &Cli,
    org: &str,
    query: RunQuery,
    auto_confirm: bool,
) -> Result<Vec<Run>, Box<dyn std::error::Error>> {
    let spinner = create_spinner(
        &format!("Fetching runs for organization '{}'...", org),
        cli.batch,
    );

    // First fetch with limit to check count
    let initial_runs = client
        .get_runs_for_organization(org, query.clone(), Some(CONFIRM_THRESHOLD as u32 + 1))
        .await?;

    if initial_runs.len() > CONFIRM_THRESHOLD {
        finish_spinner(spinner);

        if !auto_confirm && !cli.batch {
            let confirm = Confirm::new()
                .with_prompt(format!(
                    "Found more than {} runs. Continue fetching all?",
                    CONFIRM_THRESHOLD
                ))
                .default(false)
                .interact()?;

            if !confirm {
                return Ok(initial_runs.into_iter().take(CONFIRM_THRESHOLD).collect());
            }
        } else if cli.batch && !auto_confirm {
            // In batch mode without --yes, limit to threshold
            return Ok(initial_runs.into_iter().take(CONFIRM_THRESHOLD).collect());
        }

        // Fetch all runs
        let spinner = create_spinner("Fetching all runs...", cli.batch);
        let all_runs = client.get_runs_for_organization(org, query, None).await?;
        finish_spinner(spinner);
        return Ok(all_runs);
    }

    finish_spinner(spinner);
    Ok(initial_runs)
}

/// Sort runs by the specified field
fn sort_runs(mut runs: Vec<Run>, sort_field: RunSortField, reverse: bool) -> Vec<Run> {
    runs.sort_by(|a, b| {
        let cmp = match sort_field {
            RunSortField::CreatedAt => {
                // Default: newest first (reverse chronological)
                b.created_at().cmp(a.created_at())
            }
            RunSortField::Status => a.status().cmp(b.status()),
            RunSortField::WsId => a
                .workspace_id()
                .unwrap_or("")
                .cmp(b.workspace_id().unwrap_or("")),
        };

        if reverse {
            cmp.reverse()
        } else {
            cmp
        }
    });
    runs
}

/// Fetch and output a run subresource
async fn fetch_and_output_subresource(
    client: &TfeClient,
    cli: &Cli,
    run_raw: &serde_json::Value,
    subresource: &RunSubresource,
) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Get {
        resource: GetResource::Run(args),
    } = &cli.command
    else {
        unreachable!()
    };

    let run_id = run_raw["data"]["id"]
        .as_str()
        .ok_or("Missing run ID in response")?;

    match subresource {
        RunSubresource::Events => fetch_and_output_events(client, cli, run_raw).await,
        RunSubresource::Plan => {
            fetch_and_output_plan(client, cli, run_id, args.get_log, args.tail_log).await
        }
        RunSubresource::Apply => {
            fetch_and_output_apply(client, cli, run_id, args.get_log, args.tail_log).await
        }
    }
}

/// Fetch and output run events
async fn fetch_and_output_events(
    client: &TfeClient,
    cli: &Cli,
    run_raw: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Get {
        resource: GetResource::Run(args),
    } = &cli.command
    else {
        unreachable!()
    };

    let url = run_raw["data"]["relationships"]["run-events"]["links"]["related"]
        .as_str()
        .ok_or("No 'run-events' relationship found for this run")?;

    let spinner = create_spinner("Fetching run-events...", cli.batch);

    match client.get_subresource(url).await {
        Ok(raw) => {
            finish_spinner(spinner);
            let events_response: RunEventsResponse = serde_json::from_value(raw.clone())?;
            output_run_events(&events_response.data, &args.output, cli.no_header, &raw);
            Ok(())
        }
        Err(e) => {
            finish_spinner(spinner);
            Err(e.into())
        }
    }
}

/// Fetch and output plan details
async fn fetch_and_output_plan(
    client: &TfeClient,
    cli: &Cli,
    run_id: &str,
    get_log: bool,
    tail_log: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Get {
        resource: GetResource::Run(args),
    } = &cli.command
    else {
        unreachable!()
    };

    if tail_log {
        return tail_plan_log(client, cli.batch, run_id, args.raw).await;
    }

    let spinner = create_spinner("Fetching plan details...", cli.batch);

    match client.get_run_plan(run_id).await {
        Ok(plan) => {
            finish_spinner(spinner);

            if get_log {
                return output_log(client, &plan.attributes.log_read_url, args.raw).await;
            }

            // Create raw JSON for JSON/YAML output
            let raw_json = serde_json::json!({
                "data": {
                    "id": plan.id,
                    "type": "plans",
                    "attributes": {
                        "status": plan.status(),
                        "has-changes": plan.has_changes(),
                        "resource-additions": plan.resource_additions(),
                        "resource-changes": plan.resource_changes(),
                        "resource-destructions": plan.resource_destructions(),
                        "resource-imports": plan.resource_imports()
                    }
                }
            });
            output_plan(&plan, &args.output, cli.no_header, &raw_json);
            Ok(())
        }
        Err(e) => {
            finish_spinner(spinner);
            Err(e.into())
        }
    }
}

/// Fetch and output apply details
async fn fetch_and_output_apply(
    client: &TfeClient,
    cli: &Cli,
    run_id: &str,
    get_log: bool,
    tail_log: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Get {
        resource: GetResource::Run(args),
    } = &cli.command
    else {
        unreachable!()
    };

    if tail_log {
        return tail_apply_log(client, cli.batch, run_id, args.raw).await;
    }

    let spinner = create_spinner("Fetching apply details...", cli.batch);

    match client.get_run_apply(run_id).await {
        Ok(apply) => {
            finish_spinner(spinner);

            if get_log {
                return output_log(client, &apply.attributes.log_read_url, args.raw).await;
            }

            // Create raw JSON for JSON/YAML output
            let raw_json = serde_json::json!({
                "data": {
                    "id": apply.id,
                    "type": "applies",
                    "attributes": {
                        "status": apply.status(),
                        "resource-additions": apply.resource_additions(),
                        "resource-changes": apply.resource_changes(),
                        "resource-destructions": apply.resource_destructions(),
                        "resource-imports": apply.resource_imports()
                    }
                }
            });
            output_apply(&apply, &args.output, cli.no_header, &raw_json);
            Ok(())
        }
        Err(e) => {
            finish_spinner(spinner);
            Err(e.into())
        }
    }
}

/// Output log content from a log-read-url
///
/// By default, parses JSON lines and extracts @message for human-readable output.
/// With raw=true, outputs the original log content without parsing.
async fn output_log(
    client: &TfeClient,
    log_read_url: &Option<String>,
    raw: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let url = log_read_url
        .as_ref()
        .ok_or("No log-read-url available for this resource")?;

    let content = client.get_log_content(url).await?;

    if raw {
        print!("{}", content);
    } else {
        print_human_readable_log(&content);
    }
    Ok(())
}

// Use shared log parsing from log_utils module
use super::log_utils::print_human_readable_log;

/// Fetch and print log for a run (plan or apply)
///
/// Public function used by both `get run --subresource` and `logs` commands.
///
/// # Arguments
/// * `client` - TFE API client
/// * `run_id` - Run ID to fetch logs for
/// * `is_apply` - If true, fetch apply log; if false, fetch plan log
/// * `raw` - If true, output raw log; if false, extract @message from JSON lines
pub async fn fetch_and_print_log(
    client: &TfeClient,
    run_id: &str,
    is_apply: bool,
    raw: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let log_url = if is_apply {
        let apply = client.get_run_apply(run_id).await?;
        apply.attributes.log_read_url
    } else {
        let plan = client.get_run_plan(run_id).await?;
        plan.attributes.log_read_url
    };

    output_log(client, &log_url, raw).await
}

/// Tail plan log - delegates to unified tail_log
async fn tail_plan_log(
    client: &TfeClient,
    batch: bool,
    run_id: &str,
    raw: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    tail_log(client, batch, run_id, false, raw).await
}

/// Tail apply log - delegates to unified tail_log
async fn tail_apply_log(
    client: &TfeClient,
    batch: bool,
    run_id: &str,
    raw: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    tail_log(client, batch, run_id, true, raw).await
}

/// Unified log tailing for both plan and apply
///
/// Polls the plan/apply status and log content, displaying new lines as they appear.
/// Stops when the resource reaches a final state (finished, errored, canceled, unreachable).
///
/// # Arguments
/// * `client` - TFE API client
/// * `batch` - If true, no spinners (batch mode)
/// * `run_id` - Run ID to tail logs for
/// * `is_apply` - If true, tail apply log; if false, tail plan log
/// * `raw` - If true, output raw log; if false, extract @message from JSON lines
pub async fn tail_log(
    client: &TfeClient,
    batch: bool,
    run_id: &str,
    is_apply: bool,
    raw: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    const POLL_INTERVAL: Duration = Duration::from_secs(2);

    let resource_name = if is_apply { "apply" } else { "plan" };
    let mut last_log_len = 0;
    let mut spinner = create_spinner(&format!("Tailing {} log...", resource_name), batch);

    loop {
        // Fetch log URL and final state based on resource type
        let (log_url, is_final) = if is_apply {
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
                    // On first content, finish the spinner
                    if last_log_len == 0 {
                        finish_spinner(spinner.take());
                    }
                    // Print only new content
                    let new_content = &content[last_log_len..];
                    if raw {
                        print!("{}", new_content);
                    } else {
                        print_human_readable_log(new_content);
                    }
                    io::stdout().flush().ok();
                    last_log_len = content.len();
                }
            }
        }

        // Check if resource has reached final state
        if is_final {
            break;
        }

        sleep(POLL_INTERVAL).await;
    }

    // Finish spinner if never got any content
    finish_spinner(spinner.take());
    Ok(())
}

/// Run the purge run command (cancel/discard pending runs)
pub async fn run_purge_run_command(
    client: &TfeClient,
    cli: &crate::Cli,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let Command::Purge {
        resource: crate::PurgeResource::Run(args),
    } = &cli.command
    else {
        unreachable!()
    };

    // Step 1: Resolve workspace
    let resolved =
        resolve_workspace(client, &args.workspace, args.org.as_deref(), cli.batch).await?;
    let workspace = &resolved.workspace;
    let ws_id = &workspace.id;
    let ws_name = workspace.name();
    let org = &resolved.org;

    // Extract current run ID from workspace relationships (may not exist)
    let current_run_id: Option<String> = extract_current_run_id(&resolved.raw).ok();

    // Step 2: Fetch pending runs (all non-final runs that could be blocking)
    let spinner = create_spinner("Fetching pending runs...", cli.batch);
    let pending_runs = client
        .get_runs_for_workspace(ws_id, RunQuery::non_final(), None)
        .await?;
    finish_spinner(spinner);

    // Collect runs to process (non-final runs + current, deduplicated)
    // Include runs that are cancelable or discardable
    let mut runs_to_process: Vec<Run> = Vec::new();
    let mut seen_ids: HashSet<String> = HashSet::new();

    // Add non-final runs that can be canceled or discarded
    for run in pending_runs {
        if !seen_ids.contains(&run.id) && determine_action(&run).is_some() {
            seen_ids.insert(run.id.clone());
            runs_to_process.push(run);
        }
    }

    // Add current run if it exists, not already in list, and is actionable
    if let Some(ref curr_id) = current_run_id {
        if !seen_ids.contains(curr_id) {
            let spinner =
                create_spinner(&format!("Fetching current run {}...", curr_id), cli.batch);
            if let Ok(Some((run, _))) = client.get_run_by_id(curr_id).await {
                finish_spinner(spinner);
                // Only add if the run is cancelable or discardable (not final)
                if determine_action(&run).is_some() {
                    seen_ids.insert(run.id.clone());
                    runs_to_process.push(run);
                }
            } else {
                finish_spinner(spinner);
            }
        }
    }

    if runs_to_process.is_empty() {
        println!(
            "\n✓ No pending runs to process for workspace '{}'.",
            ws_name
        );
        return Ok(());
    }

    // Display header
    let dry_run_prefix = if args.dry_run { "[DRY-RUN] " } else { "" };
    println!();
    println!("{}Workspace:    {} ({})", dry_run_prefix, ws_name, ws_id);
    println!("{}Organization: {}", dry_run_prefix, org);
    println!("{}TFE instance: {}", dry_run_prefix, client.host());
    println!();
    println!("{}The following runs will be processed:", dry_run_prefix);
    println!();

    // Build and display table
    output_pending_runs_table(
        &runs_to_process,
        client.host(),
        org,
        ws_name,
        &current_run_id,
    );

    println!();

    // Confirmation prompt
    print!("{}Do you want to continue? (yes/no): ", dry_run_prefix);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    if input != "yes" && input != "y" {
        println!("\nAborted.");
        return Ok(());
    }

    println!();

    // Sort runs: pending (newest first), then current run last
    // Newest first = reverse chronological order by created_at
    runs_to_process.sort_by(|a, b| {
        let a_is_current = current_run_id.as_ref() == Some(&a.id);
        let b_is_current = current_run_id.as_ref() == Some(&b.id);

        // Current run goes last
        if a_is_current && !b_is_current {
            return std::cmp::Ordering::Greater;
        }
        if b_is_current && !a_is_current {
            return std::cmp::Ordering::Less;
        }

        // Sort by created_at descending (newest first)
        let a_time = a.attributes.created_at.as_deref().unwrap_or("");
        let b_time = b.attributes.created_at.as_deref().unwrap_or("");
        b_time.cmp(a_time)
    });

    // Process runs
    let mut success_count = 0;
    let mut error_count = 0;

    for run in &runs_to_process {
        let action = determine_action(run);
        let action_str = match action {
            Some(RunAction::Cancel) => "cancel",
            Some(RunAction::Discard) => "discard",
            None => "skip",
        };

        if args.dry_run {
            match action {
                Some(RunAction::Cancel) => {
                    println!("[DRY-RUN] Would cancel run: {}", run.id);
                }
                Some(RunAction::Discard) => {
                    println!("[DRY-RUN] Would discard run: {}", run.id);
                }
                None => {
                    println!(
                        "[DRY-RUN] Would skip run: {} (not cancelable/discardable)",
                        run.id
                    );
                }
            }
            success_count += 1;
        } else {
            match action {
                Some(RunAction::Cancel) => {
                    match client.cancel_run(&run.id).await {
                        Ok(()) => {
                            println!("✓ Canceled run: {}", run.id);
                            success_count += 1;
                        }
                        Err(e) => {
                            eprintln!("✗ Failed to {} run {}: {}", action_str, run.id, e);
                            error_count += 1;
                            // Stop on first error per spec
                            break;
                        }
                    }
                }
                Some(RunAction::Discard) => {
                    match client.discard_run(&run.id).await {
                        Ok(()) => {
                            println!("✓ Discarded run: {}", run.id);
                            success_count += 1;
                        }
                        Err(e) => {
                            eprintln!("✗ Failed to {} run {}: {}", action_str, run.id, e);
                            error_count += 1;
                            // Stop on first error per spec
                            break;
                        }
                    }
                }
                None => {
                    println!("⚠ Skipped run: {} (not cancelable/discardable)", run.id);
                }
            }
        }
    }

    // Summary
    println!();
    if args.dry_run {
        println!("Dry-run complete. No changes were made.");
    } else if error_count > 0 {
        println!(
            "Processed {} runs. {} succeeded, {} failed.",
            success_count + error_count,
            success_count,
            error_count
        );
    } else {
        println!("All {} runs processed successfully.", success_count);
    }

    Ok(())
}

/// Action to take on a run
enum RunAction {
    Cancel,
    Discard,
}

/// Determine the appropriate action for a run based on its actions flags
fn determine_action(run: &Run) -> Option<RunAction> {
    if let Some(actions) = &run.attributes.actions {
        if actions.is_cancelable == Some(true) {
            return Some(RunAction::Cancel);
        }
        if actions.is_discardable == Some(true) {
            return Some(RunAction::Discard);
        }
    }
    None
}

/// Format age from ISO timestamp
fn format_age(created_at: Option<&str>) -> String {
    let Some(ts) = created_at else {
        return "unknown".to_string();
    };

    let Ok(dt) = ts.parse::<DateTime<Utc>>() else {
        return "unknown".to_string();
    };

    let now = Utc::now();
    let duration = now.signed_duration_since(dt);

    if duration.num_days() > 0 {
        format!("{}d {}h", duration.num_days(), duration.num_hours() % 24)
    } else if duration.num_hours() > 0 {
        format!("{}h {}m", duration.num_hours(), duration.num_minutes() % 60)
    } else if duration.num_minutes() > 0 {
        format!("{}m", duration.num_minutes())
    } else {
        format!("{}s", duration.num_seconds())
    }
}

/// Output pending runs table using comfy_table
fn output_pending_runs_table(
    runs: &[Run],
    host: &str,
    org: &str,
    ws_name: &str,
    current_run_id: &Option<String>,
) {
    use comfy_table::{presets::UTF8_FULL_CONDENSED, Table};

    let mut table = Table::new();
    table.load_preset(UTF8_FULL_CONDENSED);
    table.set_header(vec!["Run ID", "Status", "Age", "Action", "URL"]);

    for run in runs {
        let action = determine_action(run);
        let action_str = match action {
            Some(RunAction::Cancel) => "cancel",
            Some(RunAction::Discard) => "discard",
            None => "skip",
        };

        let status = if current_run_id.as_ref() == Some(&run.id) {
            format!("{} (current)", run.attributes.status)
        } else {
            run.attributes.status.clone()
        };

        let age = format_age(run.attributes.created_at.as_deref());
        let url = format!(
            "https://{}/app/{}/workspaces/{}/runs/{}",
            host, org, ws_name, run.id
        );

        table.add_row(vec![&run.id, &status, &age, action_str, &url]);
    }

    println!("{}", table);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confirm_threshold() {
        assert_eq!(CONFIRM_THRESHOLD, 100);
    }

    #[test]
    fn test_format_age_minutes() {
        // Recent timestamp - should show minutes
        let now = Utc::now();
        let five_min_ago = now - chrono::Duration::minutes(5);
        let ts = five_min_ago.to_rfc3339();
        let age = format_age(Some(&ts));
        assert!(age.contains("m") || age.contains("s"));
    }

    #[test]
    fn test_format_age_hours() {
        let now = Utc::now();
        let two_hours_ago = now - chrono::Duration::hours(2);
        let ts = two_hours_ago.to_rfc3339();
        let age = format_age(Some(&ts));
        assert!(age.contains("h"));
    }

    #[test]
    fn test_format_age_days() {
        let now = Utc::now();
        let two_days_ago = now - chrono::Duration::days(2);
        let ts = two_days_ago.to_rfc3339();
        let age = format_age(Some(&ts));
        assert!(age.contains("d"));
    }

    #[test]
    fn test_format_age_none() {
        assert_eq!(format_age(None), "unknown");
    }

    #[test]
    fn test_format_age_invalid() {
        assert_eq!(format_age(Some("not-a-date")), "unknown");
    }

    #[test]
    fn test_determine_action_cancelable() {
        let run = Run {
            id: "run-test".to_string(),
            attributes: crate::hcp::runs::RunAttributes {
                status: "planning".to_string(),
                message: None,
                source: None,
                created_at: None,
                has_changes: None,
                is_destroy: None,
                plan_only: None,
                auto_apply: None,
                trigger_reason: None,
                actions: Some(crate::hcp::runs::RunActions {
                    is_cancelable: Some(true),
                    is_confirmable: None,
                    is_discardable: Some(false),
                    is_force_cancelable: None,
                }),
            },
            relationships: None,
        };
        assert!(matches!(determine_action(&run), Some(RunAction::Cancel)));
    }

    #[test]
    fn test_determine_action_discardable() {
        let run = Run {
            id: "run-test".to_string(),
            attributes: crate::hcp::runs::RunAttributes {
                status: "pending".to_string(),
                message: None,
                source: None,
                created_at: None,
                has_changes: None,
                is_destroy: None,
                plan_only: None,
                auto_apply: None,
                trigger_reason: None,
                actions: Some(crate::hcp::runs::RunActions {
                    is_cancelable: Some(false),
                    is_confirmable: None,
                    is_discardable: Some(true),
                    is_force_cancelable: None,
                }),
            },
            relationships: None,
        };
        assert!(matches!(determine_action(&run), Some(RunAction::Discard)));
    }

    #[test]
    fn test_determine_action_none() {
        let run = Run {
            id: "run-test".to_string(),
            attributes: crate::hcp::runs::RunAttributes {
                status: "applied".to_string(),
                message: None,
                source: None,
                created_at: None,
                has_changes: None,
                is_destroy: None,
                plan_only: None,
                auto_apply: None,
                trigger_reason: None,
                actions: Some(crate::hcp::runs::RunActions {
                    is_cancelable: Some(false),
                    is_confirmable: None,
                    is_discardable: Some(false),
                    is_force_cancelable: None,
                }),
            },
            relationships: None,
        };
        assert!(determine_action(&run).is_none());
    }

    // Note: print_human_readable_log tests moved to log_utils module
}
