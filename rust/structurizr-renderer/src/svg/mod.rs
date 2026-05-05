use std::collections::{HashMap, HashSet};

use structurizr_model::*;

use crate::diagram::{Diagram, DiagramFormat};
use crate::exporter::DiagramExporter;

// ── Layout constants ─────────────────────────────────────────────────────────

const BOX_W: i32 = 160;
const BOX_H: i32 = 80;
const H_GAP: i32 = 60;
const V_GAP: i32 = 100;
const MARGIN: i32 = 60;
const COLS: usize = 4;
const BOUNDARY_LABEL_HEIGHT: i32 = 16;
const PERSON_HEAD_RADIUS: i32 = 12;

// ── C4 colour palette ────────────────────────────────────────────────────────

const COLOR_PERSON: &str = "#08427B";
const COLOR_SYSTEM: &str = "#1168BD";
const COLOR_SYSTEM_EXT: &str = "#666666";
const COLOR_CONTAINER: &str = "#438DD5";
const COLOR_TEXT_LIGHT: &str = "#ffffff";
const COLOR_BOUNDARY_FILL: &str = "#ffffff";
const COLOR_BOUNDARY_STROKE: &str = "#cccccc";
const COLOR_ARROW: &str = "#555555";
const COLOR_TITLE: &str = "#333333";

// ── Internal node/edge types ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Node {
    id: String,
    name: String,
    type_label: String,
    fill: String,
    stroke: String,
    text_color: String,
    is_person: bool,
    x: i32,
    y: i32,
}

impl Node {
    fn cx(&self) -> i32 {
        self.x + BOX_W / 2
    }
    fn cy(&self) -> i32 {
        self.y + BOX_H / 2
    }
}

#[derive(Debug, Clone)]
struct Edge {
    src_id: String,
    dst_id: String,
    label: String,
    technology: String,
}

// ── Public exporter ──────────────────────────────────────────────────────────

/// SVG diagram exporter — produces standalone SVG files with no external tooling required.
pub struct SvgExporter;

impl DiagramExporter for SvgExporter {
    fn export_workspace(&self, workspace: &Workspace) -> Vec<Diagram> {
        let mut diagrams = Vec::new();
        let views = &workspace.views;

        if let Some(sl_views) = &views.system_landscape_views {
            for v in sl_views {
                let key = v.key.clone().unwrap_or_else(|| "SystemLandscape".to_string());
                let title = v.title.as_deref().unwrap_or(&key);
                let content = render_landscape(title, v, workspace);
                diagrams.push(Diagram::new(key, content, DiagramFormat::Svg));
            }
        }

        if let Some(sc_views) = &views.system_context_views {
            for v in sc_views {
                let key = v.key.clone().unwrap_or_else(|| "SystemContext".to_string());
                let title = v.title.as_deref().unwrap_or(&key);
                let content = render_system_context(title, v, workspace);
                diagrams.push(Diagram::new(key, content, DiagramFormat::Svg));
            }
        }

        if let Some(cv) = &views.container_views {
            for v in cv {
                let key = v.key.clone().unwrap_or_else(|| "Container".to_string());
                let title = v.title.as_deref().unwrap_or(&key);
                let content = render_container_view(title, v, workspace);
                diagrams.push(Diagram::new(key, content, DiagramFormat::Svg));
            }
        }

        diagrams
    }
}

// ── Per-view renderers ────────────────────────────────────────────────────────

fn render_landscape(title: &str, view: &SystemLandscapeView, workspace: &Workspace) -> String {
    let model = &workspace.model;
    let styles = get_styles(workspace);
    let (elem_filter, elem_pos) = build_element_filter(view.element_views.as_deref());
    let rel_filter = build_rel_filter(view.relationship_views.as_deref());
    let mut nodes: Vec<Node> = Vec::new();

    if let Some(people) = &model.people {
        for p in people {
            if !elem_allowed(&elem_filter, &p.id) {
                continue;
            }
            let s = resolve_node_style(p.tags.as_deref(), "Person", styles, COLOR_PERSON, COLOR_TEXT_LIGHT);
            nodes.push(Node {
                id: p.id.clone(),
                name: p.name.clone(),
                type_label: "Person".to_string(),
                fill: s.fill,
                stroke: s.stroke,
                text_color: s.text_color,
                is_person: true,
                x: 0,
                y: 0,
            });
        }
    }

    if let Some(systems) = &model.software_systems {
        for ss in systems {
            if !elem_allowed(&elem_filter, &ss.id) {
                continue;
            }
            let s = resolve_node_style(ss.tags.as_deref(), "Software System", styles, COLOR_SYSTEM, COLOR_TEXT_LIGHT);
            nodes.push(Node {
                id: ss.id.clone(),
                name: ss.name.clone(),
                type_label: "Software System".to_string(),
                fill: s.fill,
                stroke: s.stroke,
                text_color: s.text_color,
                is_person: false,
                x: 0,
                y: 0,
            });
        }
    }

    // Only use auto-layout when no stored positions are available.  Running layout
    // when some nodes already have explicit coordinates would overwrite those values.
    let edges = collect_all_edges(model, rel_filter.as_ref());
    let positioned = apply_stored_positions(&mut nodes, &elem_pos);
    if positioned == 0 {
        layout_hierarchical(&mut nodes, &edges);
    }
    render_svg(title, &nodes, &edges, None)
}

