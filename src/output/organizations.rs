//! Organization output formatter

use super::common::escape_csv;
use crate::cli::{Cli, Command, GetResource, OutputFormat};
use crate::hcp::{OrganizationWithTokens, TfeResource};
use comfy_table::{presets::NOTHING, Table};
use serde::Serialize;

/// Serializable organization for structured output (JSON/YAML)
#[derive(Serialize)]
struct SerializableOrganization {
    id: String,
    name: String,
    external_id: String,
    email: String,
    created_at: String,
    saml_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_project_id: Option<String>,
    oauth_token_ids: Vec<String>,
}

impl From<&OrganizationWithTokens> for SerializableOrganization {
    fn from(owt: &OrganizationWithTokens) -> Self {
        let org = &owt.organization;
        Self {
            id: org.id.clone(),
            name: org.name().to_string(),
            external_id: org.external_id().to_string(),
            email: org.email().to_string(),
            created_at: org.created_at().to_string(),
            saml_enabled: org.saml_enabled(),
            default_project_id: org.default_project_id().map(|s| s.to_string()),
            oauth_token_ids: owt
                .oauth_token_ids()
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }
}

/// Output organizations in the specified format
pub fn output_organizations(orgs: &[OrganizationWithTokens], cli: &Cli) {
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

fn output_table(orgs: &[OrganizationWithTokens], no_header: bool) {
    let mut table = Table::new();
    table.load_preset(NOTHING);
    if !no_header {
        table.set_header(vec![
            "Name",
            "External ID",
            "Email",
            "Created At",
            "SAML",
            "OAuth Tokens",
        ]);
    }

    for owt in orgs {
        let org = &owt.organization;
        let saml = if org.saml_enabled() { "Yes" } else { "No" };
        let token_ids = owt.oauth_token_ids().join(", ");
        table.add_row(vec![
            org.name(),
            org.external_id(),
            org.email(),
            org.created_at(),
            saml,
            &token_ids,
        ]);
    }

    println!();
    println!("{table}");
    if !no_header {
        println!("\nTotal: {} organizations", orgs.len());
    }
}

fn output_csv(orgs: &[OrganizationWithTokens], no_header: bool) {
    if !no_header {
        println!(
            "name,external_id,email,created_at,saml_enabled,default_project_id,oauth_token_ids"
        );
    }
    for owt in orgs {
        let org = &owt.organization;
        let token_ids = owt.oauth_token_ids().join(";");
        println!(
            "{},{},{},{},{},{},{}",
            escape_csv(org.name()),
            escape_csv(org.external_id()),
            escape_csv(org.email()),
            escape_csv(org.created_at()),
            org.saml_enabled(),
            escape_csv(org.default_project_id().unwrap_or("")),
            escape_csv(&token_ids)
        );
    }
}

fn output_json(orgs: &[OrganizationWithTokens]) {
    let data: Vec<SerializableOrganization> = orgs.iter().map(|o| o.into()).collect();
    super::common::print_json(&data);
}

fn output_yaml(orgs: &[OrganizationWithTokens]) {
    let data: Vec<SerializableOrganization> = orgs.iter().map(|o| o.into()).collect();
    super::common::print_yaml(&data);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hcp::{OAuthToken, Organization, OrganizationAttributes};

    fn create_test_org() -> OrganizationWithTokens {
        OrganizationWithTokens {
            organization: Organization {
                id: "test-org".to_string(),
                org_type: Some("organizations".to_string()),
                attributes: Some(OrganizationAttributes {
                    name: Some("test-org".to_string()),
                    email: Some("test@example.com".to_string()),
                    external_id: Some("org-123".to_string()),
                    created_at: Some("2025-01-01T00:00:00Z".to_string()),
                    saml_enabled: Some(false),
                }),
                relationships: None,
            },
            oauth_tokens: vec![
                OAuthToken {
                    id: "ot-abc123".to_string(),
                    token_type: Some("oauth-tokens".to_string()),
                    attributes: None,
                },
                OAuthToken {
                    id: "ot-def456".to_string(),
                    token_type: Some("oauth-tokens".to_string()),
                    attributes: None,
                },
            ],
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
