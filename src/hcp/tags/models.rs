//! Tag binding and organization tag data models

use serde::Deserialize;

use crate::hcp::traits::TfeResource;

/// Response wrapper for tag bindings list
#[derive(Deserialize, Debug)]
pub struct TagBindingsResponse {
    pub data: Vec<TagBinding>,
}

// === Organization-level tags ===

/// Organization tag data from TFE API
#[derive(Deserialize, Debug, Clone)]
pub struct OrgTag {
    pub id: String,
    #[serde(rename = "type", default)]
    pub tag_type: String,
    pub attributes: OrgTagAttributes,
}

impl TfeResource for OrgTag {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.attributes.name
    }
}

/// Organization tag attributes from TFE API
#[derive(Deserialize, Debug, Clone)]
pub struct OrgTagAttributes {
    pub name: String,
    #[serde(rename = "instance-count", default)]
    pub instance_count: u32,
    #[serde(rename = "created-at", default)]
    pub created_at: Option<String>,
}

/// Tag binding data from TFE API
#[derive(Deserialize, Debug, Clone)]
pub struct TagBinding {
    #[serde(default)]
    pub id: String,
    #[serde(rename = "type", default)]
    pub binding_type: String,
    pub attributes: TagBindingAttributes,
}

/// Tag binding attributes from TFE API
#[derive(Deserialize, Debug, Clone)]
pub struct TagBindingAttributes {
    pub key: String,
    pub value: String,
    #[serde(rename = "created-at", default)]
    pub created_at: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tag_binding_deserialization() {
        let json = r#"{
            "id": "tb-abc123",
            "type": "tag-bindings",
            "attributes": {
                "key": "env",
                "value": "prod",
                "created-at": "2024-01-01T00:00:00Z"
            }
        }"#;

        let tag: TagBinding = serde_json::from_str(json).unwrap();
        assert_eq!(tag.id, "tb-abc123");
        assert_eq!(tag.binding_type, "tag-bindings");
        assert_eq!(tag.attributes.key, "env");
        assert_eq!(tag.attributes.value, "prod");
        assert_eq!(
            tag.attributes.created_at,
            Some("2024-01-01T00:00:00Z".to_string())
        );
    }

    #[test]
    fn test_tag_binding_minimal_deserialization() {
        let json = r#"{
            "attributes": {
                "key": "team",
                "value": "backend"
            }
        }"#;

        let tag: TagBinding = serde_json::from_str(json).unwrap();
        assert_eq!(tag.id, "");
        assert_eq!(tag.attributes.key, "team");
        assert_eq!(tag.attributes.value, "backend");
        assert!(tag.attributes.created_at.is_none());
    }

    #[test]
    fn test_tag_bindings_response_deserialization() {
        let json = r#"{
            "data": [
                {
                    "id": "tb-1",
                    "type": "tag-bindings",
                    "attributes": {
                        "key": "env",
                        "value": "prod"
                    }
                },
                {
                    "id": "tb-2",
                    "type": "tag-bindings",
                    "attributes": {
                        "key": "team",
                        "value": "backend"
                    }
                }
            ]
        }"#;

        let response: TagBindingsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.data.len(), 2);
        assert_eq!(response.data[0].attributes.key, "env");
        assert_eq!(response.data[1].attributes.key, "team");
    }

    #[test]
    fn test_tag_bindings_response_empty() {
        let json = r#"{"data": []}"#;
        let response: TagBindingsResponse = serde_json::from_str(json).unwrap();
        assert!(response.data.is_empty());
    }

    // === Organization tag tests ===

    #[test]
    fn test_org_tag_deserialization() {
        let json = r#"{
            "id": "tag-1",
            "type": "tags",
            "attributes": {
                "name": "env",
                "instance-count": 3,
                "created-at": "2024-01-01T00:00:00Z"
            }
        }"#;

        let tag: OrgTag = serde_json::from_str(json).unwrap();
        assert_eq!(tag.id, "tag-1");
        assert_eq!(tag.tag_type, "tags");
        assert_eq!(tag.attributes.name, "env");
        assert_eq!(tag.attributes.instance_count, 3);
        assert_eq!(
            tag.attributes.created_at,
            Some("2024-01-01T00:00:00Z".to_string())
        );
    }

    #[test]
    fn test_org_tag_tfe_resource_trait() {
        let tag = OrgTag {
            id: "tag-1".to_string(),
            tag_type: "tags".to_string(),
            attributes: OrgTagAttributes {
                name: "env".to_string(),
                instance_count: 2,
                created_at: None,
            },
        };
        assert_eq!(tag.id(), "tag-1");
        assert_eq!(tag.name(), "env");
        assert!(tag.matches("tag-1"));
        assert!(tag.matches("env"));
        assert!(!tag.matches("other"));
    }

    #[test]
    fn test_org_tags_response_deserialization() {
        let json = r#"{
            "data": [
                {
                    "id": "tag-1",
                    "type": "tags",
                    "attributes": {
                        "name": "env",
                        "instance-count": 3
                    }
                },
                {
                    "id": "tag-2",
                    "type": "tags",
                    "attributes": {
                        "name": "team",
                        "instance-count": 1
                    }
                }
            ]
        }"#;

        let response: crate::hcp::traits::ApiListResponse<OrgTag> =
            serde_json::from_str(json).unwrap();
        assert_eq!(response.data.len(), 2);
        assert_eq!(response.data[0].attributes.name, "env");
        assert_eq!(response.data[0].attributes.instance_count, 3);
        assert_eq!(response.data[1].attributes.name, "team");
    }

    #[test]
    fn test_org_tags_response_empty() {
        let json = r#"{"data": []}"#;
        let response: crate::hcp::traits::ApiListResponse<OrgTag> =
            serde_json::from_str(json).unwrap();
        assert!(response.data.is_empty());
    }
}
