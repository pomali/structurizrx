use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{Documentation, Model, Perspective, ViewSet, WorkspaceConfiguration};

/// Named point on the roadmap, ordered by declaration.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Milestone {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A Structurizr workspace.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified_user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified_agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub milestones: Option<Vec<Milestone>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub perspectives: Option<Vec<Perspective>>,
    pub model: Model,
    pub views: ViewSet,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<Documentation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configuration: Option<WorkspaceConfiguration>,
}
