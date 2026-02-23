//! Workspace output formatter

use super::common::escape_csv;
use crate::cli::OutputFormat;
use crate::hcp::{TfeResource, Workspace};
use comfy_table::{presets::NOTHING, Table};
use serde::Serialize;

/// Flattened workspace data for output
#[derive(Debug, Clone)]
pub struct WorkspaceRow {
    pub org: String,
    pub project_id: String,
    pub name: String,
    pub id: String,
    pub resources: u32,
    pub execution_mode: String,
    pub locked: bool,
    pub terraform_version: String,
    pub updated_at: String,
    pub pending_runs: Option<usize>,
}

impl WorkspaceRow {
    /// Create a new workspace row
    pub fn new(org: &str, workspace: &Workspace) -> Self {
        Self {
            org: org.to_string(),
            project_id: workspace.project_id().unwrap_or("").to_string(),
            name: workspace.name().to_string(),
            id: workspace.id.clone(),
            resources: workspace.resource_count(),
            execution_mode: workspace.execution_mode().to_string(),
            locked: workspace.is_locked(),
            terraform_version: workspace.terraform_version().to_string(),
            updated_at: workspace.updated_at().to_string(),
            pending_runs: None,
        }
    }
}

/// Serializable workspace for structured output (JSON/YAML)
#[derive(Serialize)]
struct SerializableWorkspace {
    org: String,
    project_id: String,
    workspace_name: String,
    workspace_id: String,
    resources: u32,
    execution_mode: String,
    locked: bool,
    terraform_version: String,
    updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pending_runs: Option<usize>,
}

impl From<&WorkspaceRow> for SerializableWorkspace {
    fn from(row: &WorkspaceRow) -> Self {
        Self {
            org: row.org.clone(),
            project_id: row.project_id.clone(),
            workspace_name: row.name.clone(),
            workspace_id: row.id.clone(),
            resources: row.resources,
            execution_mode: row.execution_mode.clone(),
            locked: row.locked,
            terraform_version: row.terraform_version.clone(),
            updated_at: row.updated_at.clone(),
            pending_runs: row.pending_runs,
        }
    }
}

/// Output workspaces in the specified format
pub fn output_workspaces(rows: &[WorkspaceRow], format: &OutputFormat, no_header: bool) {
    match format {
        OutputFormat::Table => output_table(rows, no_header),
        OutputFormat::Csv => output_csv(rows, no_header),
        OutputFormat::Json => output_json(rows),
        OutputFormat::Yaml => output_yaml(rows),
    }
}

fn output_table(rows: &[WorkspaceRow], no_header: bool) {
    let mut table = Table::new();
    table.load_preset(NOTHING);
    let show_pending = rows.iter().any(|r| r.pending_runs.is_some());
    if !no_header {
        let mut header = vec![
            "Org",
            "Project ID",
            "Workspace Name",
            "Workspace ID",
            "Resources",
            "Execution Mode",
            "Locked",
            "TF Version",
            "Updated At",
        ];
        if show_pending {
            header.push("Pending Runs");
        }
        table.set_header(header);
    }

    for ws in rows {
        let locked = if ws.locked { "Yes" } else { "No" };
        let mut row = vec![
            ws.org.clone(),
            ws.project_id.clone(),
            ws.name.clone(),
            ws.id.clone(),
            ws.resources.to_string(),
            ws.execution_mode.clone(),
            locked.to_string(),
            ws.terraform_version.clone(),
            ws.updated_at.clone(),
        ];
        if show_pending {
            row.push(ws.pending_runs.unwrap_or(0).to_string());
        }
        table.add_row(row);
    }

    println!();
    println!("{table}");
    if !no_header {
        println!("\nTotal: {} workspaces", rows.len());
    }
}

fn output_csv(rows: &[WorkspaceRow], no_header: bool) {
    let show_pending = rows.iter().any(|r| r.pending_runs.is_some());
    if !no_header {
        let mut header = "org,project_id,workspace_name,workspace_id,resources,execution_mode,locked,terraform_version,updated_at".to_string();
        if show_pending {
            header.push_str(",pending_runs");
        }
        println!("{}", header);
    }

    for ws in rows {
        let mut line = format!(
            "{},{},{},{},{},{},{},{},{}",
            escape_csv(&ws.org),
            escape_csv(&ws.project_id),
            escape_csv(&ws.name),
            escape_csv(&ws.id),
            ws.resources,
            escape_csv(&ws.execution_mode),
            ws.locked,
            escape_csv(&ws.terraform_version),
            escape_csv(&ws.updated_at)
        );
        if show_pending {
            line.push_str(&format!(",{}", ws.pending_runs.unwrap_or(0)));
        }
        println!("{}", line);
    }
}

fn output_json(rows: &[WorkspaceRow]) {
    let data: Vec<SerializableWorkspace> = rows.iter().map(SerializableWorkspace::from).collect();
    super::common::print_json(&data);
}

