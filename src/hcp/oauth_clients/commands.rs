//! OAuth Client command handlers

use log::debug;

use crate::cli::{Cli, Command, GetResource, OutputFormat};
use crate::hcp::helpers::{collect_org_results, fetch_from_organizations, log_completion};
use crate::hcp::organizations::resolve_organizations;
use crate::hcp::traits::TfeResource;
use crate::hcp::TfeClient;
use crate::output::{output_oauth_clients, output_raw};
use crate::ui::{create_spinner, finish_spinner, finish_spinner_with_status};

use super::models::OAuthClient;

/// Run the OAuth client list command
pub async fn run_oc_command(
    client: &TfeClient,
    cli: &Cli,
) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Get {
        resource: GetResource::Oc(args),
    } = &cli.command
    else {
        unreachable!()
    };

    // If NAME is specified, get single OAuth client
    if let Some(name) = &args.name {
        let effective_org = client.effective_org(args.org.as_ref());
        return get_single_oauth_client(client, cli, name, effective_org.as_ref()).await;
    }

    let effective_org = client.effective_org(args.org.as_ref());

    // Otherwise list all OAuth clients
    let organizations = resolve_organizations(client, effective_org.as_ref()).await?;

    debug!(
        "Processing {} organizations: {:?}",
        organizations.len(),
        organizations
    );

    let spinner = create_spinner(
        &format!(
            "Fetching OAuth clients from {} organization(s)...",
            organizations.len()
        ),
        cli.batch,
    );

    // Fetch OAuth clients from all orgs in parallel
    let results = fetch_from_organizations(organizations, |org| async move {
        let clients = client.get_oauth_clients(&org).await;

        match clients {
            Ok(ocs) => {
                debug!("Found {} OAuth clients for org '{}'", ocs.len(), org);
                Ok((org, ocs))
            }
            Err(e) => {
                debug!("Error fetching OAuth clients for org '{}': {}", org, e);
                Err((org, e))
            }
        }
    })
    .await;

    let (all_clients, had_errors): (Vec<(String, Vec<OAuthClient>)>, bool) =
        collect_org_results(results, &spinner, "OAuth clients");

    finish_spinner_with_status(spinner, &all_clients, had_errors);

    if !all_clients.is_empty() {
        output_oauth_clients(&all_clients, cli);
    }

    log_completion(had_errors);
    Ok(())
}

/// Get a single OAuth client by ID
async fn get_single_oauth_client(
    client: &TfeClient,
    cli: &Cli,
    name: &str,
    org: Option<&String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Get {
        resource: GetResource::Oc(args),
    } = &cli.command
    else {
        unreachable!()
    };

    // If it's an ID (oc-...), we can fetch directly
    if name.starts_with("oc-") {
        let spinner = create_spinner(&format!("Fetching OAuth client '{}'...", name), cli.batch);

        match client.get_oauth_client(name).await {
            Ok((oauth_client, raw)) => {
                finish_spinner(spinner);

                // For JSON/YAML, return raw API response
                if matches!(args.output, OutputFormat::Json | OutputFormat::Yaml) {
                    output_raw(&raw, &args.output);
                    return Ok(());
                }

                let org_name = oauth_client
                    .organization_id()
                    .unwrap_or("unknown")
                    .to_string();
                let all_clients = vec![(org_name, vec![oauth_client])];
                output_oauth_clients(&all_clients, cli);
                return Ok(());
            }
            Err(e) => {
                finish_spinner(spinner);
                return Err(e.into());
            }
        }
    }

    // For name-based lookup, we need to search organizations
    let organizations = resolve_organizations(client, org).await?;

    let spinner = create_spinner(
        &format!(
            "Searching for OAuth client '{}' in {} organization(s)...",
            name,
            organizations.len()
        ),
        cli.batch,
    );

    // Search in all organizations IN PARALLEL
    let name_owned = name.to_string();
    let found = crate::hcp::helpers::search_first_in_orgs(&organizations, |org| {
        let oc_name = name_owned.clone();
        async move {
            match client.get_oauth_clients(&org).await {
                Ok(clients) => {
                    let found: Vec<_> = clients
                        .into_iter()
                        .filter(|c| c.name() == oc_name || c.id == oc_name)
                        .collect();
                    if found.is_empty() {
                        (org, None)
                    } else {
                        (org, Some(found))
                    }
                }
                _ => (org, None),
            }
        }
    })
    .await;

    if let Some((org_name, found)) = found {
        finish_spinner(spinner);

        // For JSON/YAML with name search, we need to fetch the raw JSON
        // (we only have the model from list, not raw JSON)
        if matches!(args.output, OutputFormat::Json | OutputFormat::Yaml) {
            // Fetch the first match by ID to get raw JSON
            if let Ok((_, raw)) = client.get_oauth_client(&found[0].id).await {
                output_raw(&raw, &args.output);
                return Ok(());
            }
        }

        let all_clients = vec![(org_name, found)];
        output_oauth_clients(&all_clients, cli);
        return Ok(());
    }

    finish_spinner(spinner);
    Err(crate::hcp::helpers::not_found_in_orgs_error("OAuth client", name, &organizations).into())
}
