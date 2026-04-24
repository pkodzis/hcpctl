//! State version output formatter

use super::common::escape_csv;
use crate::cli::OutputFormat;
use crate::hcp::state::StateVersionListItem;
use comfy_table::{presets::NOTHING, Table};
use serde::Serialize;

/// Serializable state version for structured output (JSON/YAML)
#[derive(Serialize)]
struct SerializableStateVersion {
    id: String,
    serial: Option<u64>,
    status: String,
    created_at: String,
    size: Option<u64>,
    resources: Option<u64>,
    delta_resources: Option<i64>,
    terraform_version: String,
    run_id: String,
    vcs_commit_sha: String,
}

/// Output state versions in the specified format
pub fn output_state_versions(
    states: &[StateVersionListItem],
    deltas: &[Option<i64>],
    format: &OutputFormat,
    no_header: bool,
) {
    match format {
        OutputFormat::Table => output_table(states, deltas, no_header),
        OutputFormat::Csv => output_csv(states, deltas, no_header),
        OutputFormat::Json => output_json(states, deltas),
        OutputFormat::Yaml => output_yaml(states, deltas),
    }
}

fn output_table(states: &[StateVersionListItem], deltas: &[Option<i64>], no_header: bool) {
    let mut table = Table::new();
    table.load_preset(NOTHING);
    if !no_header {
        table.set_header(vec![
            "ID",
            "SERIAL",
            "STATUS",
            "CREATED",
            "SIZE",
            "RESOURCES",
            "Δ RES",
            "TF VER",
            "RUN ID",
            "VCS SHA",
        ]);
    }

    for (i, state) in states.iter().enumerate() {
        let serial = state
            .attributes
            .serial
            .map(|s| s.to_string())
            .unwrap_or_else(|| "-".to_string());
        let status = state.attributes.status.as_deref().unwrap_or("-");
        let created = state.attributes.created_at.as_deref().unwrap_or("-");
        let resources = state
            .resource_count()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "-".to_string());
        let tf_ver = state.attributes.terraform_version.as_deref().unwrap_or("-");
        let delta = format_delta(deltas.get(i).copied().flatten());

        table.add_row(vec![
            &state.id,
            &serial,
            status,
            created,
            &state.size_human(),
            &resources,
            &delta,
            tf_ver,
            state.run_id(),
            state.vcs_sha_short(),
        ]);
    }

    println!();
    println!("{table}");
    if !no_header {
        println!("\nTotal: {} state versions", states.len());
    }
}

fn output_csv(states: &[StateVersionListItem], deltas: &[Option<i64>], no_header: bool) {
    if !no_header {
        println!("id,serial,status,created_at,size,resources,delta_resources,terraform_version,run_id,vcs_commit_sha");
    }

    for (i, state) in states.iter().enumerate() {
        let serial = state
            .attributes
            .serial
            .map(|s| s.to_string())
            .unwrap_or_default();
        let status = state.attributes.status.as_deref().unwrap_or("");
        let created = state.attributes.created_at.as_deref().unwrap_or("");
        let size = state
            .attributes
            .size
            .map(|s| s.to_string())
            .unwrap_or_default();
        let resources = state
            .resource_count()
            .map(|c| c.to_string())
            .unwrap_or_default();
        let delta = deltas
            .get(i)
            .copied()
            .flatten()
            .map(|d| d.to_string())
            .unwrap_or_default();
        let tf_ver = state.attributes.terraform_version.as_deref().unwrap_or("");
        let vcs = state.attributes.vcs_commit_sha.as_deref().unwrap_or("");

        println!(
            "{},{},{},{},{},{},{},{},{},{}",
            escape_csv(&state.id),
            escape_csv(&serial),
            escape_csv(status),
            escape_csv(created),
            escape_csv(&size),
            escape_csv(&resources),
            escape_csv(&delta),
            escape_csv(tf_ver),
            escape_csv(state.run_id()),
            escape_csv(vcs),
        );
    }
}

fn output_json(states: &[StateVersionListItem], deltas: &[Option<i64>]) {
    let data: Vec<SerializableStateVersion> = states
        .iter()
        .enumerate()
        .map(|(i, s)| to_serializable(s, deltas.get(i).copied().flatten()))
        .collect();
    super::common::print_json(&data);
}

fn output_yaml(states: &[StateVersionListItem], deltas: &[Option<i64>]) {
    let data: Vec<SerializableStateVersion> = states
        .iter()
        .enumerate()
        .map(|(i, s)| to_serializable(s, deltas.get(i).copied().flatten()))
        .collect();
    super::common::print_yaml(&data);
}

fn to_serializable(state: &StateVersionListItem, delta: Option<i64>) -> SerializableStateVersion {
    SerializableStateVersion {
        id: state.id.clone(),
        serial: state.attributes.serial,
        status: state.attributes.status.clone().unwrap_or_default(),
        created_at: state.attributes.created_at.clone().unwrap_or_default(),
        size: state.attributes.size,
        resources: state.resource_count(),
        delta_resources: delta,
        terraform_version: state
            .attributes
            .terraform_version
            .clone()
            .unwrap_or_default(),
        run_id: state.run_id().to_string(),
        vcs_commit_sha: state.attributes.vcs_commit_sha.clone().unwrap_or_default(),
    }
}

fn format_delta(delta: Option<i64>) -> String {
    match delta {
        None => "-".to_string(),
        Some(0) => "0".to_string(),
        Some(d) if d > 0 => format!("+{}", d),
        Some(d) => d.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_delta() {
        assert_eq!(format_delta(None), "-");
        assert_eq!(format_delta(Some(0)), "0");
        assert_eq!(format_delta(Some(5)), "+5");
        assert_eq!(format_delta(Some(-3)), "-3");
    }
}
