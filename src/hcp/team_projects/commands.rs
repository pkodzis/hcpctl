//! Team project access command handlers

use std::collections::HashMap;

use futures::stream::{self, StreamExt};
use log::debug;

use crate::cli::TeamAccessSortField;
use crate::config::api;
use crate::error::Result as TfeResult;
use crate::hcp::projects::{resolve_project, Project};
use crate::hcp::teams::Team;
use crate::hcp::TfeClient;
use crate::output::output_team_access;
use crate::ui::{create_spinner, finish_spinner};
use crate::{Cli, Command, GetResource};

use super::models::{EnrichedTeamProjectAccess, TeamProjectAccess};

/// Run the team-access list command
pub async fn run_team_access_command(
    client: &TfeClient,
    cli: &Cli,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let Command::Get {
        resource: GetResource::TeamAccess(args),
    } = &cli.command
    else {
        unreachable!()
    };

    let effective_org = client.effective_org(args.org.as_ref());

    let org = effective_org
        .as_ref()
        .ok_or("Organization is required (--org)")?;

    let team_name = args.name.as_deref();
    let prj_input = args.prj.as_deref();

    debug!(
        "Fetching team-project access for org: {}, team: {:?}, project: {:?}",
        org, team_name, prj_input
    );

    let bindings = match (team_name, prj_input) {
        // team + project: resolve both, fetch single project bindings, filter by team
        (Some(team), Some(prj)) => {
            let spinner = create_spinner(
                &format!(
                    "Fetching team-project access for team '{}' in project '{}'...",
                    team, prj
                ),
                cli.batch,
            );

            let resolved_prj = resolve_project(client, prj, org, cli.batch).await?;
            let team_id = client.resolve_team_id(org, team).await?;
            let team_id = team_id.ok_or(format!(
                "Team '{}' not found in organization '{}'",
                team, org
            ))?;

            let all_bindings = client
                .get_team_project_access(&resolved_prj.project.id)
                .await?;

            let filtered: Vec<TeamProjectAccess> = all_bindings
                .into_iter()
                .filter(|b| b.team_id() == team_id)
                .collect();

            // Fetch teams for enrichment
            let teams = client.get_teams(org).await?;
            let projects = vec![resolved_prj.project];

            finish_spinner(spinner);
            enrich_bindings(&filtered, &teams, &projects)
        }
        // team + all projects: resolve team, fan-out per project
        (Some(team), None) => {
            let spinner = create_spinner(
                &format!(
                    "Fetching team-project access for team '{}' across all projects...",
                    team
                ),
                cli.batch,
            );

            let team_id = client.resolve_team_id(org, team).await?;
            let team_id = team_id.ok_or(format!(
                "Team '{}' not found in organization '{}'",
                team, org
            ))?;

            let (teams, projects) =
                tokio::join!(client.get_teams(org), client.get_projects(org, None));
            let teams = teams?;
            let projects = projects?;

            let all_bindings = fan_out_per_project(client, &projects).await?;

            let filtered: Vec<TeamProjectAccess> = all_bindings
                .into_iter()
                .filter(|b| b.team_id() == team_id)
                .collect();

            finish_spinner(spinner);
            enrich_bindings(&filtered, &teams, &projects)
        }
        // all teams + single project: resolve project, fetch bindings
        (None, Some(prj)) => {
            let spinner = create_spinner(
                &format!("Fetching team-project access for project '{}'...", prj),
                cli.batch,
            );

            let resolved_prj = resolve_project(client, prj, org, cli.batch).await?;

            let (teams, bindings) = tokio::join!(
                client.get_teams(org),
                client.get_team_project_access(&resolved_prj.project.id)
            );
            let teams = teams?;
            let bindings = bindings?;
            let projects = vec![resolved_prj.project];

            finish_spinner(spinner);
            enrich_bindings(&bindings, &teams, &projects)
        }
        // all teams + all projects: fan-out
        (None, None) => {
            let spinner = create_spinner(
                &format!(
                    "Fetching team-project access across all projects in '{}'...",
                    org
                ),
                cli.batch,
            );

            let (teams, projects) =
                tokio::join!(client.get_teams(org), client.get_projects(org, None));
            let teams = teams?;
            let projects = projects?;

            let all_bindings = fan_out_per_project(client, &projects).await?;

            finish_spinner(spinner);
            enrich_bindings(&all_bindings, &teams, &projects)
        }
    };

    // Apply client-side filter
    let mut bindings = if let Some(filter) = &args.filter {
        filter_bindings(bindings, filter)
    } else {
        bindings
    };

    if bindings.is_empty() {
        if args.filter.is_some() {
            eprintln!("No team-project access bindings found matching filter");
        } else {
            eprintln!("No team-project access bindings found");
        }
        return Ok(());
    }

    // Sort
    sort_team_access(&mut bindings, &args.sort, args.reverse);

    output_team_access(&bindings, &args.output, cli.no_header);
    Ok(())
}

