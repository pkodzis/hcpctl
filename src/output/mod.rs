//! Output formatting module for all resources (organizations, projects, workspaces, oauth clients, runs, teams)

mod common;
mod oauth_clients;
pub mod org_memberships;
mod organizations;
mod projects;
mod runs;
mod tags;
mod teams;
mod workspaces;

pub use common::{escape_csv, output_raw};
pub use oauth_clients::output_oauth_clients;
pub use organizations::output_organizations;
pub use projects::output_projects;
pub use runs::{output_apply, output_plan, output_run_events, output_runs};
pub use tags::{
    output_org_tags, output_org_tags_with_workspaces, output_tag_bindings,
    output_workspace_all_tags,
};
pub use teams::output_teams;
pub use workspaces::WorkspaceRow;

use workspaces::output_workspaces;

use crate::cli::{Cli, Command, GetResource, WsSortField};
use crate::hcp::Workspace;

/// Main entry point for sorted workspace output - converts raw data to WorkspaceRow and outputs
pub fn output_results_sorted(org_workspaces: Vec<(String, Vec<Workspace>)>, cli: &Cli) {
    let Command::Get {
        resource: GetResource::Ws(args),
    } = &cli.command
    else {
        unreachable!()
    };

    // Convert to WorkspaceRow using the constructor
    let mut rows: Vec<WorkspaceRow> = org_workspaces
        .into_iter()
        .flat_map(|(org, workspaces)| {
            workspaces
                .iter()
                .map(move |ws| WorkspaceRow::new(&org, ws))
                .collect::<Vec<_>>()
        })
        .collect();

    // Sort
    rows.sort_by(|a, b| {
        use std::cmp::Ordering;

        // Group by org first if enabled
        if args.group_by_org() {
            let org_cmp = a.org.cmp(&b.org);
            if org_cmp != Ordering::Equal {
                return org_cmp;
            }
        }

        // Then group by project if enabled
        if args.group_by_prj {
            let prj_cmp = a.project_id.cmp(&b.project_id);
            if prj_cmp != Ordering::Equal {
                return prj_cmp;
            }
        }

        // Then sort by selected field
        match args.sort {
            WsSortField::Name => a.name.cmp(&b.name),
            WsSortField::Resources => a.resources.cmp(&b.resources),
            WsSortField::UpdatedAt => a.updated_at.cmp(&b.updated_at),
            WsSortField::TfVersion => compare_versions(&a.terraform_version, &b.terraform_version),
        }
    });

    if args.reverse {
        rows.reverse();
    }

    output_workspaces(&rows, &args.output, cli.no_header);
}

/// Compare semantic versions (handles "unknown" and partial versions)
fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    // Handle "unknown" - sort to end
    if a == "unknown" && b == "unknown" {
        return Ordering::Equal;
    }
    if a == "unknown" {
        return Ordering::Greater;
    }
    if b == "unknown" {
        return Ordering::Less;
    }

    // Parse version parts
    let parse_parts =
        |v: &str| -> Vec<u32> { v.split('.').filter_map(|p| p.parse::<u32>().ok()).collect() };

    let a_parts = parse_parts(a);
    let b_parts = parse_parts(b);

    // Compare part by part
    for (ap, bp) in a_parts.iter().zip(b_parts.iter()) {
        match ap.cmp(bp) {
            Ordering::Equal => continue,
            other => return other,
        }
    }

    // If all compared parts are equal, longer version is greater
    a_parts.len().cmp(&b_parts.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn test_compare_versions_equal() {
        assert_eq!(compare_versions("1.5.0", "1.5.0"), Ordering::Equal);
    }

    #[test]
    fn test_compare_versions_less() {
        assert_eq!(compare_versions("1.5.0", "1.6.0"), Ordering::Less);
    }

    #[test]
    fn test_compare_versions_greater() {
        assert_eq!(compare_versions("1.10.0", "1.9.0"), Ordering::Greater);
    }

    #[test]
    fn test_compare_versions_major() {
        assert_eq!(compare_versions("2.0.0", "1.99.99"), Ordering::Greater);
    }

    #[test]
    fn test_compare_versions_unknown_last() {
        assert_eq!(compare_versions("unknown", "1.5.0"), Ordering::Greater);
        assert_eq!(compare_versions("1.5.0", "unknown"), Ordering::Less);
    }

    #[test]
    fn test_compare_versions_both_unknown() {
        assert_eq!(compare_versions("unknown", "unknown"), Ordering::Equal);
    }
}
