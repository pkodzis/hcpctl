//! Tag binding and organization tag output formatter

use super::common::escape_csv;
use crate::cli::OutputFormat;
use crate::hcp::tags::{OrgTag, TagBinding};
use crate::hcp::traits::TfeResource;
use crate::hcp::Workspace;
use comfy_table::{presets::NOTHING, Table};
use serde::Serialize;

/// Serializable tag binding for structured output (JSON/YAML)
#[derive(Serialize)]
struct SerializableTagBinding {
    key: String,
    value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    created_at: Option<String>,
}

impl From<&TagBinding> for SerializableTagBinding {
    fn from(tag: &TagBinding) -> Self {
        Self {
            key: tag.attributes.key.clone(),
            value: tag.attributes.value.clone(),
            created_at: tag.attributes.created_at.clone(),
        }
    }
}

/// Output tag bindings in the specified format
pub fn output_tag_bindings(tags: &[TagBinding], format: &OutputFormat, no_header: bool) {
    match format {
        OutputFormat::Table => output_table(tags, no_header),
        OutputFormat::Csv => output_csv(tags, no_header),
        OutputFormat::Json => output_json(tags),
        OutputFormat::Yaml => output_yaml(tags),
    }
}

fn output_table(tags: &[TagBinding], no_header: bool) {
    let mut table = Table::new();
    table.load_preset(NOTHING);
    if !no_header {
        table.set_header(vec!["Key", "Value", "Created At"]);
    }

    for tag in tags {
        table.add_row(vec![
            &tag.attributes.key,
            &tag.attributes.value,
            tag.attributes.created_at.as_deref().unwrap_or(""),
        ]);
    }

    println!();
    println!("{table}");
    if !no_header {
        println!("\nTotal: {} tag(s)", tags.len());
    }
}

fn output_csv(tags: &[TagBinding], no_header: bool) {
    if !no_header {
        println!("key,value,created_at");
    }

    for tag in tags {
        println!(
            "{},{},{}",
            escape_csv(&tag.attributes.key),
            escape_csv(&tag.attributes.value),
            escape_csv(tag.attributes.created_at.as_deref().unwrap_or(""))
        );
    }
}

fn output_json(tags: &[TagBinding]) {
    let data: Vec<SerializableTagBinding> = tags.iter().map(SerializableTagBinding::from).collect();
    super::common::print_json(&data);
}

fn output_yaml(tags: &[TagBinding]) {
    let data: Vec<SerializableTagBinding> = tags.iter().map(SerializableTagBinding::from).collect();
    super::common::print_yaml(&data);
}

// === Organization-level tag output ===

/// Serializable org tag for structured output (JSON/YAML)
#[derive(Serialize)]
struct SerializableOrgTag {
    name: String,
    instance_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    created_at: Option<String>,
}

impl From<&OrgTag> for SerializableOrgTag {
    fn from(tag: &OrgTag) -> Self {
        Self {
            name: tag.attributes.name.clone(),
            instance_count: tag.attributes.instance_count,
            created_at: tag.attributes.created_at.clone(),
        }
    }
}

/// Output organization tags in the specified format
pub fn output_org_tags(tags: &[OrgTag], format: &OutputFormat, no_header: bool) {
    match format {
        OutputFormat::Table => output_org_table(tags, no_header),
        OutputFormat::Csv => output_org_csv(tags, no_header),
        OutputFormat::Json => output_org_json(tags),
        OutputFormat::Yaml => output_org_yaml(tags),
    }
}

fn output_org_table(tags: &[OrgTag], no_header: bool) {
    let mut table = Table::new();
    table.load_preset(NOTHING);
    if !no_header {
        table.set_header(vec!["Name", "Instance Count", "Created At"]);
    }

    for tag in tags {
        table.add_row(vec![
            &tag.attributes.name,
            &tag.attributes.instance_count.to_string(),
            tag.attributes.created_at.as_deref().unwrap_or(""),
        ]);
    }

    println!();
    println!("{table}");
    if !no_header {
        println!("\nTotal: {} tag(s)", tags.len());
    }
}

