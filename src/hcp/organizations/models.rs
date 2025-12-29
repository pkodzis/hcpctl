//! Organization data models

use serde::Deserialize;

use crate::hcp::traits::TfeResource;

/// Response wrapper for organizations list
#[derive(Deserialize, Debug)]
pub struct OrganizationsResponse {
    pub data: Vec<Organization>,
}

/// Response wrapper for single organization
#[derive(Deserialize, Debug)]
pub struct OrganizationResponse {
    pub data: Organization,
}

/// Organization data from TFE API
#[derive(Deserialize, Debug, Clone)]
pub struct Organization {
    pub id: String,
    #[serde(rename = "type")]
    pub org_type: Option<String>,
    pub attributes: Option<OrganizationAttributes>,
}

/// Organization attributes from TFE API
#[derive(Deserialize, Debug, Clone)]
pub struct OrganizationAttributes {
    pub name: Option<String>,
    pub email: Option<String>,
    #[serde(rename = "external-id")]
    pub external_id: Option<String>,
}

impl TfeResource for Organization {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        // For orgs, id and name are the same
        &self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_organization_name() {
        let org = Organization {
            id: "my-org".to_string(),
            org_type: Some("organizations".to_string()),
            attributes: Some(OrganizationAttributes {
                name: Some("my-org".to_string()),
                email: None,
                external_id: None,
            }),
        };
        assert_eq!(org.name(), "my-org");
    }

    #[test]
    fn test_organization_matches() {
        let org = Organization {
            id: "my-org".to_string(),
            org_type: None,
            attributes: None,
        };
        assert!(org.matches("my-org"));
        assert!(!org.matches("other"));
    }
}
