//! Output formatting for organization memberships

use comfy_table::{presets::UTF8_FULL_CONDENSED, Cell, CellAlignment, ContentArrangement, Table};

use crate::hcp::OrganizationMembership;
use crate::{OrgMemberArgs, OutputFormat};

/// Output organization memberships in the requested format
pub fn output_org_memberships(
    memberships: &[(String, OrganizationMembership)],
    args: &OrgMemberArgs,
    no_header: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    match args.output {
        OutputFormat::Json => output_json(memberships),
        OutputFormat::Yaml => output_yaml(memberships),
        OutputFormat::Csv => output_csv(memberships, no_header),
        OutputFormat::Table => output_table(memberships, no_header),
    }
}

fn output_json(
    memberships: &[(String, OrganizationMembership)],
) -> Result<(), Box<dyn std::error::Error>> {
    let output: Vec<_> = memberships
        .iter()
        .map(|(org, m)| {
            serde_json::json!({
                "id": m.id,
                "organization": org,
                "email": m.email(),
                "status": m.status(),
                "created_at": m.created_at(),
                "teams": m.team_ids()
            })
        })
        .collect();
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn output_yaml(
    memberships: &[(String, OrganizationMembership)],
) -> Result<(), Box<dyn std::error::Error>> {
    let output: Vec<_> = memberships
        .iter()
        .map(|(org, m)| {
            serde_json::json!({
                "id": m.id,
                "organization": org,
                "email": m.email(),
                "status": m.status(),
                "created_at": m.created_at(),
                "teams": m.team_ids()
            })
        })
        .collect();
    println!("{}", serde_yaml::to_string(&output)?);
    Ok(())
}

fn output_csv(
    memberships: &[(String, OrganizationMembership)],
    no_header: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if !no_header {
        println!("id,organization,email,status,created_at,teams");
    }
    for (org, m) in memberships {
        println!(
            "{},{},{},{},{},\"{}\"",
            m.id,
            org,
            m.email(),
            m.status(),
            m.created_at(),
            m.team_ids().join(",")
        );
    }
    Ok(())
}

fn output_table(
    memberships: &[(String, OrganizationMembership)],
    no_header: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if memberships.is_empty() {
        println!("No organization members found");
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic);

    if !no_header {
        table.set_header(vec![
            Cell::new("ID"),
            Cell::new("ORGANIZATION"),
            Cell::new("EMAIL"),
            Cell::new("STATUS"),
            Cell::new("TEAMS"),
        ]);
    }

    for (org, m) in memberships {
        table.add_row(vec![
            Cell::new(&m.id),
            Cell::new(org),
            Cell::new(m.email()),
            Cell::new(m.status()),
            Cell::new(m.team_ids().join(", ")).set_alignment(CellAlignment::Right),
        ]);
    }

    println!("{table}");
    Ok(())
}
