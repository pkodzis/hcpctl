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

/// Aggregated pagination info across multiple organizations
#[derive(Debug, Clone)]
pub struct AggregatedPaginationInfo {
    /// Total items across all organizations
    pub total_count: u32,
    /// Number of organizations with data
    pub org_count: usize,
    /// Estimated number of API calls needed
    pub estimated_api_calls: u32,
}

/// Aggregate pagination info from multiple results
///
/// Takes pagination info from multiple organizations and returns aggregate stats.
/// Ignores None values (orgs with no pagination info).
pub fn aggregate_pagination_info(
    results: Vec<Option<crate::hcp::PaginationInfo>>,
) -> AggregatedPaginationInfo {
    let mut total_count: u32 = 0;
    let mut org_count: usize = 0;
    let mut estimated_api_calls: u32 = 0;

    for info in results.into_iter().flatten() {
        total_count += info.total_count;
        org_count += 1;
        estimated_api_calls += info.total_pages;
    }

    AggregatedPaginationInfo {
        total_count,
        org_count,
        estimated_api_calls,
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

    #[test]
    fn test_collect_org_results_empty() {
        let results: Vec<Result<i32, (String, TfeError)>> = vec![];
        let (successes, had_errors) = collect_org_results(results, &None, "workspaces");
        assert!(successes.is_empty());
        assert!(!had_errors);
    }

    #[test]
    fn test_collect_org_results_all_errors() {
        let results: Vec<Result<i32, (String, TfeError)>> = vec![
            Err(("org1".to_string(), TfeError::Config("error1".to_string()))),
            Err(("org2".to_string(), TfeError::Config("error2".to_string()))),
        ];
        let (successes, had_errors) = collect_org_results(results, &None, "projects");
        assert!(successes.is_empty());
        assert!(had_errors);
    }

    #[test]
    fn test_collect_org_results_with_complex_type() {
        #[allow(clippy::type_complexity)]
        let results: Vec<Result<(String, Vec<u32>), (String, TfeError)>> = vec![
            Ok(("org1".to_string(), vec![1, 2])),
            Ok(("org2".to_string(), vec![3, 4, 5])),
        ];
        let (successes, had_errors) = collect_org_results(results, &None, "data");
        assert_eq!(successes.len(), 2);
        assert_eq!(successes[0].1.len(), 2);
        assert_eq!(successes[1].1.len(), 3);
        assert!(!had_errors);
    }

    #[tokio::test]
    async fn test_fetch_from_organizations() {
        let orgs = vec!["org1".to_string(), "org2".to_string()];
        let results = fetch_from_organizations(orgs, |org| async move {
            Ok::<_, (String, TfeError)>(format!("result-{}", org))
        })
        .await;

        assert_eq!(results.len(), 2);
        assert!(results[0].is_ok());
        assert!(results[1].is_ok());
        assert_eq!(results[0].as_ref().unwrap(), "result-org1");
        assert_eq!(results[1].as_ref().unwrap(), "result-org2");
    }

    #[tokio::test]
    async fn test_fetch_from_organizations_with_error() {
        let orgs = vec!["org1".to_string(), "fail".to_string()];
        let results = fetch_from_organizations(orgs, |org| async move {
            if org == "fail" {
                Err((org, TfeError::Config("simulated error".to_string())))
            } else {
                Ok::<_, (String, TfeError)>(format!("result-{}", org))
            }
        })
        .await;

        assert_eq!(results.len(), 2);
        assert!(results[0].is_ok());
        assert!(results[1].is_err());
    }

    #[tokio::test]
    async fn test_fetch_from_organizations_empty() {
        let orgs: Vec<String> = vec![];
        let results =
            fetch_from_organizations(orgs, |org| async move { Ok::<_, (String, TfeError)>(org) })
                .await;

        assert!(results.is_empty());
    }

    #[test]
    fn test_aggregate_pagination_info_multiple_orgs() {
        let results = vec![
            Some(crate::hcp::PaginationInfo {
                total_count: 1000,
                total_pages: 10,
            }),
            Some(crate::hcp::PaginationInfo {
                total_count: 2000,
                total_pages: 20,
            }),
            Some(crate::hcp::PaginationInfo {
                total_count: 500,
                total_pages: 5,
            }),
        ];

        let agg = aggregate_pagination_info(results);
        assert_eq!(agg.total_count, 3500);
        assert_eq!(agg.org_count, 3);
        assert_eq!(agg.estimated_api_calls, 35);
    }

    #[test]
    fn test_aggregate_pagination_info_with_nones() {
        let results = vec![
            Some(crate::hcp::PaginationInfo {
                total_count: 1000,
                total_pages: 10,
            }),
            None,
            Some(crate::hcp::PaginationInfo {
                total_count: 500,
                total_pages: 5,
            }),
            None,
        ];

        let agg = aggregate_pagination_info(results);
        assert_eq!(agg.total_count, 1500);
        assert_eq!(agg.org_count, 2);
        assert_eq!(agg.estimated_api_calls, 15);
    }

    #[test]
    fn test_aggregate_pagination_info_empty() {
        let results: Vec<Option<crate::hcp::PaginationInfo>> = vec![];
        let agg = aggregate_pagination_info(results);
        assert_eq!(agg.total_count, 0);
        assert_eq!(agg.org_count, 0);
        assert_eq!(agg.estimated_api_calls, 0);
    }

    #[test]
    fn test_aggregate_pagination_info_all_nones() {
        let results = vec![None, None, None];
        let agg = aggregate_pagination_info(results);
        assert_eq!(agg.total_count, 0);
        assert_eq!(agg.org_count, 0);
        assert_eq!(agg.estimated_api_calls, 0);
    }
}
