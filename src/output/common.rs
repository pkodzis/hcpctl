//! Common utilities for output formatters

/// Escape a value for CSV output
/// Handles commas, quotes, and newlines according to RFC 4180
pub fn escape_csv(value: &str) -> String {
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
    fn test_escape_csv_empty() {
        assert_eq!(escape_csv(""), "");
    }

    #[test]
    fn test_escape_csv_multiple_special() {
        assert_eq!(escape_csv("a,b\"c\nd"), "\"a,b\"\"c\nd\"");
    }
}