fn render_system_context(title: &str, view: &SystemContextView, workspace: &Workspace) -> String {
    let model = &workspace.model;
    let styles = get_styles(workspace);
    let focal_id = &view.software_system_id;
    let (elem_filter, elem_pos) = build_element_filter(view.element_views.as_deref());
    let rel_filter = build_rel_filter(view.relationship_views.as_deref());

    let mut people_nodes: Vec<Node> = Vec::new();
    let mut focal_node: Option<Node> = None;
    let mut ext_nodes: Vec<Node> = Vec::new();

    if let Some(people) = &model.people {
        for p in people {
            if !elem_allowed(&elem_filter, &p.id) {
                continue;
            }
            let s = resolve_node_style(p.tags.as_deref(), "Person", styles, COLOR_PERSON, COLOR_TEXT_LIGHT);
            people_nodes.push(Node {
                id: p.id.clone(),
                name: p.name.clone(),
                type_label: "Person".to_string(),
                fill: s.fill,
                stroke: s.stroke,
                text_color: s.text_color,
                is_person: true,
                x: 0,
                y: 0,
            });
        }
    }

    if let Some(systems) = &model.software_systems {
        for ss in systems {
            if !elem_allowed(&elem_filter, &ss.id) {
                continue;
            }
            if &ss.id == focal_id {
                let s = resolve_node_style(ss.tags.as_deref(), "Software System", styles, COLOR_SYSTEM, COLOR_TEXT_LIGHT);
                focal_node = Some(Node {
                    id: ss.id.clone(),
                    name: ss.name.clone(),
                    type_label: "Software System".to_string(),
                    fill: s.fill,
                    stroke: s.stroke,
                    text_color: s.text_color,
                    is_person: false,
                    x: 0,
                    y: 0,
                });
            } else {
                let s = resolve_node_style(ss.tags.as_deref(), "Software System", styles, COLOR_SYSTEM_EXT, COLOR_TEXT_LIGHT);
                ext_nodes.push(Node {
                    id: ss.id.clone(),
                    name: ss.name.clone(),
                    type_label: "Software System".to_string(),
                    fill: s.fill,
                    stroke: s.stroke,
                    text_color: s.text_color,
                    is_person: false,
                    x: 0,
                    y: 0,
                });
            }
        }
    }

    let mut nodes: Vec<Node> = people_nodes;
    if let Some(fn_) = focal_node {
        nodes.push(fn_);
    }
    nodes.extend(ext_nodes);

    let edges = collect_all_edges(model, rel_filter.as_ref());
    let positioned = apply_stored_positions(&mut nodes, &elem_pos);
    if positioned == 0 {
        layout_hierarchical(&mut nodes, &edges);
    }
    render_svg(title, &nodes, &edges, None)
}

fn render_container_view(title: &str, view: &ContainerView, workspace: &Workspace) -> String {
    let model = &workspace.model;
    let styles = get_styles(workspace);
    let focal_id = &view.software_system_id;
    let (elem_filter, elem_pos) = build_element_filter(view.element_views.as_deref());
    let rel_filter = build_rel_filter(view.relationship_views.as_deref());

    let mut people_nodes: Vec<Node> = Vec::new();
    let mut container_nodes: Vec<Node> = Vec::new();
    let mut ext_nodes: Vec<Node> = Vec::new();
    let mut focal_system_name = String::new();

    if let Some(people) = &model.people {
        for p in people {
            if !elem_allowed(&elem_filter, &p.id) {
                continue;
            }
            let s = resolve_node_style(p.tags.as_deref(), "Person", styles, COLOR_PERSON, COLOR_TEXT_LIGHT);
            people_nodes.push(Node {
                id: p.id.clone(),
                name: p.name.clone(),
                type_label: "Person".to_string(),
                fill: s.fill,
                stroke: s.stroke,
                text_color: s.text_color,
                is_person: true,
                x: 0,
                y: 0,
            });
        }
    }

    if let Some(systems) = &model.software_systems {
        for ss in systems {
            if &ss.id == focal_id {
                focal_system_name = ss.name.clone();
                if let Some(containers) = &ss.containers {
                    for c in containers {
                        if !elem_allowed(&elem_filter, &c.id) {
                            continue;
                        }
                        let tech = c.technology.as_deref().unwrap_or("").to_string();
                        let type_label = if tech.is_empty() {
                            "Container".to_string()
                        } else {
                            format!("Container: {}", tech)
                        };
                        let s = resolve_node_style(c.tags.as_deref(), "Container", styles, COLOR_CONTAINER, COLOR_TEXT_LIGHT);
                        container_nodes.push(Node {
                            id: c.id.clone(),
                            name: c.name.clone(),
                            type_label,
                            fill: s.fill,
                            stroke: s.stroke,
                            text_color: s.text_color,
                            is_person: false,
                            x: 0,
                            y: 0,
                        });
                    }
                }
            } else {
                if !elem_allowed(&elem_filter, &ss.id) {
                    continue;
                }
                let s = resolve_node_style(ss.tags.as_deref(), "Software System", styles, COLOR_SYSTEM_EXT, COLOR_TEXT_LIGHT);
                ext_nodes.push(Node {
                    id: ss.id.clone(),
                    name: ss.name.clone(),
                    type_label: "Software System".to_string(),
                    fill: s.fill,
                    stroke: s.stroke,
                    text_color: s.text_color,
                    is_person: false,
                    x: 0,
                    y: 0,
                });
            }
        }
    }

    // Layout: people in top row, then containers, then external systems
    let mut all_nodes: Vec<Node> = people_nodes;
    let people_count = all_nodes.len();
    all_nodes.extend(container_nodes);
    let container_end = all_nodes.len();
    all_nodes.extend(ext_nodes);

    let edges = collect_all_edges_with_containers(model, rel_filter.as_ref());
    let positioned = apply_stored_positions(&mut all_nodes, &elem_pos);
    if positioned == 0 {
        layout_hierarchical(&mut all_nodes, &edges);
    }

    // Compute the bounding box around container nodes for the system boundary
    let boundary = if container_end > people_count {
        let c_nodes = &all_nodes[people_count..container_end];
        Some(boundary_rect(c_nodes, &focal_system_name))
    } else {
        None
    };

    render_svg(title, &all_nodes, &edges, boundary.as_ref())
}

