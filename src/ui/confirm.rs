//! User confirmation prompts for potentially dangerous operations

use std::io::{self, Write};

use crate::config::api;
use crate::hcp::AggregatedPaginationInfo;

/// Information about a large pagination operation
#[derive(Debug, Clone)]
pub struct LargePaginationInfo {
    /// Total number of items to fetch
    pub total_count: u32,
    /// Total number of pages
    pub total_pages: u32,
    /// Number of API calls required (with parallel fetching)
    pub api_calls: u32,
    /// Context description (e.g., "workspaces for organization 'my-org'")
    pub context: String,
    /// Number of organizations involved
    pub org_count: usize,
}

impl LargePaginationInfo {
    /// Create new info from pagination metadata
    pub fn new(total_count: u32, total_pages: u32, context: &str) -> Self {
        Self {
            total_count,
            total_pages,
            api_calls: total_pages,
            context: context.to_string(),
            org_count: 1,
        }
    }

    /// Create from aggregated pagination info across multiple organizations
    pub fn from_aggregated(agg: &AggregatedPaginationInfo, resource_name: &str) -> Self {
        Self {
            total_count: agg.total_count,
            total_pages: agg.estimated_api_calls,
            api_calls: agg.estimated_api_calls,
            context: format!("{} across {} organization(s)", resource_name, agg.org_count),
            org_count: agg.org_count,
        }
    }

    /// Check if this operation exceeds the warning threshold
    pub fn exceeds_threshold(&self) -> bool {
        self.total_count > api::LARGE_RESULT_THRESHOLD
    }

    /// Estimated time in seconds (rough approximation)
    /// Assumes ~300ms per request with parallelism
    pub fn estimated_seconds(&self) -> u32 {
        let batches =
            (self.total_pages as f64 / api::MAX_CONCURRENT_PAGE_REQUESTS as f64).ceil() as u32;
        // ~300ms per batch
        (batches as f64 * 0.3).ceil() as u32 + 1
    }
}

/// Prompt user to confirm a large pagination operation
///
/// Returns `true` if user confirms, `false` if user declines.
/// In batch mode, always returns `false` (fails safe).
pub fn confirm_large_pagination(info: &LargePaginationInfo, batch_mode: bool) -> bool {
    if batch_mode {
        eprintln!(
            "\nWARNING: LARGE RESULT SET DETECTED - Operation aborted in batch mode\n\
             \n\
             This query would fetch {} items across {} API calls.\n\
             Context: {}\n\
             \n\
             In batch mode, large operations are automatically declined to prevent\n\
             accidental DoS attacks on your TFE/HCP instance.\n\
             \n\
             To proceed, either:\n\
             - Run interactively (without --batch flag)\n\
             - Use filters to reduce the result set (--filter, --org, --prj)\n",
            info.total_count, info.api_calls, info.context
        );
        return false;
    }

    // Interactive mode - show warning and prompt
    eprintln!(
        "\n\x1b[1;33mWARNING: LARGE RESULT SET DETECTED\x1b[0m\n\
         \n\
         \x1b[1mThis operation may impact TFE/HCP performance!\x1b[0m\n\
         \n\
         ┌─────────────────────────────────────────────────────────────┐\n\
         │  Scale of operation:                                        │\n\
         │     - Total items to fetch: \x1b[1;36m{:>8}\x1b[0m                        │\n\
         │     - API calls required:   \x1b[1;36m{:>8}\x1b[0m                        │\n\
         │     - Estimated time:       \x1b[1;36m{:>5} sec\x1b[0m                       │\n\
         ├─────────────────────────────────────────────────────────────┤\n\
         │  Context: {:<48}  │\n\
         ├─────────────────────────────────────────────────────────────┤\n\
         │  Impact:                                                    │\n\
         │     - May trigger rate limiting (429 errors)                │\n\
         │     - Can slow down TFE for other users                     │\n\
         │     - Consider using filters to reduce scope                │\n\
         └─────────────────────────────────────────────────────────────┘\n\
         \n\
         \x1b[1mRecommended filters:\x1b[0m\n\
         - --org <name>     Limit to specific organization\n\
         - --filter <term>  Filter by name (server-side)\n\
         - --prj <name>     Filter by project (requires --org)\n",
        info.total_count,
        info.api_calls,
        info.estimated_seconds(),
        truncate_context(&info.context, 48),
    );

    eprint!("\n\x1b[1;33mProceed with this operation? [y/N]:\x1b[0m ");
    let _ = io::stderr().flush();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return false;
    }

    let answer = input.trim().to_lowercase();
    matches!(answer.as_str(), "y" | "yes")
}

/// Truncate context string for display, adding ellipsis if needed
fn truncate_context(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_large_pagination_info_new() {
        let info = LargePaginationInfo::new(5000, 50, "workspaces");
        assert_eq!(info.total_count, 5000);
        assert_eq!(info.total_pages, 50);
        assert_eq!(info.api_calls, 50);
        assert_eq!(info.context, "workspaces");
        assert_eq!(info.org_count, 1);
    }

    #[test]
    fn test_from_aggregated() {
        let agg = AggregatedPaginationInfo {
            total_count: 3000,
            org_count: 5,
            estimated_api_calls: 30,
        };
        let info = LargePaginationInfo::from_aggregated(&agg, "workspaces");
        assert_eq!(info.total_count, 3000);
        assert_eq!(info.api_calls, 30);
        assert_eq!(info.org_count, 5);
        assert!(info.context.contains("workspaces"));
        assert!(info.context.contains("5 organization"));
    }

    #[test]
    fn test_exceeds_threshold_true() {
        let info = LargePaginationInfo::new(1500, 15, "test");
        assert!(info.exceeds_threshold());
    }

    #[test]
    fn test_exceeds_threshold_false() {
        let info = LargePaginationInfo::new(500, 5, "test");
        assert!(!info.exceeds_threshold());
    }

    #[test]
    fn test_exceeds_threshold_boundary() {
        let info = LargePaginationInfo::new(1000, 10, "test");
        assert!(!info.exceeds_threshold()); // 1000 is NOT greater than 1000

        let info = LargePaginationInfo::new(1001, 11, "test");
        assert!(info.exceeds_threshold());
    }

    #[test]
    fn test_estimated_seconds() {
        // 10 pages with max 10 concurrent = 1 batch = ~1 second
        let info = LargePaginationInfo::new(1000, 10, "test");
        assert!(info.estimated_seconds() >= 1);

        // 100 pages with max 10 concurrent = 10 batches = ~3-4 seconds
        let info = LargePaginationInfo::new(10000, 100, "test");
        assert!(info.estimated_seconds() >= 3);
    }

    #[test]
    fn test_truncate_context_short() {
        assert_eq!(truncate_context("short", 45), "short");
    }

    #[test]
    fn test_truncate_context_long() {
        let long = "this is a very long context string that exceeds the maximum length";
        let truncated = truncate_context(long, 20);
        assert_eq!(truncated.len(), 20);
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn test_confirm_large_pagination_batch_mode() {
        let info = LargePaginationInfo::new(5000, 50, "test");
        // In batch mode, should always return false
        assert!(!confirm_large_pagination(&info, true));
    }
}
