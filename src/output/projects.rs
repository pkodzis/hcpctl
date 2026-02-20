//! Project output formatter

use super::common::escape_csv;
use crate::cli::{Cli, Command, GetResource, OutputFormat};
use crate::hcp::{Project, ProjectWorkspaces, TfeResource, Workspace};
use comfy_table::{presets::NOTHING, Table};
use serde::Serialize;

/// Project row type alias
pub type ProjectRow = (String, Project, ProjectWorkspaces);

/// Serializable workspace for structured output (JSON/YAML) - subset of fields
#[derive(Serialize)]
struct SerializableWorkspace {
    id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    terraform_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    execution_mode: Option<String>,
    resource_count: u32,
    locked: bool,
}

impl From<&Workspace> for SerializableWorkspace {
    fn from(ws: &Workspace) -> Self {
        SerializableWorkspace {
            id: ws.id.clone(),
            name: ws.attributes.name.clone(),
            terraform_version: ws.attributes.terraform_version.clone(),
            execution_mode: ws.attributes.execution_mode.clone(),
            resource_count: ws.attributes.resource_count.unwrap_or(0),
            locked: ws.attributes.locked.unwrap_or(false),
        }
    }
}

/// Serializable project for structured output (JSON/YAML)
#[derive(Serialize)]
struct SerializableProject {
    org: String,
    name: String,
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    workspace_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    workspaces: Option<Vec<SerializableWorkspace>>,
    description: String,
}

/// Output projects in the specified format
pub fn output_projects(projects: &[ProjectRow], cli: &Cli) {
    let Command::Get {
        resource: GetResource::Prj(args),
    } = &cli.command
    else {
        unreachable!()
    };

    let show_ws = args.with_ws || args.with_ws_names || args.with_ws_ids || args.with_ws_details;
    let show_names = args.with_ws_names;
    let show_ids = args.with_ws_ids;
    let show_details = args.with_ws_details;

    match args.output {
        OutputFormat::Table => output_table(
            projects,
            cli.no_header,
            show_ws,
            show_names,
            show_ids,
            show_details,
        ),
        OutputFormat::Csv => output_csv(
            projects,
            cli.no_header,
            show_ws,
            show_names,
            show_ids,
            show_details,
        ),
        OutputFormat::Json => output_json(projects, show_ws, show_details),
        OutputFormat::Yaml => output_yaml(projects, show_ws, show_details),
    }
}

fn output_table(
    projects: &[ProjectRow],
    no_header: bool,
    show_ws: bool,
    show_names: bool,
    show_ids: bool,
    show_details: bool,
) {
    let mut table = Table::new();
    table.load_preset(NOTHING);

    // Build header dynamically
    let mut headers = vec!["Org", "Name", "ID"];
    if show_ws {
        headers.push("Workspaces");
    }
    if show_names {
        headers.push("WS Names");
    }
    if show_ids {
        headers.push("WS IDs");
    }
    if show_details {
        headers.push("WS Details");
    }
    headers.push("Description");

    if !no_header {
        table.set_header(headers);
    }

    for (org_name, prj, ws_info) in projects {
        let mut row: Vec<String> = vec![org_name.clone(), prj.name().to_string(), prj.id.clone()];

        if show_ws {
            let ws_str = if ws_info.is_empty() && !show_names && !show_ids && !show_details {
                "-".to_string()
            } else {
                ws_info.count().to_string()
            };
            row.push(ws_str);
        }

        if show_names {
            row.push(ws_info.names().join(", "));
        }

        if show_ids {
            row.push(ws_info.ids().join(", "));
        }

        if show_details {
            row.push(ws_info.name_id_pairs().join(", "));
        }

        row.push(prj.description().to_string());

        table.add_row(row);
    }

    println!();
    println!("{table}");
    if !no_header {
        println!("\nTotal: {} projects", projects.len());
    }
}

fn output_csv(
    projects: &[ProjectRow],
    no_header: bool,
    show_ws: bool,
    show_names: bool,
    show_ids: bool,
    show_details: bool,
) {
    // Build header
    let mut headers = vec!["org", "name", "id"];
    if show_ws {
        headers.push("workspaces");
    }
    if show_names {
        headers.push("ws_names");
    }
    if show_ids {
        headers.push("ws_ids");
    }
    if show_details {
        headers.push("ws_details");
    }
    headers.push("description");

    if !no_header {
        println!("{}", headers.join(","));
    }

    for (org_name, prj, ws_info) in projects {
        let mut fields: Vec<String> = vec![
            escape_csv(org_name),
            escape_csv(prj.name()),
            escape_csv(&prj.id),
        ];

        if show_ws {
            fields.push(ws_info.count().to_string());
        }

        if show_names {
            // Encapsulate list as semicolon-separated within quotes
            let names_str = ws_info.names().join(";");
            fields.push(escape_csv(&names_str));
        }

        if show_ids {
            let ids_str = ws_info.ids().join(";");
            fields.push(escape_csv(&ids_str));
        }

        if show_details {
            let details_str = ws_info.name_id_pairs().join(";");
            fields.push(escape_csv(&details_str));
        }

        fields.push(escape_csv(prj.description()));

        println!("{}", fields.join(","));
    }
}

