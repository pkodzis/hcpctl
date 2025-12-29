//! Output formatting module
//!
//! Handles different output formats: table, CSV, JSON

mod csv;
mod json;
mod table;

use crate::cli::{OutputFormat, SortField};
use crate::hcp::Workspace;

pub use self::csv::CsvFormatter;
pub use self::json::JsonFormatter;
pub use self::table::TableFormatter;

/// Trait for output formatters
pub trait Formatter {
    /// Format and print the workspaces
    fn format(&self, workspaces: &[WorkspaceRow]);
}

/// Flattened workspace data for output
#[derive(Debug, Clone)]
pub struct WorkspaceRow {
    pub org: String,
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

/// Sort options for output
pub struct SortOptions {
    pub field: SortField,
    pub reverse: bool,
    pub group_by_org: bool,
}

impl Default for SortOptions {
    fn default() -> Self {
        Self {
            field: SortField::Name,
            reverse: false,
            group_by_org: true,
        }
    }
}

/// Compare two rows by the specified field
fn compare_rows(a: &WorkspaceRow, b: &WorkspaceRow, field: &SortField) -> std::cmp::Ordering {
    match field {
        SortField::Name => a.name.cmp(&b.name),
        SortField::Resources => a.resources.cmp(&b.resources),
        SortField::UpdatedAt => a.updated_at.cmp(&b.updated_at),
        SortField::TfVersion => compare_versions(&a.terraform_version, &b.terraform_version),
    }
}

/// Compare semantic versions (handles "unknown" and partial versions)
fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    // Handle "unknown" - sort to end
    if a == "unknown" && b == "unknown" {
        return std::cmp::Ordering::Equal;
    }
    if a == "unknown" {
        return std::cmp::Ordering::Greater;
    }
    if b == "unknown" {
        return std::cmp::Ordering::Less;
    }

    // Parse version parts
    let parse_parts =
        |v: &str| -> Vec<u32> { v.split('.').filter_map(|p| p.parse::<u32>().ok()).collect() };

    let a_parts = parse_parts(a);
    let b_parts = parse_parts(b);

    // Compare part by part
    for (ap, bp) in a_parts.iter().zip(b_parts.iter()) {
        match ap.cmp(bp) {
            std::cmp::Ordering::Equal => continue,
            other => return other,
        }
    }

    // If all compared parts are equal, longer version is greater
    a_parts.len().cmp(&b_parts.len())
}

/// Sort rows according to options
fn sort_rows(rows: &mut [WorkspaceRow], options: &SortOptions) {
    if options.group_by_org {
        // Sort within each org group
        // First, sort by org name, then by the specified field within each org
        rows.sort_by(|a, b| {
            let org_cmp = a.org.cmp(&b.org);
            if org_cmp != std::cmp::Ordering::Equal {
                return org_cmp;
            }
            let field_cmp = compare_rows(a, b, &options.field);
            if options.reverse {
                field_cmp.reverse()
            } else {
                field_cmp
            }
        });
    } else {
        // Sort all rows together
        rows.sort_by(|a, b| {
            let cmp = compare_rows(a, b, &options.field);
            if options.reverse {
                cmp.reverse()
            } else {
                cmp
            }
        });
    }
}

/// Output results using the specified format with sorting options
pub fn output_results_sorted(
    workspaces: Vec<(String, Vec<Workspace>)>,
    format: &OutputFormat,
    sort_options: &SortOptions,
) {
    // Flatten the data structure
    let mut rows: Vec<WorkspaceRow> = workspaces
        .iter()
        .flat_map(|(org, ws_list)| {
            ws_list
                .iter()
                .map(|ws| WorkspaceRow::new(org, ws))
                .collect::<Vec<_>>()
        })
        .collect();

    // Sort rows
    sort_rows(&mut rows, sort_options);

    match format {
        OutputFormat::Table => TableFormatter.format(&rows),
        OutputFormat::Csv => CsvFormatter.format(&rows),
        OutputFormat::Json => JsonFormatter.format(&rows),
    }
}