// ── Layout ───────────────────────────────────────────────────────────────────

/// Assign x/y positions using a **hierarchical (layered) layout**.
///
/// The algorithm mirrors the first two phases of Sugiyama's framework:
///
/// 1. **Longest-path layering** — propagate layer numbers through the directed
///    graph so that every edge goes from a lower layer to a higher one.
/// 2. **Barycentric ordering** — reorder nodes within each layer by the
///    average position of their predecessors to reduce edge crossings.
/// 3. **Coordinate assignment** — centre each layer horizontally relative to
///    the widest layer.
///
/// Falls back to a simple COLS-wide grid when no edges connect any of the
/// supplied nodes (isolated-node diagrams look better in a compact grid).
fn layout_hierarchical(nodes: &mut Vec<Node>, edges: &[Edge]) {
    if nodes.is_empty() {
        return;
    }

    // Build node-id → index map (only for nodes that are in this diagram)
    let id_to_idx: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();

    // Check whether any edges connect two visible nodes.
    let has_local_edges = edges.iter().any(|e| {
        id_to_idx.contains_key(e.src_id.as_str())
            && id_to_idx.contains_key(e.dst_id.as_str())
    });

    if !has_local_edges {
        layout_grid(nodes, COLS);
        return;
    }

    let n = nodes.len();

    // ── 1. Longest-path layering ──────────────────────────────────────────────
    let mut layer = vec![0usize; n];
    // Iterate n times so that longest paths of any length are propagated.
    for _ in 0..n {
        for edge in edges {
            if let (Some(&src), Some(&dst)) = (
                id_to_idx.get(edge.src_id.as_str()),
                id_to_idx.get(edge.dst_id.as_str()),
            ) {
                if layer[src] + 1 > layer[dst] {
                    layer[dst] = layer[src] + 1;
                }
            }
        }
    }

    // ── 2. Group nodes by layer ───────────────────────────────────────────────
    let num_layers = layer.iter().max().copied().unwrap_or(0) + 1;
    let mut layers: Vec<Vec<usize>> = vec![Vec::new(); num_layers];
    for (i, &l) in layer.iter().enumerate() {
        layers[l].push(i);
    }

    // ── 3. Barycentric ordering (one downward sweep) ─────────────────────────
    // Track the position-within-layer of each node for barycentre calculation.
    let mut pos_in_layer = vec![0usize; n];
    for layer_nodes in &layers {
        for (pos, &idx) in layer_nodes.iter().enumerate() {
            pos_in_layer[idx] = pos;
        }
    }

    // Build predecessor lists (for nodes in this diagram only).
    let mut predecessors: Vec<Vec<usize>> = vec![Vec::new(); n];
    for edge in edges {
        if let (Some(&src), Some(&dst)) = (
            id_to_idx.get(edge.src_id.as_str()),
            id_to_idx.get(edge.dst_id.as_str()),
        ) {
            predecessors[dst].push(src);
        }
    }

    for row in 1..num_layers {
        if layers[row].len() <= 1 {
            continue;
        }
        let mut bary: Vec<(usize, f64)> = layers[row]
            .iter()
            .map(|&idx| {
                let preds = &predecessors[idx];
                let score = if preds.is_empty() {
                    pos_in_layer[idx] as f64
                } else {
                    preds.iter().map(|&p| pos_in_layer[p] as f64).sum::<f64>()
                        / preds.len() as f64
                };
                (idx, score)
            })
            .collect();
        bary.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        layers[row] = bary.iter().map(|(idx, _)| *idx).collect();
        for (pos, &idx) in layers[row].iter().enumerate() {
            pos_in_layer[idx] = pos;
        }
    }

    // ── 4. Coordinate assignment ─────────────────────────────────────────────
    let max_per_layer = layers.iter().map(|l| l.len()).max().unwrap_or(1).max(1);
    let canvas_w = max_per_layer as i32 * (BOX_W + H_GAP) - H_GAP + 2 * MARGIN;

    for (row, layer_nodes) in layers.iter().enumerate() {
        let count = layer_nodes.len() as i32;
        if count == 0 {
            continue;
        }
        let layer_w = count * BOX_W + (count - 1).max(0) * H_GAP;
        let start_x = (canvas_w - layer_w) / 2;
        for (col, &node_idx) in layer_nodes.iter().enumerate() {
            nodes[node_idx].x = start_x + col as i32 * (BOX_W + H_GAP);
            nodes[node_idx].y = MARGIN + row as i32 * (BOX_H + V_GAP);
        }
    }
}

/// Assign x/y positions in a left-to-right, top-to-bottom grid.
/// Used as the fallback when no edges exist between the diagram's nodes.
fn layout_grid(nodes: &mut Vec<Node>, cols: usize) {
    let cols = cols.max(1);
    for (i, node) in nodes.iter_mut().enumerate() {
        let col = (i % cols) as i32;
        let row = (i / cols) as i32;
        node.x = MARGIN + col * (BOX_W + H_GAP);
        node.y = MARGIN + row * (BOX_H + V_GAP);
    }
}

// ── Edge collection ───────────────────────────────────────────────────────────