/// Build serializable project data (reusable for JSON and YAML)
fn build_serializable_projects(
    projects: &[ProjectRow],
    show_ws: bool,
    show_details: bool,
) -> Vec<SerializableProject> {
    projects
        .iter()
        .map(|(org_name, p, ws_info)| SerializableProject {
            org: org_name.clone(),
            name: p.name().to_string(),
            id: p.id.clone(),
            workspace_count: if show_ws { Some(ws_info.count()) } else { None },
            workspaces: if show_details {
                Some(
                    ws_info
                        .workspaces
                        .iter()
                        .map(SerializableWorkspace::from)
                        .collect(),
                )
            } else {
                None
            },
            description: p.description().to_string(),
        })
        .collect()
}

fn output_json(projects: &[ProjectRow], show_ws: bool, show_details: bool) {
    let data = build_serializable_projects(projects, show_ws, show_details);
    println!("{}", serde_json::to_string_pretty(&data).unwrap());
}

fn output_yaml(projects: &[ProjectRow], show_ws: bool, show_details: bool) {
    let data = build_serializable_projects(projects, show_ws, show_details);
    println!("{}", serde_yml::to_string(&data).unwrap());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hcp::{ProjectAttributes, WorkspaceAttributes};

    fn create_test_project() -> Project {
        Project {
            id: "prj-123".to_string(),
            project_type: Some("projects".to_string()),
            attributes: ProjectAttributes {
                name: "test-project".to_string(),
                description: Some("A test project".to_string()),
            },
        }
    }

    fn create_test_workspace(id: &str, name: &str) -> Workspace {
        Workspace {
            id: id.to_string(),
            attributes: WorkspaceAttributes {
                name: name.to_string(),
                execution_mode: Some("remote".to_string()),
                resource_count: Some(5),
                locked: Some(false),
                terraform_version: Some("1.5.0".to_string()),
                updated_at: None,
            },
            relationships: None,
        }
    }

    fn create_test_ws_info() -> ProjectWorkspaces {
        ProjectWorkspaces::from_workspaces(vec![
            create_test_workspace("ws-id-1", "ws-one"),
            create_test_workspace("ws-id-2", "ws-two"),
        ])
    }

    #[test]
    fn test_output_table_empty() {
        // Should not panic with empty input
        output_table(&[], false, false, false, false, false);
    }

    #[test]
    fn test_output_table() {
        let projects = vec![(
            "test-org".to_string(),
            create_test_project(),
            create_test_ws_info(),
        )];
        // Should not panic
        output_table(&projects, false, true, false, false, false);
    }

    #[test]
    fn test_output_table_no_ws() {
        let projects = vec![(
            "test-org".to_string(),
            create_test_project(),
            ProjectWorkspaces::new(),
        )];
        // Should not panic
        output_table(&projects, false, false, false, false, false);
    }

    #[test]
    fn test_output_table_with_names() {
        let projects = vec![(
            "test-org".to_string(),
            create_test_project(),
            create_test_ws_info(),
        )];
        // Should not panic
        output_table(&projects, false, true, true, false, false);
    }

    #[test]
    fn test_output_csv() {
        let projects = vec![(
            "test-org".to_string(),
            create_test_project(),
            create_test_ws_info(),
        )];
        // Should not panic
        output_csv(&projects, false, true, true, true, true);
    }

    #[test]
    fn test_output_json() {
        let projects = vec![(
            "test-org".to_string(),
            create_test_project(),
            create_test_ws_info(),
        )];
        // Should not panic
        output_json(&projects, true, true);
    }

    #[test]
    fn test_output_yaml() {
        let projects = vec![(
            "test-org".to_string(),
            create_test_project(),
            create_test_ws_info(),
        )];
        // Should not panic
        output_yaml(&projects, true, true);
    }

    #[test]
    fn test_output_no_header() {
        let projects = vec![(
            "test-org".to_string(),
            create_test_project(),
            create_test_ws_info(),
        )];
        // Should not panic
        output_table(&projects, true, true, false, false, false);
        output_csv(&projects, true, true, false, false, false);
    }
}
