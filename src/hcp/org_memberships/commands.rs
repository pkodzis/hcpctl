//! Organization membership command handlers

use log::debug;

use crate::cli::OutputFormat;
use crate::hcp::helpers::fetch_from_organizations;
use crate::hcp::TfeClient;
use crate::output::org_memberships::output_org_memberships;
use crate::ui::{create_spinner, finish_spinner};
use crate::{Cli, Command, DeleteOrgMemberArgs, GetResource, InviteArgs};

/// Run the get org-member command
pub async fn run_org_member_command(
    client: &TfeClient,
    cli: &Cli,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let Command::Get {
        resource: GetResource::OrgMember(args),
    } = &cli.command
    else {
        unreachable!()
    };

    // If ID is specified, get single membership or filter by email
    if let Some(id_or_email) = &args.id {
        return get_single_org_member(client, cli, id_or_email, args.org.as_ref()).await;
    }

    let memberships = if let Some(org) = &args.org {
        // Single org
        let spinner = create_spinner(&format!("Fetching members from '{}'...", org), cli.batch);
        let result = client.get_org_memberships(org).await?;
        finish_spinner(spinner);
        result
            .into_iter()
            .map(|m| (org.clone(), m))
            .collect::<Vec<_>>()
    } else {
        // All orgs - parallel fetch
        let spinner = create_spinner("Fetching organizations...", cli.batch);
        let orgs = client.get_organizations().await?;
        finish_spinner(spinner);

        let spinner = create_spinner(
            &format!("Fetching members from {} organizations...", orgs.len()),
            cli.batch,
        );

        let results = fetch_from_organizations(orgs, |org| async move {
            match client.get_org_memberships(&org).await {
                Ok(members) => {
                    let with_org: Vec<_> = members.into_iter().map(|m| (org.clone(), m)).collect();
                    Ok(with_org)
                }
                Err(e) => Err((org, e)),
            }
        })
        .await;

        finish_spinner(spinner);

        // Collect all successful results, ignore errors
        results
            .into_iter()
            .filter_map(|r| r.ok())
            .flatten()
            .collect()
    };

    // Apply filters
    let filtered: Vec<_> = memberships
        .into_iter()
        .filter(|(_, m)| {
            // Filter by email
            if let Some(filter) = &args.filter {
                let email = m.email().to_lowercase();
                if !email.contains(&filter.to_lowercase()) {
                    return false;
                }
            }
            // Filter by status
            if let Some(status_filter) = &args.status {
                let status = m.status().to_lowercase();
                if !status.eq_ignore_ascii_case(status_filter) {
                    return false;
                }
            }
            true
        })
        .collect();

    output_org_memberships(&filtered, args, cli.no_header)?;

    Ok(())
}

/// Get a single org member by ID or email
async fn get_single_org_member(
    client: &TfeClient,
    cli: &Cli,
    id_or_email: &str,
    org: Option<&String>,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let Command::Get {
        resource: GetResource::OrgMember(args),
    } = &cli.command
    else {
        unreachable!()
    };

    // Get orgs to search
    let orgs = if let Some(org) = org {
        vec![org.clone()]
    } else {
        client.get_organizations().await?
    };

    let spinner = create_spinner(&format!("Looking up '{}'...", id_or_email), cli.batch);

    // If it's an ID (ou-...), search all orgs in parallel to find it
    if id_or_email.starts_with("ou-") {
        let target_id = id_or_email.to_string();

        let results = fetch_from_organizations(orgs, |org| {
            let target = target_id.clone();
            async move {
                match client.get_org_memberships(&org).await {
                    Ok(members) => {
                        if let Some(m) = members.into_iter().find(|m| m.id == target) {
                            Ok(Some((org, m)))
                        } else {
                            Ok(None)
                        }
                    }
                    Err(e) => Err((org, e)),
                }
            }
        })
        .await;

        finish_spinner(spinner);

        // Find the first match
        for result in results {
            if let Ok(Some((org_name, m))) = result {
                return output_single_membership(&org_name, &m, args, cli);
            }
        }

        return Err(format!("Membership '{}' not found", id_or_email).into());
    }

    // It's an email - search by email in parallel
    let email = id_or_email.to_string();

    let results = fetch_from_organizations(orgs, |org| {
        let email_ref = email.clone();
        async move {
            match client.get_org_membership_by_email(&org, &email_ref).await {
                Ok(Some(m)) => Ok(Some((org, m))),
                Ok(None) => Ok(None),
                Err(e) => Err((org, e)),
            }
        }
    })
    .await;

    finish_spinner(spinner);

    // Collect all found memberships
    let found: Vec<_> = results
        .into_iter()
        .filter_map(|r| r.ok())
        .flatten()
        .collect();

    if found.is_empty() {
        return Err(format!("No memberships found for '{}'", email).into());
    }

    // If single result, output as single; otherwise as list
    if found.len() == 1 {
        let (org_name, m) = &found[0];
        output_single_membership(org_name, m, args, cli)
    } else {
        output_org_memberships(&found, args, cli.no_header)
    }
}