/// Output results using the specified format (default sorting)
pub fn output_results(workspaces: Vec<(String, Vec<Workspace>)>, format: &OutputFormat) {
    output_results_sorted(workspaces, format, &SortOptions::default())
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
        }
    }

    #[test]
    fn test_workspace_row_creation() {
        let ws = create_test_workspace();
        let row = WorkspaceRow::new("my-org", &ws);

        assert_eq!(row.org, "my-org");
        assert_eq!(row.name, "test-workspace");
        assert_eq!(row.id, "ws-123");
        assert_eq!(row.resources, 42);
        assert_eq!(row.execution_mode, "remote");
        assert!(!row.locked);
        assert_eq!(row.terraform_version, "1.5.0");
        assert_eq!(row.updated_at, "");
    }

    #[test]
    fn test_compare_versions() {
        use std::cmp::Ordering;

        assert_eq!(compare_versions("1.5.0", "1.5.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.5.0", "1.6.0"), Ordering::Less);
        assert_eq!(compare_versions("1.10.0", "1.9.0"), Ordering::Greater);
        assert_eq!(compare_versions("2.0.0", "1.99.99"), Ordering::Greater);
        assert_eq!(compare_versions("unknown", "1.5.0"), Ordering::Greater);
        assert_eq!(compare_versions("1.5.0", "unknown"), Ordering::Less);
        assert_eq!(compare_versions("unknown", "unknown"), Ordering::Equal);
    }

    #[test]
    fn test_sort_by_name() {
        let mut rows = vec![
            WorkspaceRow {
                org: "org1".to_string(),
                name: "zeta".to_string(),
                id: "ws-1".to_string(),
                resources: 10,
                execution_mode: "remote".to_string(),
                locked: false,
                terraform_version: "1.5.0".to_string(),
                updated_at: "".to_string(),
            },
            WorkspaceRow {
                org: "org1".to_string(),
                name: "alpha".to_string(),
                id: "ws-2".to_string(),
                resources: 20,
                execution_mode: "remote".to_string(),
                locked: false,
                terraform_version: "1.6.0".to_string(),
                updated_at: "".to_string(),
            },
        ];

        let options = SortOptions {
            field: SortField::Name,
            reverse: false,
            group_by_org: false,
        };

        sort_rows(&mut rows, &options);

        assert_eq!(rows[0].name, "alpha");
        assert_eq!(rows[1].name, "zeta");
    }

    #[test]
    fn test_sort_by_resources_reverse() {
        let mut rows = vec![
            WorkspaceRow {
                org: "org1".to_string(),
                name: "ws1".to_string(),
                id: "ws-1".to_string(),
                resources: 10,
                execution_mode: "remote".to_string(),
                locked: false,
                terraform_version: "1.5.0".to_string(),
                updated_at: "".to_string(),
            },
            WorkspaceRow {
                org: "org1".to_string(),
                name: "ws2".to_string(),
                id: "ws-2".to_string(),
                resources: 100,
                execution_mode: "remote".to_string(),
                locked: false,
                terraform_version: "1.6.0".to_string(),
                updated_at: "".to_string(),
            },
        ];

        let options = SortOptions {
            field: SortField::Resources,
            reverse: true,
            group_by_org: false,
        };

        sort_rows(&mut rows, &options);

        assert_eq!(rows[0].resources, 100);
        assert_eq!(rows[1].resources, 10);
    }

    #[test]
    fn test_sort_group_by_org() {
        let mut rows = vec![
            WorkspaceRow {
                org: "org-b".to_string(),
                name: "ws1".to_string(),
                id: "ws-1".to_string(),
                resources: 10,
                execution_mode: "remote".to_string(),
                locked: false,
                terraform_version: "1.5.0".to_string(),
                updated_at: "".to_string(),
            },
            WorkspaceRow {
                org: "org-a".to_string(),
                name: "ws2".to_string(),
                id: "ws-2".to_string(),
                resources: 100,
                execution_mode: "remote".to_string(),
                locked: false,
                terraform_version: "1.6.0".to_string(),
                updated_at: "".to_string(),
            },
        ];

        let options = SortOptions {
            field: SortField::Name,
            reverse: false,
            group_by_org: true,
        };

        sort_rows(&mut rows, &options);

        // org-a should come first
        assert_eq!(rows[0].org, "org-a");
        assert_eq!(rows[1].org, "org-b");
    }
}
