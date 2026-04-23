use serde::{Deserialize, Serialize};

/// Documentation attached to a workspace.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Documentation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sections: Option<Vec<Section>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decisions: Option<Vec<Decision>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<DocumentationImage>>,
}

/// A documentation section.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_id: Option<String>,
    pub title: String,
    pub order: i32,
    pub format: String,
    pub content: String,
}

/// An architecture decision record.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Decision {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_id: Option<String>,
    pub id: String,
    pub date: String,
    pub title: String,
    pub status: String,
    pub format: String,
    pub content: String,
}

/// An image attached to documentation.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct DocumentationImage {
    pub name: String,
    pub content: String,
    #[serde(rename = "type")]
    pub image_type: String,
}

/// Documentation format.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub enum Format {
    #[default]
    Markdown,
    AsciiDoc,
}