fn collect_all_edges(model: &Model, rel_filter: Option<&HashSet<String>>) -> Vec<Edge> {
    let mut edges = Vec::new();
    if let Some(people) = &model.people {
        for p in people {
            collect_rels(&p.relationships, &mut edges, rel_filter);
        }
    }
    if let Some(systems) = &model.software_systems {
        for ss in systems {
            collect_rels(&ss.relationships, &mut edges, rel_filter);
        }
    }
    edges
}

fn collect_all_edges_with_containers(model: &Model, rel_filter: Option<&HashSet<String>>) -> Vec<Edge> {
    let mut edges = collect_all_edges(model, rel_filter);
    if let Some(systems) = &model.software_systems {
        for ss in systems {
            if let Some(containers) = &ss.containers {
                for c in containers {
                    collect_rels(&c.relationships, &mut edges, rel_filter);
                    if let Some(components) = &c.components {
                        for comp in components {
                            collect_rels(&comp.relationships, &mut edges, rel_filter);
                        }
                    }
                }
            }
        }
    }
    edges
}

fn collect_rels(rels: &Option<Vec<Relationship>>, edges: &mut Vec<Edge>, rel_filter: Option<&HashSet<String>>) {
    if let Some(rels) = rels {
        for r in rels {
            if let Some(filter) = rel_filter {
                if !filter.contains(&r.id) {
                    continue;
                }
            }
            edges.push(Edge {
                src_id: r.source_id.clone(),
                dst_id: r.destination_id.clone(),
                label: r.description.clone().unwrap_or_default(),
                technology: r.technology.clone().unwrap_or_default(),
            });
        }
    }
}

// ── View filter helpers ───────────────────────────────────────────────────────

/// Build an element ID allowlist and a stored-position map from a view's `element_views`.
///
/// Returns `(None, empty_map)` when `element_views` is absent or empty, which means
/// "show all elements" (backwards-compatible behaviour for workspaces without explicit views).
fn build_element_filter(
    element_views: Option<&[ElementView]>,
) -> (Option<HashSet<String>>, HashMap<String, (i32, i32)>) {
    let evs = match element_views {
        None => return (None, HashMap::new()),
        Some(evs) if evs.is_empty() => return (None, HashMap::new()),
        Some(evs) => evs,
    };
    let ids: HashSet<String> = evs.iter().map(|ev| ev.id.clone()).collect();
    let pos: HashMap<String, (i32, i32)> = evs
        .iter()
        .filter_map(|ev| ev.x.zip(ev.y).map(|(x, y)| (ev.id.clone(), (x, y))))
        .collect();
    (Some(ids), pos)
}

/// Build a relationship ID allowlist from a view's `relationship_views`.
///
/// Returns `None` when absent or empty (meaning "allow all relationships").
fn build_rel_filter(rel_views: Option<&[RelationshipView]>) -> Option<HashSet<String>> {
    let rvs = rel_views?;
    if rvs.is_empty() {
        return None;
    }
    Some(rvs.iter().map(|rv| rv.id.clone()).collect())
}

/// Returns `true` if `id` is allowed by the element filter.
///
/// When the filter is `None` (no `element_views` present), all elements are allowed.
fn elem_allowed(filter: &Option<HashSet<String>>, id: &str) -> bool {
    match filter {
        None => true,
        Some(set) => set.contains(id),
    }
}

/// Apply stored x/y positions from the element-position map to `nodes`.
///
/// Returns the number of nodes that received a stored position.  Nodes whose IDs
/// are absent from the map are left at their current coordinates.
fn apply_stored_positions(nodes: &mut [Node], pos: &HashMap<String, (i32, i32)>) -> usize {
    let mut count = 0;
    for node in nodes.iter_mut() {
        if let Some(&(x, y)) = pos.get(&node.id) {
            node.x = x;
            node.y = y;
            count += 1;
        }
    }
    count
}

// ── Boundary helper ───────────────────────────────────────────────────────────

struct BoundaryRect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    label: String,
}

fn boundary_rect(nodes: &[Node], label: &str) -> BoundaryRect {
    let padding = 20;
    let min_x = nodes.iter().map(|n| n.x).min().unwrap_or(0) - padding;
    let min_y = nodes.iter().map(|n| n.y).min().unwrap_or(0) - padding - BOUNDARY_LABEL_HEIGHT; // extra for label
    let max_x = nodes.iter().map(|n| n.x + BOX_W).max().unwrap_or(0) + padding;
    let max_y = nodes.iter().map(|n| n.y + BOX_H).max().unwrap_or(0) + padding;
    BoundaryRect {
        x: min_x,
        y: min_y,
        w: max_x - min_x,
        h: max_y - min_y,
        label: label.to_string(),
    }
}

// ── SVG renderer ──────────────────────────────────────────────────────────────