fn output_org_csv(tags: &[OrgTag], no_header: bool) {
    if !no_header {
        println!("name,instance_count,created_at");
    }

    for tag in tags {
        println!(
            "{},{},{}",
            escape_csv(&tag.attributes.name),
            tag.attributes.instance_count,
            escape_csv(tag.attributes.created_at.as_deref().unwrap_or(""))
        );
    }
}

fn output_org_json(tags: &[OrgTag]) {
    let data: Vec<SerializableOrgTag> = tags.iter().map(SerializableOrgTag::from).collect();
    super::common::print_json(&data);
}

fn output_org_yaml(tags: &[OrgTag]) {
    let data: Vec<SerializableOrgTag> = tags.iter().map(SerializableOrgTag::from).collect();
    super::common::print_yaml(&data);
}

// === Organization-level tag detail output (with associated workspaces) ===

/// Serializable org tag with associated workspaces for structured output
#[derive(Serialize)]
struct SerializableOrgTagDetail {
    name: String,
    instance_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    created_at: Option<String>,
    workspaces: Vec<String>,
}

impl SerializableOrgTagDetail {
    fn from_tag_and_workspaces(tag: &OrgTag, workspaces: &[Workspace]) -> Self {
        Self {
            name: tag.attributes.name.clone(),
            instance_count: tag.attributes.instance_count,
            created_at: tag.attributes.created_at.clone(),
            workspaces: workspaces.iter().map(|ws| ws.name().to_string()).collect(),
        }
    }
}

/// Output organization tags with associated workspaces
pub fn output_org_tags_with_workspaces(
    tags: &[OrgTag],
    workspaces: &[Workspace],
    format: &OutputFormat,
    no_header: bool,
) {
    match format {
        OutputFormat::Table => {
            output_org_table(tags, no_header);
            output_associated_workspaces_table(workspaces);
        }
        OutputFormat::Csv => {
            output_org_csv(tags, no_header);
            // Workspaces in a separate CSV section
            if !workspaces.is_empty() {
                println!();
                if !no_header {
                    println!("workspace_name,workspace_id");
                }
                for ws in workspaces {
                    println!("{},{}", escape_csv(ws.name()), escape_csv(ws.id()));
                }
            }
        }
        OutputFormat::Json => {
            let data: Vec<SerializableOrgTagDetail> = tags
                .iter()
                .map(|t| SerializableOrgTagDetail::from_tag_and_workspaces(t, workspaces))
                .collect();
            super::common::print_json(&data);
        }
        OutputFormat::Yaml => {
            let data: Vec<SerializableOrgTagDetail> = tags
                .iter()
                .map(|t| SerializableOrgTagDetail::from_tag_and_workspaces(t, workspaces))
                .collect();
            super::common::print_yaml(&data);
        }
    }
}

fn output_associated_workspaces_table(workspaces: &[Workspace]) {
    if workspaces.is_empty() {
        return;
    }

    println!("\nAssociated workspaces:");

    let mut table = Table::new();
    table.load_preset(NOTHING);
    table.set_header(vec!["Workspace", "ID"]);

    for ws in workspaces {
        table.add_row(vec![ws.name(), ws.id()]);
    }

    println!("{table}");
}

// === Workspace combined tags output (flat string tags + key-value tag bindings) ===

/// Serializable combined workspace tags for structured output (JSON/YAML)
#[derive(Serialize)]
struct SerializableWorkspaceAllTags {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tag_bindings: Vec<SerializableTagBinding>,
}

