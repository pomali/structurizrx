//! What the viewer's find-anything palette (⌘K, `structurizr-palette.js`)
//! searches: every model element with its path and the views showing it,
//! every view under the title the sidebar shows, every decision.
//!
//! Served as `/api/workspace/{name}/catalog`, and written next to an
//! `export-viewer` bundle as `catalog.json`, so the palette works the same
//! with or without a server.

use serde::Serialize;
use structurizr_model::Workspace;
use structurizr_query::index::{build_index, Index};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub elements: Vec<CatalogElement>,
    pub views: Vec<CatalogView>,
    pub decisions: Vec<CatalogDecision>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogElement {
    pub id: String,
    pub name: String,
    pub kind: &'static str,
    /// Canonical path (`Shop/API`), the reference the viewer selects it by.
    pub path: String,
    pub technology: Option<String>,
    pub description: Option<String>,
    /// Keys of the views showing it, best place to see it first.
    pub views: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogView {
    pub key: String,
    /// The title the workspace page's sidebar shows.
    pub title: String,
    /// `SystemLandscape`, `Container`, … as in the workspace JSON.
    pub kind: &'static str,
    pub description: Option<String>,
    /// A generated (`auto`) view.
    pub generated: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogDecision {
    pub id: String,
    pub title: String,
    pub status: String,
}

/// Build the catalog. Views are in sidebar order: hand-authored views first,
/// in C4 order, then generated ones.
pub fn catalog(workspace: &Workspace) -> Catalog {
    let idx = build_index(workspace);
    let views = views(workspace, &idx);
    let rank: std::collections::HashMap<&str, usize> = views
        .iter()
        .enumerate()
        .map(|(i, v)| (v.key.as_str(), i))
        .collect();
    let generated: std::collections::HashSet<&str> = views
        .iter()
        .filter(|v| v.generated)
        .map(|v| v.key.as_str())
        .collect();

    let elements = idx
        .elements
        .iter()
        .map(|e| {
            let mut path: Vec<&str> = e.ancestor_names.iter().rev().map(String::as_str).collect();
            path.push(&e.name);

            // Where to see an element: a hand-authored view before a generated
            // one, and the most detailed level first — a container is best
            // seen among its siblings in a container view, not as a dot in
            // the landscape.
            let mut keys: Vec<(&str, &'static str)> = idx
                .views_for(&e.id)
                .into_iter()
                .map(|v| (v.key.as_str(), v.kind))
                .collect();
            keys.sort_by_key(|(key, kind)| {
                (
                    generated.contains(key),
                    detail_order(kind),
                    rank.get(key).copied().unwrap_or(usize::MAX),
                )
            });

            CatalogElement {
                id: e.id.clone(),
                name: e.name.clone(),
                kind: e.kind,
                path: path.join("/"),
                technology: e.technology.clone(),
                description: e.description.clone(),
                views: keys.into_iter().map(|(k, _)| k.to_string()).collect(),
            }
        })
        .collect();

    let decisions = workspace
        .documentation
        .as_ref()
        .and_then(|d| d.decisions.as_ref())
        .map(|ds| {
            ds.iter()
                .map(|d| CatalogDecision {
                    id: d.id.clone(),
                    title: d.title.clone(),
                    status: d.status.clone(),
                })
                .collect()
        })
        .unwrap_or_default();

    Catalog {
        elements,
        views,
        decisions,
    }
}

/// Preference among the index's view kinds when choosing where to show an
/// element: the level that shows it with its peers first.
fn detail_order(kind: &str) -> u8 {
    match kind {
        "system context" => 0,
        "container" => 1,
        "component" => 2,
        "system landscape" => 3,
        "dynamic" => 4,
        "deployment" => 5,
        _ => 6,
    }
}

fn views(ws: &Workspace, idx: &Index) -> Vec<CatalogView> {
    let name_of = |id: &str| {
        idx.element(id)
            .map_or_else(|| id.to_string(), |e| e.name.clone())
    };
    let parent_id = |id: &str| idx.element(id).and_then(|e| e.parent_id.clone());
    let mut out: Vec<CatalogView> = Vec::new();
    let v = &ws.views;

    // The fallback titles mirror structurizr.ui.getDefaultViewName
    // (assets/js/structurizr-ui.js), so the palette and the sidebar agree.
    for view in v.system_landscape_views.iter().flatten() {
        let enterprise = ws
            .model
            .enterprise
            .as_ref()
            .map(|e| format!(": {}", e.name));
        push(
            &mut out,
            &view.key,
            &view.title,
            &view.description,
            "SystemLandscape",
            format!("System Landscape View{}", enterprise.unwrap_or_default()),
        );
    }
    for view in v.system_context_views.iter().flatten() {
        push(
            &mut out,
            &view.key,
            &view.title,
            &view.description,
            "SystemContext",
            format!("System Context View: {}", name_of(&view.software_system_id)),
        );
    }
    for view in v.container_views.iter().flatten() {
        push(
            &mut out,
            &view.key,
            &view.title,
            &view.description,
            "Container",
            format!("Container View: {}", name_of(&view.software_system_id)),
        );
    }
    for view in v.component_views.iter().flatten() {
        let container = name_of(&view.container_id);
        let system = parent_id(&view.container_id).map(|p| name_of(&p));
        push(
            &mut out,
            &view.key,
            &view.title,
            &view.description,
            "Component",
            match system {
                Some(system) => format!("Component View: {system} - {container}"),
                None => format!("Component View: {container}"),
            },
        );
    }
    for view in v.dynamic_views.iter().flatten() {
        let scope = view.element_id.as_deref().map(|id| match parent_id(id) {
            Some(parent) => format!("{} - {}", name_of(&parent), name_of(id)),
            None => name_of(id),
        });
        push(
            &mut out,
            &view.key,
            &view.title,
            &view.description,
            "Dynamic",
            scope.map_or_else(
                || "Dynamic View".to_string(),
                |s| format!("Dynamic View: {s}"),
            ),
        );
    }
    for view in v.deployment_views.iter().flatten() {
        push(
            &mut out,
            &view.key,
            &view.title,
            &view.description,
            "Deployment",
            match &view.software_system_id {
                Some(id) => format!("Deployment View: {} - {}", name_of(id), view.environment),
                None => format!("Deployment View: {}", view.environment),
            },
        );
    }
    for view in v.image_views.iter().flatten() {
        let key = view.key.clone().unwrap_or_default();
        push(
            &mut out,
            &view.key,
            &view.title,
            &None,
            "Image",
            format!("Image View: {key}"),
        );
    }
    for view in v.custom_views.iter().flatten() {
        push(
            &mut out,
            &view.key,
            &view.title,
            &view.description,
            "Custom",
            "Custom View: Untitled".to_string(),
        );
    }
    let resolved: Vec<(String, String)> = out
        .iter()
        .map(|c| (c.key.clone(), c.title.clone()))
        .collect();
    for view in v.filtered_views.iter().flatten() {
        let base = resolved
            .iter()
            .find(|(k, _)| *k == view.base_view_key)
            .map(|(_, t)| t.clone())
            .unwrap_or_else(|| view.base_view_key.clone());
        push(
            &mut out,
            &view.key,
            &view.title,
            &view.description,
            "Filtered",
            base,
        );
    }

    // Sidebar order: own views first, by C4 level; generated views after.
    // `sort_by_key` is stable, so model order holds within each level.
    out.sort_by_key(|view| (view.generated, kind_order(view.kind)));
    out
}

fn push(
    out: &mut Vec<CatalogView>,
    key: &Option<String>,
    title: &Option<String>,
    description: &Option<String>,
    kind: &'static str,
    fallback: String,
) {
    let Some(key) = key.clone() else { return };
    let title = title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .unwrap_or(fallback);
    out.push(CatalogView {
        generated: key.starts_with("auto-"),
        key,
        title,
        kind,
        description: description.clone(),
    });
}

fn kind_order(kind: &str) -> u8 {
    match kind {
        "SystemLandscape" => 0,
        "SystemContext" => 1,
        "Container" => 2,
        "Component" => 3,
        "Dynamic" => 4,
        "Deployment" => 5,
        "Filtered" => 6,
        "Image" => 7,
        _ => 8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shop() -> Workspace {
        structurizr_dsl::parse_str(
            r#"workspace "Shop" {
                model {
                    customer = person "Customer"
                    shop = softwareSystem "Shop" {
                        api = container "API" "Handles requests" "Rust"
                        db = container "Database"
                    }
                    customer -> api "uses"
                    customer -> shop "browses"
                    api -> db "reads"
                }
                views {
                    systemLandscape "landscape" { include * }
                    container shop "containers" "Containers" { include * }
                    systemContext shop "context" { include * }
                    deployment shop "Live" "live" { include * }
                }
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn element_paths_and_best_view_first() {
        let c = catalog(&shop());
        let api = c.elements.iter().find(|e| e.name == "API").unwrap();
        assert_eq!(api.path, "Shop/API");
        assert_eq!(api.technology.as_deref(), Some("Rust"));
        assert_eq!(api.views, vec!["containers"]);

        // A person in landscape, context and container views is best seen in
        // the context view.
        let customer = c.elements.iter().find(|e| e.name == "Customer").unwrap();
        assert_eq!(customer.views.first().map(String::as_str), Some("context"));
    }

    #[test]
    fn views_in_sidebar_order_with_sidebar_titles() {
        let c = catalog(&shop());
        let keys: Vec<&str> = c.views.iter().map(|v| v.key.as_str()).collect();
        assert_eq!(keys, vec!["landscape", "context", "containers", "live"]);
        let title = |key: &str| c.views.iter().find(|v| v.key == key).unwrap().title.clone();
        assert_eq!(title("containers"), "Containers");
        assert_eq!(title("context"), "System Context View: Shop");
        assert_eq!(title("live"), "Deployment View: Shop - Live");
    }
}
