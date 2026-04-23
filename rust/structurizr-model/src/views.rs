use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// All views in a workspace.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ViewSet {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_landscape_views: Option<Vec<SystemLandscapeView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_context_views: Option<Vec<SystemContextView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_views: Option<Vec<ContainerView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub component_views: Option<Vec<ComponentView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_views: Option<Vec<DynamicView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deployment_views: Option<Vec<DeploymentView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filtered_views: Option<Vec<FilteredView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_views: Option<Vec<ImageView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_views: Option<Vec<CustomView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configuration: Option<ViewConfiguration>,
}

/// Configuration for the view set.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ViewConfiguration {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub styles: Option<crate::Styles>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub themes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branding: Option<crate::Branding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminology: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata_symbols: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_view: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_saved_view: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view_sort_order: Option<String>,
}

/// An element view reference in a diagram.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ElementView {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<i32>,
}

/// A relationship view reference in a diagram.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipView {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<bool>,
}

/// Automatic layout settings.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AutomaticLayout {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implementation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rank_direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rank_separation: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_separation: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge_separation: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vertices: Option<bool>,
}

/// Animation step.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Animation {
    pub order: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationship_ids: Option<Vec<String>>,
}

/// Common view fields.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ViewBase {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_views: Option<Vec<ElementView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationship_views: Option<Vec<RelationshipView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automatic_layout: Option<AutomaticLayout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animations: Option<Vec<Animation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paper_size: Option<String>,
}

/// System landscape view.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct SystemLandscapeView {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_views: Option<Vec<ElementView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationship_views: Option<Vec<RelationshipView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automatic_layout: Option<AutomaticLayout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animations: Option<Vec<Animation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paper_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enterprise_boundary_visible: Option<bool>,
}

/// System context view.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct SystemContextView {
    pub software_system_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_views: Option<Vec<ElementView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationship_views: Option<Vec<RelationshipView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automatic_layout: Option<AutomaticLayout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animations: Option<Vec<Animation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paper_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enterprise_boundary_visible: Option<bool>,
}

/// Container view.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ContainerView {
    pub software_system_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_views: Option<Vec<ElementView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationship_views: Option<Vec<RelationshipView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automatic_layout: Option<AutomaticLayout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animations: Option<Vec<Animation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paper_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_software_system_boundary_visible: Option<bool>,
}

/// Component view.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ComponentView {
    pub container_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_views: Option<Vec<ElementView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationship_views: Option<Vec<RelationshipView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automatic_layout: Option<AutomaticLayout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animations: Option<Vec<Animation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paper_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_container_boundary_visible: Option<bool>,
}

/// Dynamic view.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct DynamicView {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_views: Option<Vec<ElementView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationship_views: Option<Vec<RelationshipView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automatic_layout: Option<AutomaticLayout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paper_size: Option<String>,
}

/// Deployment view.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentView {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub software_system_id: Option<String>,
    pub environment: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_views: Option<Vec<ElementView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationship_views: Option<Vec<RelationshipView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automatic_layout: Option<AutomaticLayout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animations: Option<Vec<Animation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paper_size: Option<String>,
}

/// Filtered view.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct FilteredView {
    pub base_view_key: String,
    pub mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

/// Image view.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImageView {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(rename = "contentType")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
}

/// Custom view.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct CustomView {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_views: Option<Vec<ElementView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationship_views: Option<Vec<RelationshipView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automatic_layout: Option<AutomaticLayout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animations: Option<Vec<Animation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paper_size: Option<String>,
}
