//! Mermaid (mermaid.js) flowchart exporter.
//!
//! One `graph TD` diagram per view. Views are scoped the same way the SVG
//! renderer scopes them (see [`crate::scope`]): an explicit `elements` list
//! wins, otherwise the focal element's neighbourhood is derived from the model.
//! Relationships between elements that are not themselves in the view are
//! lifted onto their nearest visible ancestor, matching upstream Structurizr's
//! implied relationships.

use std::collections::{HashMap, HashSet};

use structurizr_model::*;

use crate::diagram::{Diagram, DiagramFormat};
use crate::exporter::DiagramExporter;
use crate::scope::*;

/// Mermaid diagram exporter.
pub struct MermaidExporter;

impl DiagramExporter for MermaidExporter {
    fn export_workspace(&self, workspace: &Workspace) -> Vec<Diagram> {
        let mut diagrams = Vec::new();
        let views = &workspace.views;

        for v in views.system_landscape_views.iter().flatten() {
            let key = v.key.clone().unwrap_or_else(|| "SystemLandscape".to_string());
            let (filter, _) = build_element_filter(v.element_views.as_deref());
            // A landscape without an explicit element list keeps its classic
            // people + software systems scope.
            let scope = filter.unwrap_or_else(|| top_level_ids(&workspace.model));
            let content = render_view(
                workspace,
                v.title.as_deref(),
                &scope,
                build_rel_filter(v.relationship_views.as_deref()).as_ref(),
                None,
            );
            diagrams.push(Diagram::new(key, content, DiagramFormat::Mermaid));
        }

        for v in views.system_context_views.iter().flatten() {
            let key = v.key.clone().unwrap_or_else(|| "SystemContext".to_string());
            let (filter, _) = build_element_filter(v.element_views.as_deref());
            let scope =
                filter.unwrap_or_else(|| context_scope(&workspace.model, &v.software_system_id));
            let content = render_view(
                workspace,
                v.title.as_deref(),
                &scope,
                build_rel_filter(v.relationship_views.as_deref()).as_ref(),
                None,
            );
            diagrams.push(Diagram::new(key, content, DiagramFormat::Mermaid));
        }

        for v in views.container_views.iter().flatten() {
            let key = v.key.clone().unwrap_or_else(|| "Container".to_string());
            let (filter, _) = build_element_filter(v.element_views.as_deref());
            let scope =
                filter.unwrap_or_else(|| container_scope(&workspace.model, &v.software_system_id));
            let content = render_view(
                workspace,
                v.title.as_deref(),
                &scope,
                build_rel_filter(v.relationship_views.as_deref()).as_ref(),
                Some(&v.software_system_id),
            );
            diagrams.push(Diagram::new(key, content, DiagramFormat::Mermaid));
        }

        for v in views.component_views.iter().flatten() {
            let key = v.key.clone().unwrap_or_else(|| "Component".to_string());
            let (filter, _) = build_element_filter(v.element_views.as_deref());
            let scope = filter.unwrap_or_else(|| component_scope(&workspace.model, &v.container_id));
            let content = render_view(
                workspace,
                v.title.as_deref(),
                &scope,
                build_rel_filter(v.relationship_views.as_deref()).as_ref(),
                Some(&v.container_id),
            );
            diagrams.push(Diagram::new(key, content, DiagramFormat::Mermaid));
        }

        diagrams
    }
}

// ── Node collection ──────────────────────────────────────────────────────────

/// An element as it appears in a Mermaid diagram.
struct Node {
    id: String,
    /// The element this one is nested inside (software system for a container,
    /// container for a component), if any.
    parent: Option<String>,
    name: String,
    /// The `Software System` / `Container: Rust` metadata line.
    meta: String,
    tags: Option<String>,
    /// Tag used when the element carries none of its own.
    default_tag: &'static str,
    shape: Shape,
}

/// Mermaid node shape. People get a stadium so they stand out from boxes.
#[derive(Clone, Copy)]
enum Shape {
    Box,
    Rounded,
    Stadium,
}

impl Shape {
    fn wrap(self, label: &str) -> String {
        match self {
            Shape::Box => format!("[\"{}\"]", label),
            Shape::Rounded => format!("(\"{}\")", label),
            Shape::Stadium => format!("([\"{}\"])", label),
        }
    }
}

/// Every top-level element id (people, software systems, custom elements).
fn top_level_ids(model: &Model) -> HashSet<String> {
    let mut ids = HashSet::new();
    for p in model.people.iter().flatten() {
        ids.insert(p.id.clone());
    }
    for ss in model.software_systems.iter().flatten() {
        ids.insert(ss.id.clone());
    }
    for ce in model.custom_elements.iter().flatten() {
        ids.insert(ce.id.clone());
    }
    ids
}

