//! JSON output formatter

use serde::Serialize;

use super::{Formatter, WorkspaceRow};

/// Formatter for JSON output
pub struct JsonFormatter;

/// Serializable workspace for JSON output
#[derive(Serialize)]
struct JsonWorkspace {
    org: String,
    workspace_name: String,
    workspace_id: String,
    resources: u32,
    execution_mode: String,
    locked: bool,
    terraform_version: String,
    updated_at: String,
}

impl From<&WorkspaceRow> for JsonWorkspace {
    fn from(row: &WorkspaceRow) -> Self {
        Self {
            org: row.org.clone(),
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

impl Formatter for JsonFormatter {
    fn format(&self, workspaces: &[WorkspaceRow]) {
        let json_workspaces: Vec<JsonWorkspace> =
            workspaces.iter().map(JsonWorkspace::from).collect();

        match serde_json::to_string_pretty(&json_workspaces) {
            Ok(json) => println!("{}", json),
            Err(e) => eprintln!("Error serializing to JSON: {}", e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_workspace_from_row() {
        let row = WorkspaceRow {
            org: "test-org".to_string(),
            name: "test-ws".to_string(),
            id: "ws-123".to_string(),
            resources: 10,
            execution_mode: "remote".to_string(),
            locked: true,
            terraform_version: "1.5.0".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };

        let json_ws = JsonWorkspace::from(&row);
        assert_eq!(json_ws.org, "test-org");
        assert_eq!(json_ws.workspace_name, "test-ws");
        assert!(json_ws.locked);
        assert_eq!(json_ws.terraform_version, "1.5.0");
    }

    #[test]
    fn test_json_formatter_empty() {
        let formatter = JsonFormatter;
        // Should not panic with empty input
        formatter.format(&[]);
    }

    #[test]
    fn test_json_serialization() {
        let rows = [WorkspaceRow {
            org: "test-org".to_string(),
            name: "test-ws".to_string(),
            id: "ws-123".to_string(),
            resources: 10,
            execution_mode: "remote".to_string(),
            locked: false,
            terraform_version: "1.6.0".to_string(),
            updated_at: "".to_string(),
        }];

        let json_workspaces: Vec<JsonWorkspace> = rows.iter().map(JsonWorkspace::from).collect();
        let result = serde_json::to_string(&json_workspaces);
        assert!(result.is_ok());

        let json = result.unwrap();
        assert!(json.contains("test-org"));
        assert!(json.contains("test-ws"));
    }
}
