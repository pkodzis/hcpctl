//! CSV output formatter

use super::{Formatter, WorkspaceRow};

/// Formatter for CSV output
pub struct CsvFormatter;

impl Formatter for CsvFormatter {
    fn format(&self, workspaces: &[WorkspaceRow]) {
        // Print header
        println!("org,workspace_name,workspace_id,resources,execution_mode,locked,terraform_version,updated_at");

        // Print rows
        for ws in workspaces {
            println!(
                "{},{},{},{},{},{},{},{}",
                escape_csv(&ws.org),
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
}

/// Escape a value for CSV output
fn escape_csv(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_csv_simple() {
        assert_eq!(escape_csv("simple"), "simple");
    }

    #[test]
    fn test_escape_csv_with_comma() {
        assert_eq!(escape_csv("has,comma"), "\"has,comma\"");
    }

    #[test]
    fn test_escape_csv_with_quotes() {
        assert_eq!(escape_csv("has\"quote"), "\"has\"\"quote\"");
    }

    #[test]
    fn test_escape_csv_with_newline() {
        assert_eq!(escape_csv("has\nnewline"), "\"has\nnewline\"");
    }

    #[test]
    fn test_csv_formatter_empty() {
        let formatter = CsvFormatter;
        // Should not panic with empty input
        formatter.format(&[]);
    }
}
