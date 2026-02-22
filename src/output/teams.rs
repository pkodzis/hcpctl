//! Team output formatter

use super::common::escape_csv;
use crate::cli::{Cli, Command, GetResource, OutputFormat};
use crate::hcp::teams::Team;
use comfy_table::{presets::NOTHING, Table};
use serde::Serialize;

/// Serializable team for structured output (JSON/YAML)
#[derive(Serialize)]
struct SerializableTeam {
    id: String,
    name: String,
    users_count: u32,
    visibility: String,
}

impl From<&Team> for SerializableTeam {
    fn from(team: &Team) -> Self {
        Self {
            id: team.id.clone(),
            name: team.name().to_string(),
            users_count: team.users_count(),
            visibility: team.visibility().to_string(),
        }
    }
}

/// Output teams in the specified format
pub fn output_teams(teams: &[Team], cli: &Cli) {
    let Command::Get {
        resource: GetResource::Team(args),
    } = &cli.command
    else {
        unreachable!()
    };

    match args.output {
        OutputFormat::Table => output_table(teams, cli.no_header),
        OutputFormat::Csv => output_csv(teams, cli.no_header),
        OutputFormat::Json => output_json(teams),
        OutputFormat::Yaml => output_yaml(teams),
    }
}

fn output_table(teams: &[Team], no_header: bool) {
    let mut table = Table::new();
    table.load_preset(NOTHING);
    if !no_header {
        table.set_header(vec!["ID", "NAME", "USERS", "VISIBILITY"]);
    }

    for team in teams {
        table.add_row(vec![
            team.id.as_str(),
            team.name(),
            &team.users_count().to_string(),
            team.visibility(),
        ]);
    }

    println!("{table}");
}

fn output_csv(teams: &[Team], no_header: bool) {
    if !no_header {
        println!("ID,NAME,USERS,VISIBILITY");
    }
    for team in teams {
        println!(
            "{},{},{},{}",
            escape_csv(&team.id),
            escape_csv(team.name()),
            team.users_count(),
            escape_csv(team.visibility())
        );
    }
}

fn output_json(teams: &[Team]) {
    let serializable: Vec<SerializableTeam> = teams.iter().map(SerializableTeam::from).collect();
    super::common::print_json(&serializable);
}

fn output_yaml(teams: &[Team]) {
    let serializable: Vec<SerializableTeam> = teams.iter().map(SerializableTeam::from).collect();
    super::common::print_yaml(&serializable);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_team(id: &str, name: &str, users: u32, visibility: &str) -> Team {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "type": "teams",
            "attributes": {
                "name": name,
                "users-count": users,
                "visibility": visibility
            }
        }))
        .unwrap()
    }

    #[test]
    fn test_serializable_team() {
        let team = create_test_team("team-abc", "owners", 5, "organization");
        let serializable = SerializableTeam::from(&team);

        assert_eq!(serializable.id, "team-abc");
        assert_eq!(serializable.name, "owners");
        assert_eq!(serializable.users_count, 5);
        assert_eq!(serializable.visibility, "organization");
    }

    #[test]
    fn test_output_json_format() {
        let teams = [create_test_team("team-1", "devs", 3, "secret")];
        let serializable: Vec<SerializableTeam> =
            teams.iter().map(SerializableTeam::from).collect();
        let json = serde_json::to_string_pretty(&serializable).unwrap();

        assert!(json.contains("\"id\": \"team-1\""));
        assert!(json.contains("\"name\": \"devs\""));
        assert!(json.contains("\"users_count\": 3"));
        assert!(json.contains("\"visibility\": \"secret\""));
    }

    #[test]
    fn test_output_yaml_format() {
        let teams = [create_test_team("team-1", "devs", 3, "secret")];
        let serializable: Vec<SerializableTeam> =
            teams.iter().map(SerializableTeam::from).collect();
        let yaml = serde_yml::to_string(&serializable).unwrap();

        assert!(yaml.contains("id: team-1"));
        assert!(yaml.contains("name: devs"));
        assert!(yaml.contains("users_count: 3"));
        assert!(yaml.contains("visibility: secret"));
    }
}