/// Collect the in-scope elements, in a stable order (people, systems and their
/// containers/components, custom elements).
fn collect_nodes(model: &Model, scope: &HashSet<String>) -> Vec<Node> {
    let mut nodes = Vec::new();

    for p in model.people.iter().flatten() {
        if scope.contains(&p.id) {
            nodes.push(Node {
                id: p.id.clone(),
                parent: None,
                name: p.name.clone(),
                meta: "Person".to_string(),
                tags: p.tags.clone(),
                default_tag: "Person",
                shape: Shape::Stadium,
            });
        }
    }

    for ss in model.software_systems.iter().flatten() {
        if scope.contains(&ss.id) {
            nodes.push(Node {
                id: ss.id.clone(),
                parent: None,
                name: ss.name.clone(),
                meta: "Software System".to_string(),
                tags: ss.tags.clone(),
                default_tag: "Software System",
                shape: Shape::Box,
            });
        }
        for c in ss.containers.iter().flatten() {
            if scope.contains(&c.id) {
                nodes.push(Node {
                    id: c.id.clone(),
                    parent: Some(ss.id.clone()),
                    name: c.name.clone(),
                    meta: meta_line("Container", c.technology.as_deref()),
                    tags: c.tags.clone(),
                    default_tag: "Container",
                    shape: Shape::Box,
                });
            }
            for comp in c.components.iter().flatten() {
                if scope.contains(&comp.id) {
                    nodes.push(Node {
                        id: comp.id.clone(),
                        parent: Some(c.id.clone()),
                        name: comp.name.clone(),
                        meta: meta_line("Component", comp.technology.as_deref()),
                        tags: comp.tags.clone(),
                        default_tag: "Component",
                        shape: Shape::Rounded,
                    });
                }
            }
        }
    }

    for ce in model.custom_elements.iter().flatten() {
        if scope.contains(&ce.id) {
            nodes.push(Node {
                id: ce.id.clone(),
                parent: None,
                name: ce.name.clone(),
                meta: ce.metadata.clone().unwrap_or_else(|| "Element".to_string()),
                tags: ce.tags.clone(),
                default_tag: "Element",
                shape: Shape::Box,
            });
        }
    }

    nodes
}

fn meta_line(kind: &str, technology: Option<&str>) -> String {
    match technology.filter(|t| !t.is_empty()) {
        Some(t) => format!("{}: {}", kind, t),
        None => kind.to_string(),
    }
}

// ── Edge collection ──────────────────────────────────────────────────────────

struct MEdge {
    src: String,
    dst: String,
    label: String,
    dotted: bool,
}

/// Relationships visible in this view, with endpoints lifted onto their nearest
/// visible ancestor and lifted duplicates removed.
fn collect_edges(
    model: &Model,
    visible: &HashSet<String>,
    rel_filter: Option<&HashSet<String>>,
) -> Vec<MEdge> {
    let parents = child_parent_map(model);
    let mut out = Vec::new();
    let mut seen: HashSet<(String, String, String)> = HashSet::new();

    for r in all_relationships(model) {
        if let Some(allowed) = rel_filter {
            if !allowed.contains(&r.id) {
                continue;
            }
        }
        let (Some(src), Some(dst)) = (
            lift_to_visible(&r.source_id, &parents, visible),
            lift_to_visible(&r.destination_id, &parents, visible),
        ) else {
            continue;
        };
        if src == dst {
            continue;
        }
        let label = rel_label(r);
        if !seen.insert((src.clone(), dst.clone(), label.clone())) {
            continue;
        }
        // Async-family kinds render as dotted arrows (spec §5.2 → mermaid mapping).
        use RelationshipKind::*;
        let dotted = matches!(
            r.kind,
            Some(Async) | Some(Publish) | Some(Subscribe) | Some(Dataflow)
        );
        out.push(MEdge { src, dst, label, dotted });
    }
    out
}

fn rel_label(rel: &Relationship) -> String {
    let desc = rel.description.as_deref().unwrap_or("");
    match rel.technology.as_deref().filter(|t| !t.is_empty()) {
        Some(tech) if desc.is_empty() => format!("[{}]", tech),
        Some(tech) => format!("{}<br/>[{}]", desc, tech),
        None => desc.to_string(),
    }
}

// ── Rendering ────────────────────────────────────────────────────────────────

