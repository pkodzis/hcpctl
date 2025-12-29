//! Helper functions for multi-organization operations
//!
//! These utilities support fetching data across multiple organizations in parallel.

use futures::future::join_all;
use indicatif::ProgressBar;
use std::future::Future;

use crate::TfeError;

/// Fetch data from multiple organizations in parallel
///
/// The `fetcher` function is called for each organization and should return
/// either the fetched data or an error tuple containing the org name and error.
pub async fn fetch_from_organizations<T, F, Fut>(
    organizations: Vec<String>,
    fetcher: F,
) -> Vec<Result<T, (String, TfeError)>>
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Result<T, (String, TfeError)>>,
{
    let futures = organizations.into_iter().map(fetcher);
    join_all(futures).await
}

/// Collect results from parallel organization fetches
///
/// Returns a tuple of (successes, had_errors). Errors are printed to stderr,
/// respecting spinner suspension if a spinner is active.
pub fn collect_org_results<T>(
    results: Vec<Result<T, (String, TfeError)>>,
    spinner: &Option<ProgressBar>,
    resource_name: &str,
) -> (Vec<T>, bool) {
    let mut successes = Vec::new();
    let mut had_errors = false;

    for result in results {
        match result {
            Ok(data) => successes.push(data),
            Err((org, e)) => {
                had_errors = true;
                let msg = format!(
                    "Error fetching {} for org '{}':\n  {}\n",
                    resource_name, org, e
                );
                if let Some(ref s) = spinner {
                    s.suspend(|| eprintln!("{}", msg));
                } else {
                    eprintln!("{}", msg);
                }
            }
        }
    }

    (successes, had_errors)
}

/// Log completion status to info log
pub fn log_completion(had_errors: bool) {
    if had_errors {
        log::info!("Completed with some errors");
    } else {
        log::info!("Completed successfully");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_org_results_all_success() {
        let results: Vec<Result<i32, (String, TfeError)>> = vec![Ok(1), Ok(2), Ok(3)];
        let (successes, had_errors) = collect_org_results(results, &None, "items");
        assert_eq!(successes, vec![1, 2, 3]);
        assert!(!had_errors);
    }

    #[test]
    fn test_collect_org_results_with_errors() {
        let results: Vec<Result<i32, (String, TfeError)>> = vec![
            Ok(1),
            Err(("org1".to_string(), TfeError::Config("test".to_string()))),
            Ok(3),
        ];
        let (successes, had_errors) = collect_org_results(results, &None, "items");
        assert_eq!(successes, vec![1, 3]);
        assert!(had_errors);
    }
}