/// Output both flat string tags and key-value tag bindings for a workspace
pub fn output_workspace_all_tags(
    workspace_tags: &[OrgTag],
    tag_bindings: &[TagBinding],
    format: &OutputFormat,
    no_header: bool,
) {
    match format {
        OutputFormat::Table => {
            if !workspace_tags.is_empty() {
                println!("\nTags:");
                let mut table = Table::new();
                table.load_preset(NOTHING);
                for tag in workspace_tags {
                    table.add_row(vec![&tag.attributes.name]);
                }
                println!("{table}");
            }
            if !tag_bindings.is_empty() {
                println!("\nTag bindings:");
                let mut table = Table::new();
                table.load_preset(NOTHING);
                if !no_header {
                    table.set_header(vec!["Key", "Value", "Created At"]);
                }
                for tag in tag_bindings {
                    table.add_row(vec![
                        &tag.attributes.key,
                        &tag.attributes.value,
                        tag.attributes.created_at.as_deref().unwrap_or(""),
                    ]);
                }
                println!("{table}");
            }
            if !no_header {
                println!(
                    "\nTotal: {} tag(s), {} tag binding(s)",
                    workspace_tags.len(),
                    tag_bindings.len()
                );
            }
        }
        OutputFormat::Csv => {
            if !workspace_tags.is_empty() {
                if !no_header {
                    println!("tag_name");
                }
                for tag in workspace_tags {
                    println!("{}", escape_csv(&tag.attributes.name));
                }
            }
            if !tag_bindings.is_empty() {
                if !workspace_tags.is_empty() {
                    println!();
                }
                output_csv(tag_bindings, no_header);
            }
        }
        OutputFormat::Json => {
            let data = SerializableWorkspaceAllTags {
                tags: workspace_tags
                    .iter()
                    .map(|t| t.attributes.name.clone())
                    .collect(),
                tag_bindings: tag_bindings
                    .iter()
                    .map(SerializableTagBinding::from)
                    .collect(),
            };
            println!("{}", serde_json::to_string_pretty(&data).unwrap());
        }
        OutputFormat::Yaml => {
            let data = SerializableWorkspaceAllTags {
                tags: workspace_tags
                    .iter()
                    .map(|t| t.attributes.name.clone())
                    .collect(),
                tag_bindings: tag_bindings
                    .iter()
                    .map(SerializableTagBinding::from)
                    .collect(),
            };
            println!("{}", serde_yml::to_string(&data).unwrap());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hcp::tags::{OrgTagAttributes, TagBindingAttributes};

    fn create_test_tags() -> Vec<TagBinding> {
        vec![
            TagBinding {
                id: "tb-1".to_string(),
                binding_type: "tag-bindings".to_string(),
                attributes: TagBindingAttributes {
                    key: "env".to_string(),
                    value: "prod".to_string(),
                    created_at: Some("2024-01-01T00:00:00Z".to_string()),
                },
            },
            TagBinding {
                id: "tb-2".to_string(),
                binding_type: "tag-bindings".to_string(),
                attributes: TagBindingAttributes {
                    key: "team".to_string(),
                    value: "backend".to_string(),
                    created_at: None,
                },
            },
        ]
    }

    #[test]
    fn test_output_tag_bindings_empty() {
        output_tag_bindings(&[], &OutputFormat::Table, false);
        output_tag_bindings(&[], &OutputFormat::Csv, false);
        output_tag_bindings(&[], &OutputFormat::Json, false);
        output_tag_bindings(&[], &OutputFormat::Yaml, false);
    }

    #[test]
    fn test_output_tag_bindings_table() {
        let tags = create_test_tags();
        output_tag_bindings(&tags, &OutputFormat::Table, false);
    }

    #[test]
    fn test_output_tag_bindings_csv() {
        let tags = create_test_tags();
        output_tag_bindings(&tags, &OutputFormat::Csv, false);
    }

    #[test]
    fn test_output_tag_bindings_json() {
        let tags = create_test_tags();
        output_tag_bindings(&tags, &OutputFormat::Json, false);
    }

    #[test]
    fn test_output_tag_bindings_yaml() {
        let tags = create_test_tags();
        output_tag_bindings(&tags, &OutputFormat::Yaml, false);
    }

    #[test]
    fn test_output_tag_bindings_no_header() {
        let tags = create_test_tags();
        output_tag_bindings(&tags, &OutputFormat::Table, true);
        output_tag_bindings(&tags, &OutputFormat::Csv, true);
    }

    #[test]
    fn test_serializable_from_tag() {
        let tag = TagBinding {
            id: "tb-1".to_string(),
            binding_type: "tag-bindings".to_string(),
            attributes: TagBindingAttributes {
                key: "env".to_string(),
                value: "prod".to_string(),
                created_at: Some("2024-01-01T00:00:00Z".to_string()),
            },
        };
        let s = SerializableTagBinding::from(&tag);
        assert_eq!(s.key, "env");
        assert_eq!(s.value, "prod");
        assert_eq!(s.created_at, Some("2024-01-01T00:00:00Z".to_string()));
    }

    // === Org tag output tests ===

    fn create_test_org_tags() -> Vec<OrgTag> {
        vec![
            OrgTag {
                id: "tag-1".to_string(),
                tag_type: "tags".to_string(),
                attributes: OrgTagAttributes {
                    name: "env".to_string(),
                    instance_count: 5,
                    created_at: Some("2024-01-01T00:00:00Z".to_string()),
                },
            },
            OrgTag {
                id: "tag-2".to_string(),
                tag_type: "tags".to_string(),
                attributes: OrgTagAttributes {
                    name: "team".to_string(),
                    instance_count: 3,
                    created_at: None,
                },
            },
        ]
    }

    #[test]
    fn test_output_org_tags_table() {
        let tags = create_test_org_tags();
        output_org_tags(&tags, &OutputFormat::Table, false);
    }

    #[test]
    fn test_output_org_tags_csv() {
        let tags = create_test_org_tags();
        output_org_tags(&tags, &OutputFormat::Csv, false);
    }

    #[test]
    fn test_output_org_tags_json() {
        let tags = create_test_org_tags();
        output_org_tags(&tags, &OutputFormat::Json, false);
    }

    #[test]
    fn test_output_org_tags_yaml() {
        let tags = create_test_org_tags();
        output_org_tags(&tags, &OutputFormat::Yaml, false);
    }

    #[test]
    fn test_output_org_tags_empty() {
        output_org_tags(&[], &OutputFormat::Table, false);
    }

    #[test]
    fn test_output_org_tags_no_header() {
        let tags = create_test_org_tags();
        output_org_tags(&tags, &OutputFormat::Table, true);
        output_org_tags(&tags, &OutputFormat::Csv, true);
    }

    #[test]
    fn test_serializable_from_org_tag() {
        let tag = OrgTag {
            id: "tag-1".to_string(),
            tag_type: "tags".to_string(),
            attributes: OrgTagAttributes {
                name: "env".to_string(),
                instance_count: 5,
                created_at: Some("2024-01-01T00:00:00Z".to_string()),
            },
        };
        let s = SerializableOrgTag::from(&tag);
        assert_eq!(s.name, "env");
        assert_eq!(s.instance_count, 5);
        assert_eq!(s.created_at, Some("2024-01-01T00:00:00Z".to_string()));
    }

    // === Org tag with workspaces output tests ===

    fn create_test_workspaces() -> Vec<Workspace> {
        use crate::hcp::workspaces::WorkspaceAttributes;
        vec![
            Workspace {
                id: "ws-abc".to_string(),
                attributes: WorkspaceAttributes {
                    name: "alpha-ws".to_string(),
                    execution_mode: None,
                    resource_count: None,
                    locked: None,
                    terraform_version: None,
                    updated_at: None,
                },
                relationships: None,
            },
            Workspace {
                id: "ws-def".to_string(),
                attributes: WorkspaceAttributes {
                    name: "beta-ws".to_string(),
                    execution_mode: None,
                    resource_count: None,
                    locked: None,
                    terraform_version: None,
                    updated_at: None,
                },
                relationships: None,
            },
        ]
    }

    #[test]
    fn test_output_org_tags_with_workspaces_table() {
        let tags = create_test_org_tags();
        let workspaces = create_test_workspaces();
        output_org_tags_with_workspaces(&tags, &workspaces, &OutputFormat::Table, false);
    }

    #[test]
    fn test_output_org_tags_with_workspaces_json() {
        let tags = create_test_org_tags();
        let workspaces = create_test_workspaces();
        output_org_tags_with_workspaces(&tags, &workspaces, &OutputFormat::Json, false);
    }

    #[test]
    fn test_output_org_tags_with_workspaces_yaml() {
        let tags = create_test_org_tags();
        let workspaces = create_test_workspaces();
        output_org_tags_with_workspaces(&tags, &workspaces, &OutputFormat::Yaml, false);
    }

    #[test]
    fn test_output_org_tags_with_workspaces_csv() {
        let tags = create_test_org_tags();
        let workspaces = create_test_workspaces();
        output_org_tags_with_workspaces(&tags, &workspaces, &OutputFormat::Csv, false);
    }

    #[test]
    fn test_output_org_tags_with_empty_workspaces() {
        let tags = create_test_org_tags();
        output_org_tags_with_workspaces(&tags, &[], &OutputFormat::Table, false);
    }

    #[test]
    fn test_serializable_org_tag_detail() {
        let tag = OrgTag {
            id: "tag-1".to_string(),
            tag_type: "tags".to_string(),
            attributes: OrgTagAttributes {
                name: "env".to_string(),
                instance_count: 2,
                created_at: Some("2024-01-01T00:00:00Z".to_string()),
            },
        };
        let workspaces = create_test_workspaces();
        let detail = SerializableOrgTagDetail::from_tag_and_workspaces(&tag, &workspaces);
        assert_eq!(detail.name, "env");
        assert_eq!(detail.instance_count, 2);
        assert_eq!(detail.workspaces, vec!["alpha-ws", "beta-ws"]);
    }

    // === Workspace combined tags output tests ===

    #[test]
    fn test_output_workspace_all_tags_table() {
        let ws_tags = create_test_org_tags();
        let bindings = create_test_tags();
        output_workspace_all_tags(&ws_tags, &bindings, &OutputFormat::Table, false);
    }

    #[test]
    fn test_output_workspace_all_tags_json() {
        let ws_tags = create_test_org_tags();
        let bindings = create_test_tags();
        output_workspace_all_tags(&ws_tags, &bindings, &OutputFormat::Json, false);
    }

    #[test]
    fn test_output_workspace_all_tags_yaml() {
        let ws_tags = create_test_org_tags();
        let bindings = create_test_tags();
        output_workspace_all_tags(&ws_tags, &bindings, &OutputFormat::Yaml, false);
    }

    #[test]
    fn test_output_workspace_all_tags_csv() {
        let ws_tags = create_test_org_tags();
        let bindings = create_test_tags();
        output_workspace_all_tags(&ws_tags, &bindings, &OutputFormat::Csv, false);
    }

    #[test]
    fn test_output_workspace_all_tags_only_flat_tags() {
        let ws_tags = create_test_org_tags();
        output_workspace_all_tags(&ws_tags, &[], &OutputFormat::Table, false);
        output_workspace_all_tags(&ws_tags, &[], &OutputFormat::Json, false);
        output_workspace_all_tags(&ws_tags, &[], &OutputFormat::Yaml, false);
    }

    #[test]
    fn test_output_workspace_all_tags_only_bindings() {
        let bindings = create_test_tags();
        output_workspace_all_tags(&[], &bindings, &OutputFormat::Table, false);
        output_workspace_all_tags(&[], &bindings, &OutputFormat::Json, false);
    }

    #[test]
    fn test_serializable_workspace_all_tags() {
        let data = SerializableWorkspaceAllTags {
            tags: vec!["model__env".to_string(), "team-infra".to_string()],
            tag_bindings: vec![SerializableTagBinding {
                key: "env".to_string(),
                value: "prod".to_string(),
                created_at: None,
            }],
        };
        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("model__env"));
        assert!(json.contains("team-infra"));
        assert!(json.contains("\"key\":\"env\""));
    }
}