/// Render one view. `boundary` is the element whose children get wrapped in a
/// Mermaid `subgraph` (the focal system of a container view, or the focal
/// container of a component view).
fn render_view(
    workspace: &Workspace,
    title: Option<&str>,
    scope: &HashSet<String>,
    rel_filter: Option<&HashSet<String>>,
    boundary: Option<&str>,
) -> String {
    let model = &workspace.model;
    let styles = get_styles(workspace);
    let nodes = collect_nodes(model, scope);
    let visible: HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();
    let edges = collect_edges(model, &visible, rel_filter);

    let mut out = String::new();
    if let Some(t) = title {
        // Mermaid front matter — renderers show this as the diagram title. It is
        // parsed as YAML, so the title is always double-quoted: an unquoted
        // `title: Shop: Containers` is a YAML syntax error.
        out.push_str(&format!("---\ntitle: \"{}\"\n---\n", yaml_escape(t)));
    }
    out.push_str("graph TD\n");

    // Track (alias → css_class) mappings to emit at the end.
    let mut class_assignments: Vec<(String, String)> = Vec::new();
    // Collect unique class definitions: class_name → "fill:…,color:…"
    let mut class_defs: HashMap<String, String> = HashMap::new();

    let mut emit_node = |n: &Node, indent: &str, out: &mut String| {
        let alias = safe_alias(&n.id);
        let label = format!("{}<br/>[{}]", escape(&n.name), escape(&n.meta));
        out.push_str(&format!("{}{}{}\n", indent, alias, n.shape.wrap(&label)));
        if let Some((cls, def)) = build_mermaid_class(n.tags.as_deref(), n.default_tag, styles) {
            class_defs.entry(cls.clone()).or_insert(def);
            class_assignments.push((alias, cls));
        }
    };

    // Nodes inside the boundary go in a subgraph; everything else is top level.
    let inside: Vec<&Node> = match boundary {
        Some(b) => nodes
            .iter()
            .filter(|n| n.parent.as_deref() == Some(b))
            .collect(),
        None => Vec::new(),
    };
    if let (Some(boundary_id), false) = (boundary, inside.is_empty()) {
        let name = element_name(model, boundary_id).unwrap_or_else(|| boundary_id.to_string());
        out.push_str(&format!(
            "    subgraph {}[\"{}\"]\n",
            safe_alias(boundary_id),
            escape(&name)
        ));
        for n in &inside {
            emit_node(n, "        ", &mut out);
        }
        out.push_str("    end\n");
    }
    let inside_ids: HashSet<&str> = inside.iter().map(|n| n.id.as_str()).collect();
    for n in nodes.iter().filter(|n| !inside_ids.contains(n.id.as_str())) {
        emit_node(n, "    ", &mut out);
    }

    for e in &edges {
        let src = safe_alias(&e.src);
        let dst = safe_alias(&e.dst);
        if e.label.is_empty() {
            let arrow = if e.dotted { "-.->" } else { "-->" };
            out.push_str(&format!("    {} {} {}\n", src, arrow, dst));
        } else {
            // Labelled arrows split around the label: `A --"text"--> B`.
            let (head, tail) = if e.dotted { ("-.", ".->") } else { ("--", "-->") };
            out.push_str(&format!(
                "    {} {}\"{}\"{} {}\n",
                src,
                head,
                escape(&e.label),
                tail,
                dst
            ));
        }
    }

    // Emit classDef lines
    let mut defs: Vec<(&String, &String)> = class_defs.iter().collect();
    defs.sort_by_key(|(k, _)| k.as_str());
    for (cls, def) in defs {
        out.push_str(&format!("    classDef {} {}\n", cls, def));
    }

    // Emit class assignment lines
    for (alias, cls) in &class_assignments {
        out.push_str(&format!("    class {} {}\n", alias, cls));
    }

    out
}

/// Display name of any element in the model, by id.
fn element_name(model: &Model, id: &str) -> Option<String> {
    for p in model.people.iter().flatten() {
        if p.id == id {
            return Some(p.name.clone());
        }
    }
    for ss in model.software_systems.iter().flatten() {
        if ss.id == id {
            return Some(ss.name.clone());
        }
        for c in ss.containers.iter().flatten() {
            if c.id == id {
                return Some(c.name.clone());
            }
            for comp in c.components.iter().flatten() {
                if comp.id == id {
                    return Some(comp.name.clone());
                }
            }
        }
    }
    for ce in model.custom_elements.iter().flatten() {
        if ce.id == id {
            return Some(ce.name.clone());
        }
    }
    None
}

