//! Progress spinner utilities

use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

/// Create a spinner with the given message
///
/// Returns `None` if quiet mode is enabled.
pub fn create_spinner(message: &str, quiet: bool) -> Option<ProgressBar> {
    if quiet {
        return None;
    }
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner:.blue} {msg}")
            .unwrap(),
    );
    spinner.set_message(message.to_string());
    spinner.enable_steady_tick(Duration::from_millis(100));
    Some(spinner)
}

/// Finish spinner with a message
pub fn finish_spinner(spinner: Option<ProgressBar>, message: &str) {
    if let Some(s) = spinner {
        s.finish_with_message(message.to_string());
    }
}

/// Finish spinner with appropriate message based on results
pub fn finish_spinner_with_status<T>(
    spinner: Option<ProgressBar>,
    results: &[T],
    had_errors: bool,
) {
    if let Some(s) = spinner {
        if had_errors && results.is_empty() {
            s.finish_and_clear();
        } else if had_errors {
            s.finish_with_message("Completed with errors");
        } else {
            s.finish_with_message("Done");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_spinner_quiet_mode() {
        assert!(create_spinner("test", true).is_none());
    }

    #[test]
    fn test_finish_spinner_none() {
        // Should not panic
        finish_spinner(None, "Done");
    }

    #[test]
    fn test_finish_spinner_with_status_none() {
        // Should not panic
        let results: Vec<i32> = vec![];
        finish_spinner_with_status(None, &results, false);
    }
}