fn output_yaml(rows: &[WorkspaceRow]) {
    let data: Vec<SerializableWorkspace> = rows.iter().map(SerializableWorkspace::from).collect();
    super::common::print_yaml(&data);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hcp::WorkspaceAttributes;

    fn create_test_workspace() -> Workspace {
        Workspace {
            id: "ws-123".to_string(),
            attributes: WorkspaceAttributes {
                name: "test-workspace".to_string(),
                execution_mode: Some("remote".to_string()),
                resource_count: Some(42),
                locked: Some(false),
                terraform_version: Some("1.5.0".to_string()),
                updated_at: None,
            },
            relationships: None,
        }
    }

    #[test]
    fn test_workspace_row_creation() {
        let ws = create_test_workspace();
        let row = WorkspaceRow::new("my-org", &ws);

        assert_eq!(row.org, "my-org");
        assert_eq!(row.project_id, "");
        assert_eq!(row.name, "test-workspace");
        assert_eq!(row.id, "ws-123");
        assert_eq!(row.resources, 42);
        assert_eq!(row.execution_mode, "remote");
        assert!(!row.locked);
        assert_eq!(row.terraform_version, "1.5.0");
    }

    #[test]
    fn test_serializable_from_row() {
        let row = WorkspaceRow {
            org: "test-org".to_string(),
            project_id: "prj-123".to_string(),
            name: "test-ws".to_string(),
            id: "ws-123".to_string(),
            resources: 10,
            execution_mode: "remote".to_string(),
            locked: true,
            terraform_version: "1.5.0".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            pending_runs: None,
        };

        let serialized_ws = SerializableWorkspace::from(&row);
        assert_eq!(serialized_ws.org, "test-org");
        assert_eq!(serialized_ws.project_id, "prj-123");
        assert_eq!(serialized_ws.workspace_name, "test-ws");
        assert!(serialized_ws.locked);
        assert!(serialized_ws.pending_runs.is_none());
    }

    #[test]
    fn test_serializable_with_pending_runs() {
        let row = WorkspaceRow {
            org: "test-org".to_string(),
            project_id: "prj-123".to_string(),
            name: "test-ws".to_string(),
            id: "ws-123".to_string(),
            resources: 10,
            execution_mode: "remote".to_string(),
            locked: false,
            terraform_version: "1.5.0".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            pending_runs: Some(5),
        };

        let serialized_ws = SerializableWorkspace::from(&row);
        assert_eq!(serialized_ws.pending_runs, Some(5));
    }

    #[test]
    fn test_workspace_row_pending_runs_default_none() {
        let ws = create_test_workspace();
        let row = WorkspaceRow::new("my-org", &ws);
        assert!(
            row.pending_runs.is_none(),
            "pending_runs should default to None"
        );
    }

    #[test]
    fn test_serializable_pending_runs_skipped_when_none() {
        let row = WorkspaceRow {
            org: "org".to_string(),
            project_id: "prj-1".to_string(),
            name: "ws".to_string(),
            id: "ws-1".to_string(),
            resources: 0,
            execution_mode: "remote".to_string(),
            locked: false,
            terraform_version: "1.5.0".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            pending_runs: None,
        };

        let json = serde_json::to_string(&SerializableWorkspace::from(&row)).unwrap();
        assert!(
            !json.contains("pending_runs"),
            "pending_runs should be skipped in JSON when None"
        );
    }

    #[test]
    fn test_serializable_pending_runs_included_when_some() {
        let row = WorkspaceRow {
            org: "org".to_string(),
            project_id: "prj-1".to_string(),
            name: "ws".to_string(),
            id: "ws-1".to_string(),
            resources: 0,
            execution_mode: "remote".to_string(),
            locked: false,
            terraform_version: "1.5.0".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            pending_runs: Some(3),
        };

        let json = serde_json::to_string(&SerializableWorkspace::from(&row)).unwrap();
        assert!(
            json.contains("\"pending_runs\":3"),
            "pending_runs should be included in JSON when Some, got: {}",
            json
        );
    }

    #[test]
    fn test_output_workspaces_with_pending_runs_column() {
        let rows = vec![WorkspaceRow {
            org: "org".to_string(),
            project_id: "prj-1".to_string(),
            name: "ws-a".to_string(),
            id: "ws-aaa".to_string(),
            resources: 5,
            execution_mode: "remote".to_string(),
            locked: false,
            terraform_version: "1.5.0".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            pending_runs: Some(2),
        }];
        // Should not panic — table includes Pending Runs column
        output_workspaces(&rows, &OutputFormat::Table, false);
        output_workspaces(&rows, &OutputFormat::Csv, false);
    }

    #[test]
    fn test_output_workspaces_empty() {
        // Should not panic with empty input
        output_workspaces(&[], &OutputFormat::Table, false);
        output_workspaces(&[], &OutputFormat::Csv, false);
        output_workspaces(&[], &OutputFormat::Json, false);
        output_workspaces(&[], &OutputFormat::Yaml, false);
    }

    #[test]
    fn test_output_workspaces_no_header() {
        // Should not panic
        output_workspaces(&[], &OutputFormat::Table, true);
        output_workspaces(&[], &OutputFormat::Csv, true);
    }
}
