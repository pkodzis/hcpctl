//! Log parsing and formatting utilities
//!
//! Shared functionality for processing TFE run logs across commands:
//! - `logs` command
//! - `watch ws` command
//! - `get run --subresource plan/apply`

use std::io::{self, Write};

/// Extract @message from JSON log line or return line as-is
///
/// TFE logs often contain JSON lines with structured data.
/// This function extracts the human-readable message.
///
/// # Arguments
/// * `line` - A single log line (may be JSON or plain text)
///
/// # Returns
/// * For JSON lines with @message: the message content
/// * For JSON lines without @message: empty string (to skip)
/// * For non-JSON lines: the original line
pub fn extract_log_message(line: &str) -> String {
    if line.starts_with('{') {
        // Try to parse as JSON and extract @message
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(message) = json.get("@message").and_then(|m| m.as_str()) {
                return message.to_string();
            }
            // JSON without @message - skip
            return String::new();
        }
    }
    // Non-JSON or invalid JSON - return as-is
    line.to_string()
}

/// Print log content with human-readable formatting
///
/// For lines starting with '{', tries to parse as JSON and extract @message.
/// For other lines (headers, plain text), prints them as-is.
/// Used by `tail_log` and log output functions.
pub fn print_human_readable_log(content: &str) {
    for line in content.lines() {
        let message = extract_log_message(line);
        if !message.is_empty() {
            println!("{}", message);
        }
    }
}

/// Print log content with optional run ID prefix
///
/// Used by `watch ws` command to distinguish logs from different runs.
///
/// # Arguments
/// * `content` - Log content (may contain multiple lines)
/// * `prefix` - Optional prefix to prepend to each line (e.g., run ID)
/// * `raw` - If true, output raw content; if false, extract @message from JSON
pub fn print_log_with_prefix(content: &str, prefix: Option<&str>, raw: bool) {
    for line in content.lines() {
        let message = if raw {
            line.to_string()
        } else {
            extract_log_message(line)
        };

        // Skip empty messages from JSON parsing
        if message.is_empty() {
            continue;
        }

        match prefix {
            Some(p) => println!("[{}] {}", p, message),
            None => println!("{}", message),
        }
    }
    io::stdout().flush().ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_log_message_json_with_message() {
        let line = r#"{"@level":"info","@message":"Plan: 1 to add","type":"planned_change"}"#;
        assert_eq!(extract_log_message(line), "Plan: 1 to add");
    }

    #[test]
    fn test_extract_log_message_json_without_message() {
        let line = r#"{"@level":"info","type":"version"}"#;
        assert_eq!(extract_log_message(line), "");
    }

    #[test]
    fn test_extract_log_message_plain_text() {
        let line = "Terraform v1.12.2";
        assert_eq!(extract_log_message(line), "Terraform v1.12.2");
    }

    #[test]
    fn test_extract_log_message_invalid_json() {
        let line = "{invalid json}";
        assert_eq!(extract_log_message(line), "{invalid json}");
    }

    #[test]
    fn test_extract_log_message_empty_line() {
        assert_eq!(extract_log_message(""), "");
    }

    #[test]
    fn test_print_human_readable_log_mixed() {
        // This should not panic - just verify it runs
        let log = r#"Header line
{"@message":"First message"}
Plain text
{"@message":"Second message"}
{"no_message":"skipped"}
Footer
"#;
        print_human_readable_log(log);
    }

    #[test]
    fn test_print_log_with_prefix_no_prefix() {
        // Should not panic
        print_log_with_prefix("Test line\nAnother line", None, true);
    }

    #[test]
    fn test_print_log_with_prefix_with_prefix() {
        // Should not panic
        print_log_with_prefix("Test line\nAnother line", Some("run-123"), true);
    }

    #[test]
    fn test_print_log_with_prefix_parsed() {
        let content = r#"{"@message":"Hello world"}
Plain text"#;
        // Should not panic, parsed mode
        print_log_with_prefix(content, Some("run-123"), false);
    }
}