/// Output a single membership
fn output_single_membership(
    org: &str,
    m: &crate::hcp::OrganizationMembership,
    args: &crate::OrgMemberArgs,
    cli: &Cli,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    match args.output {
        OutputFormat::Json => {
            let output = serde_json::json!({
                "id": m.id,
                "organization": org,
                "email": m.email(),
                "status": m.status(),
                "created_at": m.created_at(),
                "teams": m.team_ids()
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        OutputFormat::Yaml => {
            let output = serde_json::json!({
                "id": m.id,
                "organization": org,
                "email": m.email(),
                "status": m.status(),
                "created_at": m.created_at(),
                "teams": m.team_ids()
            });
            println!("{}", serde_yaml::to_string(&output)?);
        }
        OutputFormat::Csv | OutputFormat::Table => {
            let memberships = vec![(org.to_string(), m.clone())];
            output_org_memberships(&memberships, args, cli.no_header)?;
        }
    }
    Ok(())
}

/// Run the delete org-member command
pub async fn run_delete_org_member_command(
    client: &TfeClient,
    cli: &Cli,
    _args: &DeleteOrgMemberArgs,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let Command::Delete {
        resource: crate::DeleteResource::OrgMember(args),
    } = &cli.command
    else {
        unreachable!()
    };

    // Track email and org for better confirmation message
    let mut resolved_email: Option<String> = None;
    let mut resolved_org: Option<String> = None;

    let id_or_email = &args.id;

    // Resolve membership ID from argument (can be ou-xxx ID or email)
    let membership_id = if id_or_email.contains('@') {
        // It's an email, need --org
        let org = args
            .org
            .as_ref()
            .ok_or("Email provided requires --org. Use: delete org-member EMAIL --org ORG")?;
        resolved_email = Some(id_or_email.clone());
        resolved_org = Some(org.clone());
        let spinner = create_spinner(
            &format!("Looking up {} in '{}'...", id_or_email, org),
            cli.batch,
        );
        let membership = client.get_org_membership_by_email(org, id_or_email).await?;
        finish_spinner(spinner);
        match membership {
            Some(m) => m.id,
            None => {
                return Err(
                    format!("User '{}' not found in organization '{}'", id_or_email, org).into(),
                )
            }
        }
    } else if !id_or_email.starts_with("ou-") {
        return Err(format!(
            "Invalid argument '{}'. Must be membership ID (ou-xxx) or email with --org",
            id_or_email
        )
        .into());
    } else {
        id_or_email.clone()
    };

    // Confirm deletion - show email and org if available for better readability
    if !args.yes && !cli.batch {
        let confirm_msg = if let (Some(email), Some(org)) = (&resolved_email, &resolved_org) {
            format!(
                "Delete membership for '{}' from organization '{}'? [y/N] ",
                email, org
            )
        } else {
            format!("Delete membership {}? [y/N] ", membership_id)
        };
        print!("{}", confirm_msg);
        use std::io::{self, Write};
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled");
            return Ok(());
        }
    }

    let spinner = create_spinner(
        &format!("Deleting membership {}...", membership_id),
        cli.batch,
    );
    client.delete_org_membership(&membership_id).await?;
    finish_spinner(spinner);

    // Show confirmation with email if available
    if let Some(email) = &resolved_email {
        println!("✓ Deleted membership for '{}'", email);
    } else {
        println!("✓ Deleted membership {}", membership_id);
    }

    Ok(())
}

/// Run the invite user command
pub async fn run_invite_command(
    client: &TfeClient,
    cli: &Cli,
    _args: &InviteArgs,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let Command::Invite(args) = &cli.command else {
        unreachable!()
    };

    let org = &args.org;
    let email = &args.email;

    debug!("Inviting user {} to organization {}", email, org);

    // Resolve team IDs if provided (comma-separated)
    let team_ids = if let Some(teams_str) = &args.teams {
        let spinner = create_spinner(&format!("Resolving teams '{}'...", teams_str), cli.batch);
        let mut ids = Vec::new();
        for team_ref in teams_str.split(',').map(|s| s.trim()) {
            if let Some(team_id) = client.resolve_team_id(org, team_ref).await? {
                ids.push(team_id);
            } else {
                finish_spinner(spinner);
                return Err(
                    format!("Team '{}' not found in organization '{}'", team_ref, org).into(),
                );
            }
        }
        finish_spinner(spinner);
        Some(ids)
    } else {
        None
    };

    let spinner = create_spinner(&format!("Inviting {} to '{}'...", email, org), cli.batch);

    let membership = client.invite_user(org, email, team_ids).await?;

    finish_spinner(spinner);

    println!(
        "✓ Invited {} to '{}' (membership ID: {}, status: {})",
        email,
        org,
        membership.id,
        membership.status()
    );

    if !membership.team_ids().is_empty() {
        println!("  Teams: {}", membership.team_ids().join(", "));
    }

    Ok(())
}
