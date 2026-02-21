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
    if !no_header {
        table.set_header(vec![
            "Org",
            "Project ID",
            "Workspace Name",
            "Workspace ID",
            "Resources",
            "Execution Mode",
            "Locked",
            "TF Version",
            "Updated At",
        ]);
    }

    for ws in rows {
        let locked = if ws.locked { "Yes" } else { "No" };
        table.add_row(vec![
            &ws.org,
            &ws.project_id,
            &ws.name,
            &ws.id,
            &ws.resources.to_string(),
            &ws.execution_mode,
            locked,
            &ws.terraform_version,
            &ws.updated_at,
        ]);
    }

    println!();
    println!("{table}");
    if !no_header {
        println!("\nTotal: {} workspaces", rows.len());
    }
}

fn output_csv(rows: &[WorkspaceRow], no_header: bool) {
    if !no_header {
        println!("org,project_id,workspace_name,workspace_id,resources,execution_mode,locked,terraform_version,updated_at");
    }

    for ws in rows {
        println!(
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
        };

        let serialized_ws = SerializableWorkspace::from(&row);
        assert_eq!(serialized_ws.org, "test-org");
        assert_eq!(serialized_ws.project_id, "prj-123");
        assert_eq!(serialized_ws.workspace_name, "test-ws");
        assert!(serialized_ws.locked);
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
