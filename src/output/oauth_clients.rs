//! OAuth Client output formatter

use super::common::escape_csv;
use crate::cli::{Cli, Command, GetResource, OutputFormat};
use crate::hcp::{OAuthClient, TfeResource};
use comfy_table::{presets::NOTHING, Table};
use serde::Serialize;

/// OAuth Client row type alias
pub type OAuthClientRow = (String, Vec<OAuthClient>);

/// Serializable OAuth Client for structured output (JSON/YAML)
#[derive(Serialize)]
struct SerializableOAuthClient {
    org: String,
    id: String,
    name: String,
    service_provider: String,
    service_provider_display_name: String,
    http_url: String,
    created_at: String,
    organization_scoped: bool,
    oauth_token_ids: Vec<String>,
}

impl SerializableOAuthClient {
    fn from_client(org: &str, client: &OAuthClient) -> Self {
        Self {
            org: org.to_string(),
            id: client.id.clone(),
            name: client.name().to_string(),
            service_provider: client.service_provider().to_string(),
            service_provider_display_name: client.service_provider_display_name().to_string(),
            http_url: client.http_url().to_string(),
            created_at: client.created_at().to_string(),
            organization_scoped: client.is_organization_scoped(),
            oauth_token_ids: client
                .oauth_token_ids()
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }
}

/// Output OAuth clients in the specified format
pub fn output_oauth_clients(clients: &[OAuthClientRow], cli: &Cli) {
    let Command::Get {
        resource: GetResource::Oc(args),
    } = &cli.command
    else {
        unreachable!()
    };

    match args.output {
        OutputFormat::Table => output_table(clients, cli.no_header),
        OutputFormat::Csv => output_csv(clients, cli.no_header),
        OutputFormat::Json => output_json(clients),
        OutputFormat::Yaml => output_yaml(clients),
    }
}

fn output_table(clients: &[OAuthClientRow], no_header: bool) {
    let mut table = Table::new();
    table.load_preset(NOTHING);
    if !no_header {
        table.set_header(vec![
            "Org",
            "ID",
            "Name",
            "Provider",
            "URL",
            "Created At",
            "Org Scoped",
            "OAuth Tokens",
        ]);
    }

    let mut total = 0;
    for (org_name, org_clients) in clients {
        for client in org_clients {
            let org_scoped = if client.is_organization_scoped() {
                "Yes"
            } else {
                "No"
            };
            let token_ids = client.oauth_token_ids().join(", ");
            table.add_row(vec![
                org_name,
                &client.id,
                client.name(),
                client.service_provider_display_name(),
                client.http_url(),
                client.created_at(),
                org_scoped,
                &token_ids,
            ]);
            total += 1;
        }
    }

    println!();
    println!("{table}");
    if !no_header {
        println!("\nTotal: {} OAuth clients", total);
    }
}

fn output_csv(clients: &[OAuthClientRow], no_header: bool) {
    if !no_header {
        println!("org,id,name,service_provider,service_provider_display_name,http_url,created_at,organization_scoped,oauth_token_ids");
    }

    for (org_name, org_clients) in clients {
        for client in org_clients {
            let token_ids = client.oauth_token_ids().join(";");
            println!(
                "{},{},{},{},{},{},{},{},{}",
                escape_csv(org_name),
                escape_csv(&client.id),
                escape_csv(client.name()),
                escape_csv(client.service_provider()),
                escape_csv(client.service_provider_display_name()),
                escape_csv(client.http_url()),
                escape_csv(client.created_at()),
                client.is_organization_scoped(),
                escape_csv(&token_ids)
            );
        }
    }
}

fn build_serializable_clients(clients: &[OAuthClientRow]) -> Vec<SerializableOAuthClient> {
    clients
        .iter()
        .flat_map(|(org_name, org_clients)| {
            org_clients
                .iter()
                .map(|c| SerializableOAuthClient::from_client(org_name, c))
        })
        .collect()
}

fn output_json(clients: &[OAuthClientRow]) {
    let data = build_serializable_clients(clients);
    println!("{}", serde_json::to_string_pretty(&data).unwrap());
}

fn output_yaml(clients: &[OAuthClientRow]) {
    let data = build_serializable_clients(clients);
    println!("{}", serde_yaml::to_string(&data).unwrap());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hcp::OAuthClientAttributes;

    fn create_test_oauth_client() -> OAuthClient {
        OAuthClient {
            id: "oc-123".to_string(),
            client_type: Some("oauth-clients".to_string()),
            attributes: OAuthClientAttributes {
                created_at: Some("2025-01-01T00:00:00Z".to_string()),
                service_provider: Some("github".to_string()),
                service_provider_display_name: Some("GitHub".to_string()),
                name: Some("My GitHub".to_string()),
                http_url: Some("https://github.com".to_string()),
                api_url: None,
                callback_url: None,
                organization_scoped: Some(true),
            },
            relationships: None,
        }
    }

    #[test]
    fn test_output_table_empty() {
        output_table(&[], false);
    }

    #[test]
    fn test_output_table() {
        let clients = vec![("test-org".to_string(), vec![create_test_oauth_client()])];
        output_table(&clients, false);
    }

    #[test]
    fn test_output_csv() {
        let clients = vec![("test-org".to_string(), vec![create_test_oauth_client()])];
        output_csv(&clients, false);
    }

    #[test]
    fn test_output_json() {
        let clients = vec![("test-org".to_string(), vec![create_test_oauth_client()])];
        output_json(&clients);
    }

    #[test]
    fn test_output_yaml() {
        let clients = vec![("test-org".to_string(), vec![create_test_oauth_client()])];
        output_yaml(&clients);
    }

    #[test]
    fn test_output_no_header() {
        let clients = vec![("test-org".to_string(), vec![create_test_oauth_client()])];
        output_table(&clients, true);
        output_csv(&clients, true);
    }
}