/// Fan out team-project access fetches per project with concurrency
async fn fan_out_per_project(
    client: &TfeClient,
    projects: &[Project],
) -> std::result::Result<Vec<TeamProjectAccess>, Box<dyn std::error::Error>> {
    let project_ids: Vec<String> = projects.iter().map(|p| p.id.clone()).collect();

    let results: Vec<TfeResult<Vec<TeamProjectAccess>>> = stream::iter(
        project_ids
            .into_iter()
            .map(|prj_id| async move { client.get_team_project_access(&prj_id).await }),
    )
    .buffer_unordered(api::MAX_CONCURRENT_PAGE_REQUESTS)
    .collect()
    .await;

    let mut all_bindings = Vec::new();
    for result in results {
        match result {
            Ok(bindings) => all_bindings.extend(bindings),
            Err(e) => {
                eprintln!("Error fetching team-project access: {}", e);
                // Continue with partial results
            }
        }
    }

    Ok(all_bindings)
}

/// Enrich bindings with team and project names from pre-fetched data
fn enrich_bindings(
    bindings: &[TeamProjectAccess],
    teams: &[Team],
    projects: &[Project],
) -> Vec<EnrichedTeamProjectAccess> {
    let team_names: HashMap<&str, &str> = teams.iter().map(|t| (t.id.as_str(), t.name())).collect();

    let project_names: HashMap<&str, &str> = projects
        .iter()
        .map(|p| (p.id.as_str(), p.attributes.name.as_str()))
        .collect();

    bindings
        .iter()
        .map(|b| EnrichedTeamProjectAccess {
            id: b.id.clone(),
            team_id: b.team_id().to_string(),
            team_name: team_names
                .get(b.team_id())
                .unwrap_or(&b.team_id())
                .to_string(),
            project_id: b.project_id().to_string(),
            project_name: project_names
                .get(b.project_id())
                .unwrap_or(&b.project_id())
                .to_string(),
            access: b.access().to_string(),
        })
        .collect()
}

/// Filter enriched bindings by substring match on team name, project name, or access level
fn filter_bindings(
    bindings: Vec<EnrichedTeamProjectAccess>,
    filter: &str,
) -> Vec<EnrichedTeamProjectAccess> {
    let filter_lower = filter.to_lowercase();
    bindings
        .into_iter()
        .filter(|b| {
            b.team_name.to_lowercase().contains(&filter_lower)
                || b.project_name.to_lowercase().contains(&filter_lower)
                || b.access.to_lowercase().contains(&filter_lower)
        })
        .collect()
}