/// Escape text for use inside a Mermaid `"…"` label.
///
/// Mermaid has no backslash escapes in quoted strings, so quotes and angle
/// brackets go through HTML entities instead. The `<br/>` separators this
/// module inserts itself are added after escaping, so they survive.
fn escape(s: &str) -> String {
    s.split("<br/>")
        .map(|part| {
            part.replace('&', "&amp;")
                .replace('"', "&quot;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
        })
        .collect::<Vec<_>>()
        .join("<br/>")
}

/// Escape text for a YAML double-quoted scalar (the front-matter title).
fn yaml_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Build a Mermaid class name + definition for a node, if any matching styles exist.
/// Returns `(class_name, "fill:…,color:…,stroke:…")` or `None` if no styles apply.
fn build_mermaid_class(
    tags: Option<&str>,
    default_type_tag: &str,
    styles: Option<&Styles>,
) -> Option<(String, String)> {
    let styles = styles?;
    let element_styles = styles.elements.as_ref()?;

    let owned;
    let tags_str: &str = match tags {
        Some(t) => t,
        None => {
            owned = format!("Element,{}", default_type_tag);
            &owned
        }
    };

    let mut bg: Option<String> = None;
    let mut fg: Option<String> = None;
    let mut stroke: Option<String> = None;
    let mut last_matching_tag = String::new();

    for tag in tags_str.split(',').map(|t| t.trim()) {
        for style in element_styles {
            if style.tag.eq_ignore_ascii_case(tag) {
                if let Some(b) = &style.background { bg = Some(b.clone()); }
                if let Some(c) = &style.color      { fg = Some(c.clone()); }
                if let Some(s) = &style.stroke     { stroke = Some(s.clone()); }
                last_matching_tag = tag.to_string();
            }
        }
    }

    if bg.is_none() && fg.is_none() && stroke.is_none() {
        return None;
    }

    // Use the last matched tag as the class name (sanitized)
    let cls = sanitize_mermaid_class(&last_matching_tag);
    let mut parts: Vec<String> = Vec::new();
    if let Some(b) = bg     { parts.push(format!("fill:{}", b)); }
    if let Some(f) = fg     { parts.push(format!("color:{}", f)); }
    if let Some(s) = stroke { parts.push(format!("stroke:{}", s)); }
    let def = parts.join(",");

    Some((cls, def))
}

fn sanitize_mermaid_class(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect()
}

/// Extract the workspace-level element styles, if any.
fn get_styles(workspace: &Workspace) -> Option<&Styles> {
    workspace.views.configuration.as_ref()?.styles.as_ref()
}

fn safe_alias(id: &str) -> String {
    format!("elem{}", id.replace('-', "_").replace(' ', "_"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use structurizr_model::{
        Component, Container, ContainerView, ElementStyle, Person, Relationship, SoftwareSystem,
        Styles, SystemContextView, SystemLandscapeView, ViewConfiguration, Workspace,
    };

    fn basic_workspace_with_landscape() -> Workspace {
        let mut ws = Workspace::default();
        ws.name = "Test".to_string();
        ws.model.people = Some(vec![Person {
            id: "1".to_string(),
            name: "Alice".to_string(),
            ..Default::default()
        }]);
        ws.model.software_systems = Some(vec![SoftwareSystem {
            id: "2".to_string(),
            name: "MySystem".to_string(),
            ..Default::default()
        }]);
        ws.views.system_landscape_views = Some(vec![SystemLandscapeView {
            key: Some("Landscape".to_string()),
            ..Default::default()
        }]);
        ws
    }

    /// Alice → Shop (Web App + Database), plus an unrelated Other system.
    fn workspace_with_containers() -> Workspace {
        let mut ws = Workspace::default();
        ws.name = "Test".to_string();
        ws.model.people = Some(vec![Person {
            id: "1".to_string(),
            name: "Alice".to_string(),
            relationships: Some(vec![Relationship {
                id: "r1".to_string(),
                source_id: "1".to_string(),
                destination_id: "10".to_string(),
                description: Some("Uses".to_string()),
                ..Default::default()
            }]),
            ..Default::default()
        }]);
        ws.model.software_systems = Some(vec![
            SoftwareSystem {
                id: "2".to_string(),
                name: "Shop".to_string(),
                containers: Some(vec![
                    Container {
                        id: "10".to_string(),
                        name: "Web App".to_string(),
                        technology: Some("Rust".to_string()),
                        relationships: Some(vec![Relationship {
                            id: "r2".to_string(),
                            source_id: "10".to_string(),
                            destination_id: "11".to_string(),
                            description: Some("Reads from".to_string()),
                            technology: Some("SQL".to_string()),
                            ..Default::default()
                        }]),
                        components: Some(vec![Component {
                            id: "100".to_string(),
                            name: "Cart".to_string(),
                            ..Default::default()
                        }]),
                        ..Default::default()
                    },
                    Container {
                        id: "11".to_string(),
                        name: "Database".to_string(),
                        ..Default::default()
                    },
                ]),
                ..Default::default()
            },
            SoftwareSystem {
                id: "3".to_string(),
                name: "Other".to_string(),
                ..Default::default()
            },
        ]);
        ws
    }

    #[test]
    fn mermaid_exporter_basic() {
        let workspace = basic_workspace_with_landscape();
        let exporter = MermaidExporter;
        let diagrams = exporter.export_workspace(&workspace);
        assert_eq!(diagrams.len(), 1);
        assert!(diagrams[0].content.starts_with("graph TD"));
        assert!(diagrams[0].content.contains("Alice"));
        assert!(diagrams[0].content.contains("MySystem"));
    }

    #[test]
    fn mermaid_exporter_respects_element_styles() {
        let mut workspace = basic_workspace_with_landscape();
        workspace.views.configuration = Some(ViewConfiguration {
            styles: Some(Styles {
                elements: Some(vec![ElementStyle {
                    tag: "Person".to_string(),
                    background: Some("#CC0000".to_string()),
                    color: Some("#FFFFFF".to_string()),
                    ..Default::default()
                }]),
                ..Default::default()
            }),
            ..Default::default()
        });

        let exporter = MermaidExporter;
        let diagrams = exporter.export_workspace(&workspace);
        let md = &diagrams[0].content;
        assert!(md.contains("classDef"), "should emit classDef");
        assert!(md.contains("#CC0000"), "fill colour should appear in classDef");
        assert!(md.contains("#FFFFFF"), "text colour should appear in classDef");
        assert!(md.contains("class "), "should assign class to nodes");
    }

    #[test]
    fn context_view_is_scoped_to_the_focal_system() {
        let mut ws = workspace_with_containers();
        ws.views.system_context_views = Some(vec![SystemContextView {
            key: Some("Context".to_string()),
            software_system_id: "2".to_string(),
            ..Default::default()
        }]);

        let diagrams = MermaidExporter.export_workspace(&ws);
        let md = &diagrams[0].content;
        assert!(md.contains("Shop"), "focal system should appear");
        assert!(md.contains("Alice"), "related person should appear");
        assert!(!md.contains("Other"), "unrelated system should be excluded");
        // Alice→Web App lifts to Alice→Shop, since Web App is not in this view.
        assert!(md.contains("elem1 --\"Uses\"--> elem2"), "got:\n{}", md);
    }

    #[test]
    fn container_view_renders_containers_in_a_boundary() {
        let mut ws = workspace_with_containers();
        ws.views.container_views = Some(vec![ContainerView {
            key: Some("Containers".to_string()),
            software_system_id: "2".to_string(),
            ..Default::default()
        }]);

        let diagrams = MermaidExporter.export_workspace(&ws);
        assert_eq!(diagrams.len(), 1);
        let md = &diagrams[0].content;
        assert!(md.contains("subgraph elem2[\"Shop\"]"), "got:\n{}", md);
        assert!(md.contains("Web App<br/>[Container: Rust]"), "got:\n{}", md);
        assert!(md.contains("Database"), "got:\n{}", md);
        assert!(md.contains("Alice"), "related person should appear");
        // The component inside Web App is below this view's level of detail.
        assert!(!md.contains("Cart"), "got:\n{}", md);
        assert!(
            md.contains("elem10 --\"Reads from<br/>[SQL]\"--> elem11"),
            "got:\n{}",
            md
        );
    }

    /// The front matter is YAML, so a title containing `:` must be quoted —
    /// unquoted, mermaid rejects the whole diagram with a YAML error.
    #[test]
    fn view_title_is_quoted_in_front_matter() {
        let mut ws = basic_workspace_with_landscape();
        ws.views.system_landscape_views.as_mut().unwrap()[0].title =
            Some("Shop: The \"Big\" View".to_string());
        let diagrams = MermaidExporter.export_workspace(&ws);
        let md = &diagrams[0].content;
        assert!(
            md.starts_with("---\ntitle: \"Shop: The \\\"Big\\\" View\"\n---\ngraph TD\n"),
            "got:\n{}",
            md
        );
    }

    #[test]
    fn labels_with_quotes_are_escaped() {
        let mut ws = basic_workspace_with_landscape();
        ws.model.software_systems.as_mut().unwrap()[0].name = "The \"Big\" System".to_string();
        let diagrams = MermaidExporter.export_workspace(&ws);
        let md = &diagrams[0].content;
        assert!(md.contains("The &quot;Big&quot; System"), "got:\n{}", md);
    }
}
