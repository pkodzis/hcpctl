//! Run output formatter

use super::common::escape_csv;
use crate::cli::OutputFormat;
use crate::hcp::runs::{Apply, Plan, RunEvent};
use crate::hcp::Run;
use comfy_table::{presets::NOTHING, Table};
use serde::Serialize;

/// Serializable run for structured output (JSON/YAML)
#[derive(Serialize)]
struct SerializableRun {
    run_id: String,
    workspace_id: String,
    status: String,
    source: String,
    message: String,
    has_changes: bool,
    is_destroy: bool,
    plan_only: bool,
    trigger_reason: String,
    created_at: String,
}

impl From<&Run> for SerializableRun {
    fn from(run: &Run) -> Self {
        Self {
            run_id: run.id.clone(),
            workspace_id: run.workspace_id().unwrap_or("").to_string(),
            status: run.status().to_string(),
            source: run.source().to_string(),
            message: run.message().to_string(),
            has_changes: run.has_changes(),
            is_destroy: run.is_destroy(),
            plan_only: run.is_plan_only(),
            trigger_reason: run.trigger_reason().to_string(),
            created_at: run.created_at().to_string(),
        }
    }
}

/// Output runs in the specified format
pub fn output_runs(runs: &[Run], format: &OutputFormat, no_header: bool) {
    match format {
        OutputFormat::Table => output_table(runs, no_header),
        OutputFormat::Csv => output_csv(runs, no_header),
        OutputFormat::Json => output_json(runs),
        OutputFormat::Yaml => output_yaml(runs),
    }
}

fn output_table(runs: &[Run], no_header: bool) {
    let mut table = Table::new();
    table.load_preset(NOTHING);
    if !no_header {
        table.set_header(vec![
            "Run ID",
            "Workspace ID",
            "Status",
            "Source",
            "Changes",
            "Destroy",
            "Plan Only",
            "Trigger",
            "Created At",
        ]);
    }

    for run in runs {
        let has_changes = if run.has_changes() { "Yes" } else { "No" };
        let is_destroy = if run.is_destroy() { "Yes" } else { "No" };
        let plan_only = if run.is_plan_only() { "Yes" } else { "No" };

        table.add_row(vec![
            &run.id,
            run.workspace_id().unwrap_or(""),
            run.status(),
            run.source(),
            has_changes,
            is_destroy,
            plan_only,
            run.trigger_reason(),
            run.created_at(),
        ]);
    }

    println!();
    println!("{table}");
    if !no_header {
        println!("\nTotal: {} runs", runs.len());
    }
}

fn output_csv(runs: &[Run], no_header: bool) {
    if !no_header {
        println!("run_id,workspace_id,status,source,message,has_changes,is_destroy,plan_only,trigger_reason,created_at");
    }

    for run in runs {
        println!(
            "{},{},{},{},{},{},{},{},{},{}",
            escape_csv(&run.id),
            escape_csv(run.workspace_id().unwrap_or("")),
            escape_csv(run.status()),
            escape_csv(run.source()),
            escape_csv(run.message()),
            run.has_changes(),
            run.is_destroy(),
            run.is_plan_only(),
            escape_csv(run.trigger_reason()),
            escape_csv(run.created_at())
        );
    }
}

fn output_json(runs: &[Run]) {
    let data: Vec<SerializableRun> = runs.iter().map(SerializableRun::from).collect();
    super::common::print_json(&data);
}

fn output_yaml(runs: &[Run]) {
    let data: Vec<SerializableRun> = runs.iter().map(SerializableRun::from).collect();
    super::common::print_yaml(&data);
}

/// Output run events in the specified format
pub fn output_run_events(
    events: &[RunEvent],
    format: &OutputFormat,
    no_header: bool,
    raw: &serde_json::Value,
) {
    match format {
        OutputFormat::Table => output_events_table(events, no_header),
        OutputFormat::Csv => output_events_csv(events, no_header),
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(raw).unwrap()),
        OutputFormat::Yaml => println!("{}", serde_yml::to_string(raw).unwrap()),
    }
}

fn output_events_table(events: &[RunEvent], no_header: bool) {
    let mut table = Table::new();
    table.load_preset(NOTHING);
    if !no_header {
        table.set_header(vec![
            "Event ID",
            "Action",
            "Target ID",
            "Target Type",
            "Created At",
        ]);
    }

    for event in events {
        table.add_row(vec![
            &event.id,
            event.action(),
            event.target_id(),
            event.target_type(),
            event.created_at(),
        ]);
    }

    println!();
    println!("{table}");
    if !no_header {
        println!("\nTotal: {} events", events.len());
    }
}