fn render_svg(
    title: &str,
    nodes: &[Node],
    edges: &[Edge],
    boundary: Option<&BoundaryRect>,
) -> String {
    // Build lookup for quick position access
    let pos: HashMap<&str, &Node> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    // Compute canvas size from node positions.
    // Person nodes have a head that protrudes PERSON_HEAD_RADIUS px above their y coordinate.
    // We clamp `top` to at most 0: a positive minimum means everything is already below the
    // origin and no extra padding is needed; a negative value means some element extends
    // above y=0 and we must enlarge the canvas to avoid clipping.
    let top = nodes.iter().map(|n| {
        if n.is_person { n.y - PERSON_HEAD_RADIUS } else { n.y }
    }).min().unwrap_or(0).min(0);
    let right = nodes.iter().map(|n| n.x + BOX_W).max().unwrap_or(200);
    let bottom = nodes.iter().map(|n| n.y + BOX_H).max().unwrap_or(200);
    let title_h = 40;
    // Extra top padding so that person heads that protrude above y=0 are not clipped.
    let extra_top = if top < 0 { -top } else { 0 };
    let width = right + MARGIN;
    let height = bottom + MARGIN + title_h + extra_top;

    let mut svg = String::new();

    svg.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">"##,
        width = width,
        height = height
    ));
    svg.push('\n');

    // Defs: arrowhead marker
    svg.push_str(
        r##"  <defs>
    <marker id="arrowhead" markerWidth="10" markerHeight="7" refX="10" refY="3.5" orient="auto">
      <polygon points="0 0, 10 3.5, 0 7" fill="#555555"/>
    </marker>
  </defs>
"##,
    );

    // Background
    svg.push_str(&format!(
        r##"  <rect width="{}" height="{}" fill="#f8f8f8"/>
"##,
        width, height
    ));

    // Title
    svg.push_str(&format!(
        r##"  <text x="{}" y="28" font-family="Arial, sans-serif" font-size="18" font-weight="bold" fill="{}" text-anchor="middle">{}</text>
"##,
        width / 2,
        COLOR_TITLE,
        xml_escape(title)
    ));

    // Translate remaining elements downward by title_h + extra_top so that person
    // heads that protrude above their y coordinate are not clipped by the title area.
    svg.push_str(&format!(r##"  <g transform="translate(0,{})">"##, title_h + extra_top));
    svg.push('\n');

    // System boundary (if any)
    if let Some(b) = boundary {
        svg.push_str(&format!(
            r##"    <rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" stroke-width="1" stroke-dasharray="6,4" rx="4"/>
"##,
            b.x, b.y, b.w, b.h, COLOR_BOUNDARY_FILL, COLOR_BOUNDARY_STROKE
        ));
        svg.push_str(&format!(
            r##"    <text x="{}" y="{}" font-family="Arial, sans-serif" font-size="11" fill="{}" font-style="italic">{}</text>
"##,
            b.x + 6,
            b.y + 13,
            COLOR_SYSTEM_EXT,
            xml_escape(&b.label)
        ));
    }

    // Edges (drawn before nodes so nodes appear on top)
    for edge in edges {
        let src = match pos.get(edge.src_id.as_str()) {
            Some(n) => n,
            None => continue,
        };
        let dst = match pos.get(edge.dst_id.as_str()) {
            Some(n) => n,
            None => continue,
        };

        let (x1, y1) = edge_point(src.cx(), src.cy(), dst.cx(), dst.cy());
        let (x2, y2) = edge_point(dst.cx(), dst.cy(), src.cx(), src.cy());

        svg.push_str(&format!(
            r##"    <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1.5" marker-end="url(#arrowhead)"/>
"##,
            x1, y1, x2, y2, COLOR_ARROW
        ));

        // Edge label
        let lx = (x1 + x2) / 2;
        let ly = (y1 + y2) / 2;
        if !edge.label.is_empty() || !edge.technology.is_empty() {
            let label_text = if edge.technology.is_empty() {
                edge.label.clone()
            } else if edge.label.is_empty() {
                format!("[{}]", edge.technology)
            } else {
                format!("{} [{}]", edge.label, edge.technology)
            };
            svg.push_str(&format!(
                r##"    <text x="{}" y="{}" font-family="Arial, sans-serif" font-size="10" fill="{}" text-anchor="middle" dy="-3">{}</text>
"##,
                lx,
                ly,
                COLOR_ARROW,
                xml_escape(&label_text)
            ));
        }
    }

    // Nodes
    for node in nodes {
        render_node(&mut svg, node);
    }

    svg.push_str("  </g>\n");
    svg.push_str("</svg>\n");
    svg
}

/// Render a single node box (person or system/container).
fn render_node(svg: &mut String, node: &Node) {
    let x = node.x;
    let y = node.y;

    if node.is_person {
        render_person_shape(svg, node);
    } else {
        svg.push_str(&format!(
            r##"    <rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" stroke-width="1.5" rx="4"/>
"##,
            x, y, BOX_W, BOX_H, node.fill, node.stroke
        ));
    }

    // Name text — centred inside the box for both person and non-person nodes.
    // For person nodes the head protrudes above y, so the box interior still starts at y.
    let text_x = x + BOX_W / 2;
    let name_y = y + BOX_H / 2 - 6;

    svg.push_str(&format!(
        r##"    <text x="{}" y="{}" font-family="Arial, sans-serif" font-size="13" font-weight="bold" fill="{}" text-anchor="middle">{}</text>
"##,
        text_x,
        name_y,
        node.text_color,
        xml_escape(&node.name)
    ));

    // Type label
    svg.push_str(&format!(
        r##"    <text x="{}" y="{}" font-family="Arial, sans-serif" font-size="10" fill="{}" text-anchor="middle" font-style="italic">[{}]</text>
"##,
        text_x,
        name_y + 15,
        node.text_color,
        xml_escape(&node.type_label)
    ));
}

/// Render a person shape: a circle (head) above the box, in the classic C4 style.
fn render_person_shape(svg: &mut String, node: &Node) {
    let x = node.x;
    let y = node.y;
    let cx = x + BOX_W / 2;

    // Draw background box (body)
    svg.push_str(&format!(
        r##"    <rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" stroke-width="1.5" rx="4"/>
"##,
        x, y, BOX_W, BOX_H, node.fill, node.stroke
    ));

    // Head circle centred just above the box top — classic C4 person glyph.
    // The circle centre is at y - head_r so the circle bottom edge touches the box top.
    let head_r = PERSON_HEAD_RADIUS;
    let head_cy = y - head_r;
    svg.push_str(&format!(
        r##"    <circle cx="{}" cy="{}" r="{}" fill="{}" stroke="{}" stroke-width="1.5"/>
"##,
        cx, head_cy, head_r, node.fill, node.stroke
    ));
}

// ── Geometry helpers ──────────────────────────────────────────────────────────

/// Returns the point on the edge of a BOX_W×BOX_H box centred at (cx, cy)
/// in the direction of (tx, ty).
fn edge_point(cx: i32, cy: i32, tx: i32, ty: i32) -> (i32, i32) {
    let dx = tx - cx;
    let dy = ty - cy;
    if dx == 0 && dy == 0 {
        return (cx, cy);
    }
    let hw = BOX_W as f64 / 2.0;
    let hh = BOX_H as f64 / 2.0;
    let fdx = dx as f64;
    let fdy = dy as f64;
    // Scale factor so the point lies exactly on the box boundary
    let scale = if fdx.abs() * hh > fdy.abs() * hw {
        hw / fdx.abs()
    } else {
        hh / fdy.abs()
    };
    let ex = cx as f64 + fdx * scale;
    let ey = cy as f64 + fdy * scale;
    (ex.round() as i32, ey.round() as i32)
}

// ── Utility ───────────────────────────────────────────────────────────────────

/// Factor by which each RGB channel is scaled when computing a darker stroke colour.
const DARKEN_FACTOR: f64 = 0.7;

/// Produce a slightly darker hex colour for strokes.
fn darken(hex: &str) -> String {
    if let Some(s) = try_darken_hex(hex) {
        return s;
    }
    "#333333".to_string()
}

/// Parse a 6-digit hex colour and reduce each channel by `DARKEN_FACTOR`.
fn try_darken_hex(hex: &str) -> Option<String> {
    let h = hex.trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&h[0..2], 16).ok()?;
    let g = u8::from_str_radix(&h[2..4], 16).ok()?;
    let b = u8::from_str_radix(&h[4..6], 16).ok()?;
    Some(format!(
        "#{:02X}{:02X}{:02X}",
        (r as f64 * DARKEN_FACTOR) as u8,
        (g as f64 * DARKEN_FACTOR) as u8,
        (b as f64 * DARKEN_FACTOR) as u8,
    ))
}

