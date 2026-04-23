use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Workspace configuration.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceConfiguration {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub users: Option<Vec<User>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, String>>,
}

/// A user with workspace access.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub username: String,
    pub role: String,
}

/// Visibility scope.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub enum Visibility {
    #[default]
    Private,
    Public,
}

/// Workspace scope.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub enum WorkspaceScope {
    #[default]
    Landscape,
    SoftwareSystem,
}

/// Role for users.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub enum Role {
    #[default]
    ReadOnly,
    ReadWrite,
}
