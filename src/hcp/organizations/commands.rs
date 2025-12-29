//! Organization command handlers

use log::debug;

use crate::error::Result;
use crate::hcp::traits::TfeResource;
use crate::hcp::TfeClient;
use crate::output::output_organizations;
use crate::ui::{create_spinner, finish_spinner};
use crate::{Cli, Command, GetResource};

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

    let spinner = create_spinner("Fetching organizations...", cli.quiet);
    let mut organizations = client.get_organizations_full().await?;
    finish_spinner(spinner, "Done");

    // If NAME is specified, filter to that single org
    if let Some(name) = &args.name {
        organizations.retain(|org| org.matches(name));

        if organizations.is_empty() {
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

    output_organizations(&organizations, cli);
    Ok(())
}
