//! Organization command handlers

use futures::future::join_all;
use log::debug;

use crate::cli::OutputFormat;
use crate::error::Result;
use crate::hcp::oauth_clients::OAuthToken;
use crate::hcp::traits::TfeResource;
use crate::hcp::TfeClient;
use crate::output::{output_organizations, output_raw};
use crate::ui::{create_spinner, finish_spinner};
use crate::{Cli, Command, GetResource};

use super::Organization;

/// Organization with its OAuth tokens
pub struct OrganizationWithTokens {
    pub organization: Organization,
    pub oauth_tokens: Vec<OAuthToken>,
}

impl OrganizationWithTokens {
    /// Get OAuth token IDs as strings
    pub fn oauth_token_ids(&self) -> Vec<&str> {
        self.oauth_tokens.iter().map(|t| t.id.as_str()).collect()
    }
}

/// Resolve organizations - either use the specified one or fetch all
pub async fn resolve_organizations(
    client: &TfeClient,
    org: Option<&String>,
) -> Result<Vec<String>> {
    match org {
        Some(org) => {
            debug!("Using specified organization: {}", org);
            Ok(vec![org.clone()])
        }
        None => {
            debug!("Fetching all organizations");
            client.get_organizations().await
        }
    }
}

/// Run the org list command
pub async fn run_org_command(
    client: &TfeClient,
    cli: &Cli,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let Command::Get {
        resource: GetResource::Org(args),
    } = &cli.command
    else {
        unreachable!()
    };

    debug!("Fetching organizations");

    let spinner = create_spinner("Fetching organizations...", cli.batch);

    // If NAME is specified and output is JSON/YAML, use direct API call for raw output
    if let Some(name) = &args.name {
        if matches!(args.output, OutputFormat::Json | OutputFormat::Yaml) {
            // Direct API call - returns raw JSON
            match client.get_organization(name).await? {
                Some((_org, raw)) => {
                    finish_spinner(spinner);
                    output_raw(&raw, &args.output);
                    return Ok(());
                }
                None => {
                    finish_spinner(spinner);
                    return Err(format!("Organization '{}' not found", name).into());
                }
            }
        }
    }

    // For lists or table/csv output, use the full fetch approach
    let mut organizations = client.get_organizations_full().await?;

    // If NAME is specified, filter to that single org
    if let Some(name) = &args.name {
        organizations.retain(|org| org.matches(name));

        if organizations.is_empty() {
            finish_spinner(spinner);
            return Err(format!("Organization '{}' not found", name).into());
        }
    }

    // Apply filter if specified
    if let Some(filter) = &args.filter {
        let filter_lower = filter.to_lowercase();
        organizations.retain(|org| org.name().to_lowercase().contains(&filter_lower));
        debug!(
            "Filtered to {} organizations matching '{}'",
            organizations.len(),
            filter
        );
    }

    // Fetch OAuth tokens for all organizations in parallel
    let token_futures: Vec<_> = organizations
        .iter()
        .map(|org| {
            let org_name = org.name().to_string();
            async move {
                let tokens = client.get_oauth_tokens_for_org(&org_name).await;
                (org_name, tokens)
            }
        })
        .collect();

    let token_results = join_all(token_futures).await;

    // Build map of org name -> tokens
    let mut token_map: std::collections::HashMap<String, Vec<OAuthToken>> =
        std::collections::HashMap::new();
    for (org_name, result) in token_results {
        if let Ok(tokens) = result {
            token_map.insert(org_name, tokens);
        }
    }

    finish_spinner(spinner);

    // Combine organizations with their tokens
    let orgs_with_tokens: Vec<OrganizationWithTokens> = organizations
        .into_iter()
        .map(|org| {
            let tokens = token_map.remove(org.name()).unwrap_or_default();
            OrganizationWithTokens {
                organization: org,
                oauth_tokens: tokens,
            }
        })
        .collect();

    output_organizations(&orgs_with_tokens, cli);
    Ok(())
}