fn output_events_csv(events: &[RunEvent], no_header: bool) {
    if !no_header {
        println!("event_id,action,target_id,target_type,created_at");
    }

    for event in events {
        println!(
            "{},{},{},{},{}",
            escape_csv(&event.id),
            escape_csv(event.action()),
            escape_csv(event.target_id()),
            escape_csv(event.target_type()),
            escape_csv(event.created_at())
        );
    }
}

/// Output plan in the specified format
pub fn output_plan(plan: &Plan, format: &OutputFormat, no_header: bool, raw: &serde_json::Value) {
    match format {
        OutputFormat::Table => output_plan_table(plan, no_header),
        OutputFormat::Csv => output_plan_csv(plan, no_header),
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(raw).unwrap()),
        OutputFormat::Yaml => println!("{}", serde_yml::to_string(raw).unwrap()),
    }
}

fn output_plan_table(plan: &Plan, no_header: bool) {
    let mut table = Table::new();
    table.load_preset(NOTHING);
    if !no_header {
        table.set_header(vec![
            "Plan ID",
            "Status",
            "Changes",
            "Additions",
            "Changes",
            "Destructions",
            "Imports",
        ]);
    }

    let has_changes = if plan.has_changes() { "Yes" } else { "No" };

    table.add_row(vec![
        &plan.id,
        plan.status(),
        has_changes,
        &plan.resource_additions().to_string(),
        &plan.resource_changes().to_string(),
        &plan.resource_destructions().to_string(),
        &plan.resource_imports().to_string(),
    ]);

    println!();
    println!("{table}");
}

fn output_plan_csv(plan: &Plan, no_header: bool) {
    if !no_header {
        println!("plan_id,status,has_changes,additions,changes,destructions,imports");
    }

    println!(
        "{},{},{},{},{},{},{}",
        escape_csv(&plan.id),
        escape_csv(plan.status()),
        plan.has_changes(),
        plan.resource_additions(),
        plan.resource_changes(),
        plan.resource_destructions(),
        plan.resource_imports()
    );
}

/// Output apply in the specified format
pub fn output_apply(
    apply: &Apply,
    format: &OutputFormat,
    no_header: bool,
    raw: &serde_json::Value,
) {
    match format {
        OutputFormat::Table => output_apply_table(apply, no_header),
        OutputFormat::Csv => output_apply_csv(apply, no_header),
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(raw).unwrap()),
        OutputFormat::Yaml => println!("{}", serde_yml::to_string(raw).unwrap()),
    }
}

fn output_apply_table(apply: &Apply, no_header: bool) {
    let mut table = Table::new();
    table.load_preset(NOTHING);
    if !no_header {
        table.set_header(vec![
            "Apply ID",
            "Status",
            "Additions",
            "Changes",
            "Destructions",
            "Imports",
        ]);
    }

    table.add_row(vec![
        &apply.id,
        apply.status(),
        &apply.resource_additions().to_string(),
        &apply.resource_changes().to_string(),
        &apply.resource_destructions().to_string(),
        &apply.resource_imports().to_string(),
    ]);

    println!();
    println!("{table}");
}

