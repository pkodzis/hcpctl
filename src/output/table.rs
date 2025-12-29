//! Table output formatter

use comfy_table::{presets::NOTHING, Table};

use super::{Formatter, WorkspaceRow};

/// Formatter for ASCII table output
pub struct TableFormatter;

impl Formatter for TableFormatter {
    fn format(&self, workspaces: &[WorkspaceRow]) {
        let mut table = Table::new();
        table.load_preset(NOTHING).set_header(vec![
            "Org",
            "Workspace Name",
            "Workspace ID",
            "Resources",
            "Execution Mode",
            "Locked",
            "TF Version",
            "Updated At",
        ]);

        for ws in workspaces {
            let locked = if ws.locked { "Yes" } else { "No" };
            table.add_row(vec![
                &ws.org,
                &ws.name,
                &ws.id,
                &ws.resources.to_string(),
                &ws.execution_mode,
                locked,
                &ws.terraform_version,
                &ws.updated_at,
            ]);
        }

        println!("{}", table);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_formatter_empty() {
        let formatter = TableFormatter;
        // Should not panic with empty input
        formatter.format(&[]);
    }

    #[test]
    fn test_table_formatter_with_data() {
        let rows = vec![WorkspaceRow {
            org: "test-org".to_string(),
            name: "test-ws".to_string(),
            id: "ws-123".to_string(),
            resources: 10,
            execution_mode: "remote".to_string(),
            locked: false,
            terraform_version: "1.5.0".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        }];

        let formatter = TableFormatter;
        // Should not panic
        formatter.format(&rows);
    }
}
