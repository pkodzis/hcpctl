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

/// Finish spinner - clears it completely without leaving a message
///
/// The spinner disappears without leaving any trace in the terminal.
pub fn finish_spinner(spinner: Option<ProgressBar>) {
    if let Some(s) = spinner {
        s.finish_and_clear();
    }
}

/// Finish spinner - clears it completely
///
/// Always clears the spinner without leaving any trace.
pub fn finish_spinner_with_status<T>(
    spinner: Option<ProgressBar>,
    _results: &[T],
    _had_errors: bool,
) {
    if let Some(s) = spinner {
        s.finish_and_clear();
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
        finish_spinner(None);
    }

    #[test]
    fn test_finish_spinner_with_status_none() {
        // Should not panic
        let results: Vec<i32> = vec![];
        finish_spinner_with_status(None, &results, false);
    }
}