/// Extract the workspace-level element styles, if any.
fn get_styles(workspace: &Workspace) -> Option<&Styles> {
    workspace.views.configuration.as_ref()?.styles.as_ref()
}

/// Resolved fill / stroke / text colours for a single node.
struct ResolvedNodeStyle {
    fill: String,
    stroke: String,
    text_color: String,
}

/// Compute the effective colours for a node by walking its comma-separated tag
/// list and applying any matching `ElementStyle` entries in order (first tag =
/// lowest priority, last tag = highest priority).
fn resolve_node_style(
    tags: Option<&str>,
    default_type_tag: &str,
    styles: Option<&Styles>,
    default_fill: &str,
    default_text: &str,
) -> ResolvedNodeStyle {
    let mut fill = default_fill.to_string();
    let mut text_color = default_text.to_string();
    let mut stroke: Option<String> = None;

    if let Some(styles) = styles {
        if let Some(element_styles) = &styles.elements {
            let owned;
            let tags_str: &str = match tags {
                Some(t) => t,
                None => {
                    owned = format!("Element,{}", default_type_tag);
                    &owned
                }
            };
            for tag in tags_str.split(',').map(|t| t.trim()) {
                for style in element_styles {
                    if style.tag.eq_ignore_ascii_case(tag) {
                        if let Some(bg) = &style.background {
                            fill = bg.clone();
                        }
                        if let Some(color) = &style.color {
                            text_color = color.clone();
                        }
                        if let Some(s) = &style.stroke {
                            stroke = Some(s.clone());
                        }
                    }
                }
            }
        }
    }

    let stroke = stroke.unwrap_or_else(|| darken(&fill));
    ResolvedNodeStyle { fill, stroke, text_color }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use structurizr_model::{
        Container, ContainerView, Person, SoftwareSystem, SystemContextView, SystemLandscapeView,
        Workspace,
    };

    fn basic_workspace() -> Workspace {
        let mut workspace = Workspace::default();
        workspace.name = "Test".to_string();

        let person = Person {
            id: "1".to_string(),
            name: "Alice".to_string(),
            ..Default::default()
        };
        let system = SoftwareSystem {
            id: "2".to_string(),
            name: "My System".to_string(),
            ..Default::default()
        };
        workspace.model.people = Some(vec![person]);
        workspace.model.software_systems = Some(vec![system]);
        workspace
    }

    #[test]
    fn svg_exporter_system_landscape() {
        let mut workspace = basic_workspace();
        workspace.views.system_landscape_views = Some(vec![SystemLandscapeView {
            key: Some("Landscape".to_string()),
            ..Default::default()
        }]);

        let exporter = SvgExporter;
        let diagrams = exporter.export_workspace(&workspace);
        assert_eq!(diagrams.len(), 1);
        let svg = &diagrams[0].content;
        assert!(svg.starts_with("<svg"), "should be an SVG document");
        assert!(svg.contains("Alice"));
        assert!(svg.contains("My System"));
        assert_eq!(diagrams[0].format, DiagramFormat::Svg);
        assert_eq!(diagrams[0].extension(), "svg");
    }

    #[test]
    fn svg_exporter_system_context() {
        let mut workspace = basic_workspace();
        workspace.views.system_context_views = Some(vec![SystemContextView {
            software_system_id: "2".to_string(),
            key: Some("Context".to_string()),
            ..Default::default()
        }]);

        let exporter = SvgExporter;
        let diagrams = exporter.export_workspace(&workspace);
        assert_eq!(diagrams.len(), 1);
        let svg = &diagrams[0].content;
        assert!(svg.contains("Alice"));
        assert!(svg.contains("My System"));
        // Focal system should use the primary blue; external grey should not appear for a
        // single-system workspace.
        assert!(svg.contains(COLOR_SYSTEM));
    }

    #[test]
    fn svg_exporter_container_view() {
        let mut workspace = Workspace::default();
        workspace.name = "ContainerTest".to_string();

        let container = Container {
            id: "3".to_string(),
            name: "API".to_string(),
            technology: Some("Rust".to_string()),
            ..Default::default()
        };
        let system = SoftwareSystem {
            id: "2".to_string(),
            name: "My System".to_string(),
            containers: Some(vec![container]),
            ..Default::default()
        };
        workspace.model.software_systems = Some(vec![system]);
        workspace.views.container_views = Some(vec![ContainerView {
            software_system_id: "2".to_string(),
            key: Some("Containers".to_string()),
            ..Default::default()
        }]);

        let exporter = SvgExporter;
        let diagrams = exporter.export_workspace(&workspace);
        assert_eq!(diagrams.len(), 1);
        let svg = &diagrams[0].content;
        assert!(svg.contains("API"));
        assert!(svg.contains("Rust"));
        assert!(svg.contains(COLOR_CONTAINER));
    }

    #[test]
    fn svg_exporter_relationships() {
        let mut workspace = Workspace::default();
        workspace.name = "RelTest".to_string();

        let rel = Relationship {
            id: "r1".to_string(),
            source_id: "1".to_string(),
            destination_id: "2".to_string(),
            description: Some("Uses".to_string()),
            ..Default::default()
        };
        let person = Person {
            id: "1".to_string(),
            name: "Alice".to_string(),
            relationships: Some(vec![rel]),
            ..Default::default()
        };
        let system = SoftwareSystem {
            id: "2".to_string(),
            name: "My System".to_string(),
            ..Default::default()
        };
        workspace.model.people = Some(vec![person]);
        workspace.model.software_systems = Some(vec![system]);
        workspace.views.system_landscape_views = Some(vec![SystemLandscapeView {
            key: Some("Landscape".to_string()),
            ..Default::default()
        }]);

        let exporter = SvgExporter;
        let diagrams = exporter.export_workspace(&workspace);
        let svg = &diagrams[0].content;
        assert!(svg.contains("Uses"), "relationship label should appear in SVG");
        assert!(svg.contains("arrowhead"), "arrowhead marker should be present");
    }

    #[test]
    fn xml_escape_chars() {
        assert_eq!(xml_escape("a&b<c>d\"e'f"), "a&amp;b&lt;c&gt;d&quot;e&#39;f");
    }

    #[test]
    fn edge_point_horizontal() {
        // Node centred at (100, 100), target to the right at (300, 100)
        let (ex, ey) = edge_point(100, 100, 300, 100);
        assert_eq!(ex, 100 + BOX_W / 2);
        assert_eq!(ey, 100);
    }

    #[test]
    fn edge_point_vertical() {
        let (ex, ey) = edge_point(100, 100, 100, 300);
        assert_eq!(ex, 100);
        assert_eq!(ey, 100 + BOX_H / 2);
    }

    #[test]
    fn svg_exporter_respects_element_styles() {
        use structurizr_model::{ElementStyle, Styles, ViewConfiguration};

        let mut workspace = basic_workspace();
        workspace.views.system_landscape_views = Some(vec![SystemLandscapeView {
            key: Some("Landscape".to_string()),
            ..Default::default()
        }]);

        // Override the Person fill colour to red and text to black.
        let elem_style = ElementStyle {
            tag: "Person".to_string(),
            background: Some("#FF0000".to_string()),
            color: Some("#000000".to_string()),
            ..Default::default()
        };
        workspace.views.configuration = Some(ViewConfiguration {
            styles: Some(Styles {
                elements: Some(vec![elem_style]),
                ..Default::default()
            }),
            ..Default::default()
        });

        let exporter = SvgExporter;
        let diagrams = exporter.export_workspace(&workspace);
        let svg = &diagrams[0].content;

        assert!(svg.contains("#FF0000"), "custom background colour should appear in SVG");
        assert!(svg.contains("#000000"), "custom text colour should appear in SVG");
        // The default person blue should NOT appear since it was overridden.
        assert!(!svg.contains(COLOR_PERSON), "default person colour should be replaced");
    }

    #[test]
    fn svg_exporter_custom_stroke_overrides_darken() {
        use structurizr_model::{ElementStyle, Styles, ViewConfiguration};

        let mut workspace = basic_workspace();
        workspace.views.system_landscape_views = Some(vec![SystemLandscapeView {
            key: Some("Landscape".to_string()),
            ..Default::default()
        }]);

        let elem_style = ElementStyle {
            tag: "Software System".to_string(),
            stroke: Some("#ABCDEF".to_string()),
            ..Default::default()
        };
        workspace.views.configuration = Some(ViewConfiguration {
            styles: Some(Styles {
                elements: Some(vec![elem_style]),
                ..Default::default()
            }),
            ..Default::default()
        });

        let exporter = SvgExporter;
        let diagrams = exporter.export_workspace(&workspace);
        let svg = &diagrams[0].content;

        assert!(svg.contains("#ABCDEF"), "custom stroke colour should appear in SVG");
    }

    #[test]
    fn darken_hex_colour() {
        // 0xFF * 0.7 = 0xB2
        let darkened = darken("#FFFFFF");
        assert_eq!(darkened, "#B2B2B2");

        // Unknown format falls back to a safe dark colour.
        let fallback = darken("invalid");
        assert_eq!(fallback, "#333333");
    }

    // ── Element filtering & stored-position tests ─────────────────────────────

    #[test]
    fn element_filter_excludes_unlisted_elements() {
        // Workspace has two systems but the view only lists one.
        let mut workspace = Workspace::default();
        workspace.name = "FilterTest".to_string();
        workspace.model.software_systems = Some(vec![
            SoftwareSystem { id: "1".to_string(), name: "Alpha".to_string(), ..Default::default() },
            SoftwareSystem { id: "2".to_string(), name: "Beta".to_string(), ..Default::default() },
        ]);
        workspace.views.system_landscape_views = Some(vec![SystemLandscapeView {
            key: Some("Landscape".to_string()),
            element_views: Some(vec![ElementView { id: "1".to_string(), x: None, y: None }]),
            ..Default::default()
        }]);

        let exporter = SvgExporter;
        let diagrams = exporter.export_workspace(&workspace);
        let svg = &diagrams[0].content;

        assert!(svg.contains("Alpha"), "included element should appear");
        assert!(!svg.contains("Beta"), "excluded element must NOT appear");
    }

    #[test]
    fn stored_positions_used_when_present() {
        // View provides explicit x/y — auto-layout must NOT be used.
        let mut workspace = Workspace::default();
        workspace.name = "PosTest".to_string();
        workspace.model.software_systems = Some(vec![
            SoftwareSystem { id: "1".to_string(), name: "Sys".to_string(), ..Default::default() },
        ]);
        workspace.views.system_landscape_views = Some(vec![SystemLandscapeView {
            key: Some("Landscape".to_string()),
            element_views: Some(vec![ElementView { id: "1".to_string(), x: Some(500), y: Some(300) }]),
            ..Default::default()
        }]);

        let exporter = SvgExporter;
        let diagrams = exporter.export_workspace(&workspace);
        let svg = &diagrams[0].content;

        // The stored coordinates must appear in the rect element.
        assert!(svg.contains(r#"x="500""#), "stored x=500 should be in SVG");
        assert!(svg.contains(r#"y="300""#), "stored y=300 should be in SVG");
    }

    #[test]
    fn person_head_circle_above_box() {
        // With the fix, head_cy = node.y - PERSON_HEAD_RADIUS.
        // For a single person laid out by the grid: node.y = MARGIN = 60.
        // Expected: head_cy = 60 - 12 = 48, rect y = 60 (48 < 60 ⇒ head is above box).
        let mut workspace = Workspace::default();
        workspace.name = "PersonTest".to_string();
        workspace.model.people = Some(vec![
            Person { id: "1".to_string(), name: "Bob".to_string(), ..Default::default() },
        ]);
        workspace.views.system_landscape_views = Some(vec![SystemLandscapeView {
            key: Some("Landscape".to_string()),
            ..Default::default()
        }]);

        let exporter = SvgExporter;
        let diagrams = exporter.export_workspace(&workspace);
        let svg = &diagrams[0].content;

        assert!(svg.contains("<circle"), "person shape must include a circle");
        // Head circle centre must be above the box top: cy = MARGIN - PERSON_HEAD_RADIUS = 48.
        assert!(
            svg.contains(r#"cy="48""#),
            "head circle cy should be {} (MARGIN - PERSON_HEAD_RADIUS)",
            MARGIN - PERSON_HEAD_RADIUS
        );
        // The person box rect must have y = MARGIN = 60 (strictly greater than cy=48).
        assert!(
            svg.contains(r#"y="60""#),
            "person box rect y should be MARGIN = {}", MARGIN
        );
    }

    #[test]
    fn relationship_filter_excludes_unlisted_edges() {
        use structurizr_model::Relationship;

        let mut workspace = Workspace::default();
        workspace.name = "RelFilterTest".to_string();
        let rel1 = Relationship {
            id: "r1".to_string(),
            source_id: "1".to_string(),
            destination_id: "2".to_string(),
            description: Some("Sends".to_string()),
            ..Default::default()
        };
        let rel2 = Relationship {
            id: "r2".to_string(),
            source_id: "2".to_string(),
            destination_id: "1".to_string(),
            description: Some("Replies".to_string()),
            ..Default::default()
        };
        workspace.model.people = Some(vec![
            Person { id: "1".to_string(), name: "Alice".to_string(), relationships: Some(vec![rel1]), ..Default::default() },
        ]);
        workspace.model.software_systems = Some(vec![
            SoftwareSystem { id: "2".to_string(), name: "System".to_string(), relationships: Some(vec![rel2]), ..Default::default() },
        ]);
        workspace.views.system_landscape_views = Some(vec![SystemLandscapeView {
            key: Some("Landscape".to_string()),
            relationship_views: Some(vec![
                RelationshipView { id: "r1".to_string(), ..Default::default() },
            ]),
            ..Default::default()
        }]);

        let exporter = SvgExporter;
        let diagrams = exporter.export_workspace(&workspace);
        let svg = &diagrams[0].content;

        assert!(svg.contains("Sends"), "included relationship label should appear");
        assert!(!svg.contains("Replies"), "excluded relationship label must NOT appear");
    }
}
