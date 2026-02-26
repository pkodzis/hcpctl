//! Team access output formatter

use super::common::escape_csv;
use crate::cli::OutputFormat;
use crate::hcp::team_projects::EnrichedTeamProjectAccess;
use comfy_table::{presets::NOTHING, Table};
use serde::Serialize;

/// Serializable team access for structured output (JSON/YAML)
#[derive(Serialize)]
struct SerializableTeamAccess {
    id: String,
    team_id: String,
    team_name: String,
    project_id: String,
    project_name: String,
    access: String,
}

impl From<&EnrichedTeamProjectAccess> for SerializableTeamAccess {
    fn from(binding: &EnrichedTeamProjectAccess) -> Self {
        Self {
            id: binding.id.clone(),
            team_id: binding.team_id.clone(),
            team_name: binding.team_name.clone(),
            project_id: binding.project_id.clone(),
            project_name: binding.project_name.clone(),
            access: binding.access.clone(),
        }
    }
}

/// Output team access bindings in the specified format
pub fn output_team_access(
    bindings: &[EnrichedTeamProjectAccess],
    format: &OutputFormat,
    no_header: bool,
) {
    match format {
        OutputFormat::Table => output_table(bindings, no_header),
        OutputFormat::Csv => output_csv(bindings, no_header),
        OutputFormat::Json => output_json(bindings),
        OutputFormat::Yaml => output_yaml(bindings),
    }
}

fn output_table(bindings: &[EnrichedTeamProjectAccess], no_header: bool) {
    let mut table = Table::new();
    table.load_preset(NOTHING);
    if !no_header {
        table.set_header(vec!["ID", "TEAM", "PROJECT", "ACCESS"]);
    }

    for binding in bindings {
        table.add_row(vec![
            binding.id.as_str(),
            binding.team_name.as_str(),
            binding.project_name.as_str(),
            binding.access.as_str(),
        ]);
    }

    println!("{table}");
}

fn output_csv(bindings: &[EnrichedTeamProjectAccess], no_header: bool) {
    if !no_header {
        println!("ID,TEAM,PROJECT,ACCESS");
    }
    for binding in bindings {
        println!(
            "{},{},{},{}",
            escape_csv(&binding.id),
            escape_csv(&binding.team_name),
            escape_csv(&binding.project_name),
            escape_csv(&binding.access),
        );
    }
}

fn output_json(bindings: &[EnrichedTeamProjectAccess]) {
    let serializable: Vec<SerializableTeamAccess> =
        bindings.iter().map(SerializableTeamAccess::from).collect();
    super::common::print_json(&serializable);
}

fn output_yaml(bindings: &[EnrichedTeamProjectAccess]) {
    let serializable: Vec<SerializableTeamAccess> =
        bindings.iter().map(SerializableTeamAccess::from).collect();
    super::common::print_yaml(&serializable);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_binding(
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
    fn test_serializable_team_access() {
        let binding = create_test_binding("tprj-1", "owners", "my-project", "admin");
        let serializable = SerializableTeamAccess::from(&binding);

        assert_eq!(serializable.id, "tprj-1");
        assert_eq!(serializable.team_name, "owners");
        assert_eq!(serializable.project_name, "my-project");
        assert_eq!(serializable.access, "admin");
        assert_eq!(serializable.team_id, "team-owners");
        assert_eq!(serializable.project_id, "prj-my-project");
    }

    #[test]
    fn test_output_json_format() {
        let bindings = [create_test_binding("tprj-1", "devs", "infra", "write")];
        let serializable: Vec<SerializableTeamAccess> =
            bindings.iter().map(SerializableTeamAccess::from).collect();
        let json = serde_json::to_string_pretty(&serializable).unwrap();

        assert!(json.contains("\"id\": \"tprj-1\""));
        assert!(json.contains("\"team_name\": \"devs\""));
        assert!(json.contains("\"project_name\": \"infra\""));
        assert!(json.contains("\"access\": \"write\""));
    }

    #[test]
    fn test_output_yaml_format() {
        let bindings = [create_test_binding("tprj-1", "devs", "infra", "write")];
        let serializable: Vec<SerializableTeamAccess> =
            bindings.iter().map(SerializableTeamAccess::from).collect();
        let yaml = serde_yml::to_string(&serializable).unwrap();

        assert!(yaml.contains("id: tprj-1"));
        assert!(yaml.contains("team_name: devs"));
        assert!(yaml.contains("project_name: infra"));
        assert!(yaml.contains("access: write"));
    }

    #[test]
    fn test_serializable_team_access_multiple() {
        let bindings = [
            create_test_binding("tprj-1", "owners", "infra", "admin"),
            create_test_binding("tprj-2", "devs", "app", "read"),
        ];
        let serializable: Vec<SerializableTeamAccess> =
            bindings.iter().map(SerializableTeamAccess::from).collect();

        assert_eq!(serializable.len(), 2);
        assert_eq!(serializable[0].id, "tprj-1");
        assert_eq!(serializable[0].team_name, "owners");
        assert_eq!(serializable[1].id, "tprj-2");
        assert_eq!(serializable[1].team_name, "devs");
    }

    #[test]
    fn test_output_json_multiple_items() {
        let bindings = [
            create_test_binding("tprj-1", "owners", "infra", "admin"),
            create_test_binding("tprj-2", "devs", "app", "write"),
        ];
        let serializable: Vec<SerializableTeamAccess> =
            bindings.iter().map(SerializableTeamAccess::from).collect();
        let json = serde_json::to_string_pretty(&serializable).unwrap();

        assert!(json.contains("\"id\": \"tprj-1\""));
        assert!(json.contains("\"id\": \"tprj-2\""));
        assert!(json.contains("\"team_name\": \"owners\""));
        assert!(json.contains("\"team_name\": \"devs\""));
    }

    #[test]
    fn test_output_csv_escaping() {
        let binding = create_test_binding("tprj-1", "team,with,commas", "project\"quoted", "admin");
        let serializable = SerializableTeamAccess::from(&binding);

        // Verify the field values that would need escaping in CSV
        assert_eq!(serializable.team_name, "team,with,commas");
        assert_eq!(serializable.project_name, "project\"quoted");
    }

    #[test]
    fn test_serializable_preserves_ids() {
        let binding = create_test_binding("tprj-abc", "owners", "infra", "admin");
        let serializable = SerializableTeamAccess::from(&binding);

        assert_eq!(serializable.team_id, "team-owners");
        assert_eq!(serializable.project_id, "prj-infra");
    }
}