fn output_apply_csv(apply: &Apply, no_header: bool) {
    if !no_header {
        println!("apply_id,status,additions,changes,destructions,imports");
    }

    println!(
        "{},{},{},{},{},{}",
        escape_csv(&apply.id),
        escape_csv(apply.status()),
        apply.resource_additions(),
        apply.resource_changes(),
        apply.resource_destructions(),
        apply.resource_imports()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_run() -> Run {
        serde_json::from_value(serde_json::json!({
            "id": "run-abc123",
            "type": "runs",
            "attributes": {
                "status": "planning",
                "message": "Test run",
                "source": "tfe-api",
                "created-at": "2025-01-01T10:00:00.000Z",
                "has-changes": true,
                "is-destroy": false,
                "plan-only": false,
                "trigger-reason": "manual"
            },
            "relationships": {
                "workspace": {
                    "data": {
                        "id": "ws-xyz789",
                        "type": "workspaces"
                    }
                }
            }
        }))
        .unwrap()
    }

    #[test]
    fn test_serializable_run_from() {
        let run = create_test_run();
        let serializable: SerializableRun = (&run).into();

        assert_eq!(serializable.run_id, "run-abc123");
        assert_eq!(serializable.workspace_id, "ws-xyz789");
        assert_eq!(serializable.status, "planning");
        assert_eq!(serializable.source, "tfe-api");
        assert!(serializable.has_changes);
        assert!(!serializable.is_destroy);
        assert!(!serializable.plan_only);
    }

    #[test]
    fn test_output_json() {
        let run = create_test_run();
        // Just verify it doesn't panic
        let data: Vec<SerializableRun> = [&run].iter().map(|r| SerializableRun::from(*r)).collect();
        let json = serde_json::to_string_pretty(&data).unwrap();
        assert!(json.contains("run-abc123"));
    }

    fn create_test_run_event() -> RunEvent {
        serde_json::from_value(serde_json::json!({
            "id": "re-abc123",
            "type": "run-events",
            "attributes": {
                "action": "queued",
                "created-at": "2025-01-01T10:00:00.000Z",
                "description": null
            },
            "relationships": {
                "target": {
                    "data": {
                        "id": "plan-xyz789",
                        "type": "plans"
                    }
                }
            }
        }))
        .unwrap()
    }

    #[test]
    fn test_run_event_accessors() {
        let event = create_test_run_event();
        assert_eq!(event.action(), "queued");
        assert_eq!(event.created_at(), "2025-01-01T10:00:00.000Z");
        assert_eq!(event.target_id(), "plan-xyz789");
        assert_eq!(event.target_type(), "plans");
    }

    #[test]
    fn test_run_event_no_target() {
        let event: RunEvent = serde_json::from_value(serde_json::json!({
            "id": "re-abc123",
            "type": "run-events",
            "attributes": {
                "action": "confirmed",
                "created-at": "2025-01-01T10:00:00.000Z"
            },
            "relationships": {
                "target": {
                    "data": null
                }
            }
        }))
        .unwrap();

        assert_eq!(event.target_id(), "");
        assert_eq!(event.target_type(), "");
    }

    #[test]
    fn test_output_events_csv_format() {
        // Just verify it doesn't panic
        let events = vec![create_test_run_event()];
        output_events_csv(&events, false);
        output_events_csv(&events, true);
    }

    fn create_test_plan() -> Plan {
        serde_json::from_value(serde_json::json!({
            "id": "plan-abc123",
            "type": "plans",
            "attributes": {
                "status": "finished",
                "has-changes": true,
                "resource-additions": 5,
                "resource-changes": 2,
                "resource-destructions": 1,
                "resource-imports": 0,
                "log-read-url": "https://archivist.terraform.io/v1/object/abc123"
            }
        }))
        .unwrap()
    }

    fn create_test_apply() -> Apply {
        serde_json::from_value(serde_json::json!({
            "id": "apply-xyz789",
            "type": "applies",
            "attributes": {
                "status": "finished",
                "resource-additions": 3,
                "resource-changes": 1,
                "resource-destructions": 0,
                "resource-imports": 2,
                "log-read-url": "https://archivist.terraform.io/v1/object/xyz789"
            }
        }))
        .unwrap()
    }

    #[test]
    fn test_output_plan_table_no_panic() {
        let plan = create_test_plan();
        output_plan_table(&plan, false);
        output_plan_table(&plan, true);
    }

    #[test]
    fn test_output_plan_csv_format() {
        let plan = create_test_plan();
        output_plan_csv(&plan, false);
        output_plan_csv(&plan, true);
    }

    #[test]
    fn test_output_apply_table_no_panic() {
        let apply = create_test_apply();
        output_apply_table(&apply, false);
        output_apply_table(&apply, true);
    }

    #[test]
    fn test_output_apply_csv_format() {
        let apply = create_test_apply();
        output_apply_csv(&apply, false);
        output_apply_csv(&apply, true);
    }

    #[test]
    fn test_plan_accessors() {
        let plan = create_test_plan();
        assert_eq!(plan.status(), "finished");
        assert!(plan.has_changes());
        assert_eq!(plan.resource_additions(), 5);
        assert_eq!(plan.resource_changes(), 2);
        assert_eq!(plan.resource_destructions(), 1);
        assert_eq!(plan.resource_imports(), 0);
        assert!(plan.log_read_url().is_some());
    }

    #[test]
    fn test_apply_accessors() {
        let apply = create_test_apply();
        assert_eq!(apply.status(), "finished");
        assert_eq!(apply.resource_additions(), 3);
        assert_eq!(apply.resource_changes(), 1);
        assert_eq!(apply.resource_destructions(), 0);
        assert_eq!(apply.resource_imports(), 2);
        assert!(apply.log_read_url().is_some());
    }
}
