//! Organization output formatter

use super::common::escape_csv;
use crate::cli::{Cli, Command, GetResource, OutputFormat};
use crate::hcp::{Organization, TfeResource};
use comfy_table::{presets::NOTHING, Table};
use serde::Serialize;

/// Serializable organization for structured output (JSON/YAML)
#[derive(Serialize)]
struct SerializableOrganization {
    name: String,
    id: String,
}

impl From<&Organization> for SerializableOrganization {
    fn from(org: &Organization) -> Self {
        Self {
            name: org.name().to_string(),
            id: org.id.clone(),
        }
    }
}

/// Output organizations in the specified format
pub fn output_organizations(orgs: &[Organization], cli: &Cli) {
    let Command::Get {
        resource: GetResource::Org(args),
    } = &cli.command
    else {
        unreachable!()
    };

    match args.output {
        OutputFormat::Table => output_table(orgs, cli.no_header),
        OutputFormat::Csv => output_csv(orgs, cli.no_header),
        OutputFormat::Json => output_json(orgs),
        OutputFormat::Yaml => output_yaml(orgs),
    }
}

fn output_table(orgs: &[Organization], no_header: bool) {
    let mut table = Table::new();
    table.load_preset(NOTHING);
    if !no_header {
        table.set_header(vec!["Name", "ID"]);
    }

    for org in orgs {
        table.add_row(vec![org.name(), &org.id]);
    }

    println!();
    println!("{table}");
    if !no_header {
        println!("\nTotal: {} organizations", orgs.len());
    }
}

fn output_csv(orgs: &[Organization], no_header: bool) {
    if !no_header {
        println!("name,id");
    }
    for org in orgs {
        println!("{},{}", escape_csv(org.name()), escape_csv(&org.id));
    }
}

fn output_json(orgs: &[Organization]) {
    let data: Vec<SerializableOrganization> = orgs.iter().map(|o| o.into()).collect();
    println!("{}", serde_json::to_string_pretty(&data).unwrap());
}

fn output_yaml(orgs: &[Organization]) {
    let data: Vec<SerializableOrganization> = orgs.iter().map(|o| o.into()).collect();
    println!("{}", serde_yaml::to_string(&data).unwrap());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hcp::OrganizationAttributes;

    fn create_test_org() -> Organization {
        Organization {
            id: "test-org".to_string(),
            org_type: Some("organizations".to_string()),
            attributes: Some(OrganizationAttributes {
                name: Some("test-org".to_string()),
                email: None,
                external_id: None,
            }),
        }
    }

    #[test]
    fn test_output_table_empty() {
        // Should not panic with empty input
        output_table(&[], false);
    }

    #[test]
    fn test_output_table() {
        let orgs = vec![create_test_org()];
        // Should not panic
        output_table(&orgs, false);
    }

    #[test]
    fn test_output_csv() {
        let orgs = vec![create_test_org()];
        // Should not panic
        output_csv(&orgs, false);
    }

    #[test]
    fn test_output_json() {
        let orgs = vec![create_test_org()];
        // Should not panic
        output_json(&orgs);
    }

    #[test]
    fn test_output_yaml() {
        let orgs = vec![create_test_org()];
        // Should not panic
        output_yaml(&orgs);
    }

    #[test]
    fn test_output_no_header() {
        let orgs = vec![create_test_org()];
        // Should not panic
        output_table(&orgs, true);
        output_csv(&orgs, true);
    }
}