/// Sort enriched team-project access bindings
fn sort_team_access(
    bindings: &mut [EnrichedTeamProjectAccess],
    sort_field: &TeamAccessSortField,
    reverse: bool,
) {
    bindings.sort_by(|a, b| {
        let cmp = match sort_field {
            TeamAccessSortField::Team => a.team_name.cmp(&b.team_name),
            TeamAccessSortField::Project => a.project_name.cmp(&b.project_name),
            TeamAccessSortField::Access => a.access.cmp(&b.access),
        };
        if reverse {
            cmp.reverse()
        } else {
            cmp
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_enriched(
        id: &str,
        team_name: &str,
        project_name: &str,
        access: &str,
    ) -> EnrichedTeamProjectAccess {
        EnrichedTeamProjectAccess {
            id: id.to_string(),
            team_id: format!("team-{}", team_name),
            team_name: team_name.to_string(),
            project_id: format!("prj-{}", project_name),
            project_name: project_name.to_string(),
            access: access.to_string(),
        }
    }

    #[test]
    fn test_sort_team_access_by_team() {
        let mut bindings = vec![
            make_enriched("tprj-1", "zebra", "infra", "admin"),
            make_enriched("tprj-2", "alpha", "infra", "read"),
            make_enriched("tprj-3", "middle", "infra", "write"),
        ];
        sort_team_access(&mut bindings, &TeamAccessSortField::Team, false);
        assert_eq!(bindings[0].team_name, "alpha");
        assert_eq!(bindings[1].team_name, "middle");
        assert_eq!(bindings[2].team_name, "zebra");
    }

    #[test]
    fn test_sort_team_access_by_project() {
        let mut bindings = vec![
            make_enriched("tprj-1", "owners", "zebra-prj", "admin"),
            make_enriched("tprj-2", "owners", "alpha-prj", "admin"),
            make_enriched("tprj-3", "owners", "middle-prj", "admin"),
        ];
        sort_team_access(&mut bindings, &TeamAccessSortField::Project, false);
        assert_eq!(bindings[0].project_name, "alpha-prj");
        assert_eq!(bindings[1].project_name, "middle-prj");
        assert_eq!(bindings[2].project_name, "zebra-prj");
    }

    #[test]
    fn test_sort_team_access_by_access() {
        let mut bindings = vec![
            make_enriched("tprj-1", "owners", "infra", "write"),
            make_enriched("tprj-2", "devs", "infra", "admin"),
            make_enriched("tprj-3", "ops", "infra", "read"),
        ];
        sort_team_access(&mut bindings, &TeamAccessSortField::Access, false);
        assert_eq!(bindings[0].access, "admin");
        assert_eq!(bindings[1].access, "read");
        assert_eq!(bindings[2].access, "write");
    }

    #[test]
    fn test_sort_team_access_reverse() {
        let mut bindings = vec![
            make_enriched("tprj-1", "alpha", "infra", "admin"),
            make_enriched("tprj-2", "zebra", "infra", "read"),
            make_enriched("tprj-3", "middle", "infra", "write"),
        ];
        sort_team_access(&mut bindings, &TeamAccessSortField::Team, true);
        assert_eq!(bindings[0].team_name, "zebra");
        assert_eq!(bindings[1].team_name, "middle");
        assert_eq!(bindings[2].team_name, "alpha");
    }

    #[test]
    fn test_sort_team_access_empty() {
        let mut bindings: Vec<EnrichedTeamProjectAccess> = vec![];
        sort_team_access(&mut bindings, &TeamAccessSortField::Team, false);
        assert!(bindings.is_empty());
    }

    #[test]
    fn test_sort_team_access_single_item() {
        let mut bindings = vec![make_enriched("tprj-1", "owners", "infra", "admin")];
        sort_team_access(&mut bindings, &TeamAccessSortField::Team, false);
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].team_name, "owners");
    }

    #[test]
    fn test_enrich_bindings_basic() {
        let bindings = vec![
            serde_json::from_value::<TeamProjectAccess>(serde_json::json!({
                "id": "tprj-1",
                "type": "team-projects",
                "attributes": { "access": "admin" },
                "relationships": {
                    "team": { "data": { "id": "team-abc", "type": "teams" } },
                    "project": { "data": { "id": "prj-def", "type": "projects" } }
                }
            }))
            .unwrap(),
        ];

        let teams = vec![serde_json::from_value::<Team>(serde_json::json!({
            "id": "team-abc",
            "type": "teams",
            "attributes": { "name": "owners" }
        }))
        .unwrap()];

        let projects = vec![serde_json::from_value::<Project>(serde_json::json!({
            "id": "prj-def",
            "type": "projects",
            "attributes": { "name": "infra" }
        }))
        .unwrap()];

        let enriched = enrich_bindings(&bindings, &teams, &projects);
        assert_eq!(enriched.len(), 1);
        assert_eq!(enriched[0].id, "tprj-1");
        assert_eq!(enriched[0].team_name, "owners");
        assert_eq!(enriched[0].project_name, "infra");
        assert_eq!(enriched[0].access, "admin");
        assert_eq!(enriched[0].team_id, "team-abc");
        assert_eq!(enriched[0].project_id, "prj-def");
    }

    #[test]
    fn test_enrich_bindings_missing_names_falls_back_to_ids() {
        let bindings = vec![
            serde_json::from_value::<TeamProjectAccess>(serde_json::json!({
                "id": "tprj-1",
                "type": "team-projects",
                "attributes": { "access": "read" },
                "relationships": {
                    "team": { "data": { "id": "team-unknown", "type": "teams" } },
                    "project": { "data": { "id": "prj-unknown", "type": "projects" } }
                }
            }))
            .unwrap(),
        ];

        // Empty team/project lists — names won't be resolved
        let teams: Vec<Team> = vec![];
        let projects: Vec<Project> = vec![];

        let enriched = enrich_bindings(&bindings, &teams, &projects);
        assert_eq!(enriched.len(), 1);
        // Should fall back to IDs
        assert_eq!(enriched[0].team_name, "team-unknown");
        assert_eq!(enriched[0].project_name, "prj-unknown");
    }

    #[test]
    fn test_enrich_bindings_empty() {
        let bindings: Vec<TeamProjectAccess> = vec![];
        let teams: Vec<Team> = vec![];
        let projects: Vec<Project> = vec![];

        let enriched = enrich_bindings(&bindings, &teams, &projects);
        assert!(enriched.is_empty());
    }

    #[test]
    fn test_enrich_bindings_multiple() {
        let bindings = vec![
            serde_json::from_value::<TeamProjectAccess>(serde_json::json!({
                "id": "tprj-1",
                "type": "team-projects",
                "attributes": { "access": "admin" },
                "relationships": {
                    "team": { "data": { "id": "team-a", "type": "teams" } },
                    "project": { "data": { "id": "prj-x", "type": "projects" } }
                }
            }))
            .unwrap(),
            serde_json::from_value::<TeamProjectAccess>(serde_json::json!({
                "id": "tprj-2",
                "type": "team-projects",
                "attributes": { "access": "read" },
                "relationships": {
                    "team": { "data": { "id": "team-b", "type": "teams" } },
                    "project": { "data": { "id": "prj-y", "type": "projects" } }
                }
            }))
            .unwrap(),
        ];

        let teams = vec![
            serde_json::from_value::<Team>(serde_json::json!({
                "id": "team-a",
                "type": "teams",
                "attributes": { "name": "alpha" }
            }))
            .unwrap(),
            serde_json::from_value::<Team>(serde_json::json!({
                "id": "team-b",
                "type": "teams",
                "attributes": { "name": "beta" }
            }))
            .unwrap(),
        ];

        let projects = vec![
            serde_json::from_value::<Project>(serde_json::json!({
                "id": "prj-x",
                "type": "projects",
                "attributes": { "name": "proj-x" }
            }))
            .unwrap(),
            serde_json::from_value::<Project>(serde_json::json!({
                "id": "prj-y",
                "type": "projects",
                "attributes": { "name": "proj-y" }
            }))
            .unwrap(),
        ];

        let enriched = enrich_bindings(&bindings, &teams, &projects);
        assert_eq!(enriched.len(), 2);
        assert_eq!(enriched[0].team_name, "alpha");
        assert_eq!(enriched[0].project_name, "proj-x");
        assert_eq!(enriched[1].team_name, "beta");
        assert_eq!(enriched[1].project_name, "proj-y");
    }

    #[test]
    fn test_filter_bindings_by_team_name() {
        let bindings = vec![
            make_enriched("tprj-1", "owners", "infra", "admin"),
            make_enriched("tprj-2", "devs", "app", "read"),
            make_enriched("tprj-3", "ops-team", "infra", "write"),
        ];
        let filtered = filter_bindings(bindings, "owners");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].team_name, "owners");
    }

    #[test]
    fn test_filter_bindings_by_project_name() {
        let bindings = vec![
            make_enriched("tprj-1", "owners", "infra", "admin"),
            make_enriched("tprj-2", "devs", "app", "read"),
            make_enriched("tprj-3", "ops", "infra", "write"),
        ];
        let filtered = filter_bindings(bindings, "infra");
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].project_name, "infra");
        assert_eq!(filtered[1].project_name, "infra");
    }

    #[test]
    fn test_filter_bindings_by_access_level() {
        let bindings = vec![
            make_enriched("tprj-1", "owners", "infra", "admin"),
            make_enriched("tprj-2", "devs", "app", "admin"),
            make_enriched("tprj-3", "ops", "infra", "read"),
        ];
        let filtered = filter_bindings(bindings, "admin");
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].access, "admin");
        assert_eq!(filtered[1].access, "admin");
    }

    #[test]
    fn test_filter_bindings_case_insensitive() {
        let bindings = vec![
            make_enriched("tprj-1", "Owners", "infra", "admin"),
            make_enriched("tprj-2", "devs", "app", "read"),
        ];
        let filtered = filter_bindings(bindings, "OWNERS");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].team_name, "Owners");
    }

    #[test]
    fn test_filter_bindings_no_match() {
        let bindings = vec![
            make_enriched("tprj-1", "owners", "infra", "admin"),
            make_enriched("tprj-2", "devs", "app", "read"),
        ];
        let filtered = filter_bindings(bindings, "nonexistent");
        assert!(filtered.is_empty());
    }

    #[test]
    fn test_filter_bindings_empty_input() {
        let bindings: Vec<EnrichedTeamProjectAccess> = vec![];
        let filtered = filter_bindings(bindings, "anything");
        assert!(filtered.is_empty());
    }

    #[test]
    fn test_filter_bindings_substring_match() {
        let bindings = vec![
            make_enriched("tprj-1", "platform-owners", "infra-prod", "admin"),
            make_enriched("tprj-2", "devs", "app", "read"),
        ];
        let filtered = filter_bindings(bindings, "owner");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].team_name, "platform-owners");
    }
}
