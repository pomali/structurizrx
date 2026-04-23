use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{CustomElement, DeploymentNode, Person, SoftwareSystem};

/// A relationship between two model elements.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Relationship {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, String>>,
    pub source_id: String,
    pub destination_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub technology: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interaction_style: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linked_relationship_id: Option<String>,
}

impl Relationship {
    pub fn tags_as_vec(&self) -> Vec<String> {
        match &self.tags {
            Some(t) => t.split(',').map(|s| s.trim().to_string()).collect(),
            None => vec![],
        }
    }
}

/// The model containing all elements.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enterprise: Option<Enterprise>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub people: Option<Vec<Person>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub software_systems: Option<Vec<SoftwareSystem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deployment_nodes: Option<Vec<DeploymentNode>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_elements: Option<Vec<CustomElement>>,
}

/// Enterprise boundary.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Enterprise {
    pub name: String,
}

/// A group of elements.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub name: String,
}

/// Perspective on a model item.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Perspective {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// Common model item fields.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModelItem {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub perspectives: Option<Vec<Perspective>>,
}
