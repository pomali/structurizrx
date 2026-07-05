use std::collections::{HashMap, HashSet};

use structurizr_model::*;

use crate::diagram::{Diagram, DiagramFormat};
use crate::exporter::DiagramExporter;

// ── Layout constants ─────────────────────────────────────────────────────────

const NODE_W: i32 = 230;
const NODE_MIN_H: i32 = 70;
const PAD_X: f64 = 14.0;
const H_GAP: i32 = 44;
const V_GAP: i32 = 96;
const MARGIN: i32 = 40;
const TITLE_H: i32 = 48;
const GRID_COLS: usize = 4;
const BOUNDARY_PAD: i32 = 26;
const BOUNDARY_LABEL_HEIGHT: i32 = 18;
const PERSON_HEAD_RADIUS: i32 = 17;
/// The head circle centre sits this far above the box top edge.
const PERSON_HEAD_OVERLAP: i32 = 6;
/// Perpendicular spacing between parallel edges connecting the same node pair.
const EDGE_SPREAD: f64 = 30.0;
/// Maximum pixel width of a wrapped edge-label line.
const EDGE_LABEL_W: f64 = 180.0;

// Font sizes / line heights (Arial-ish metrics).
const FS_TITLE: f64 = 18.0;
const FS_NAME: f64 = 14.0;
const FS_META: f64 = 11.0;
const FS_DESC: f64 = 11.0;
const FS_EDGE: f64 = 10.5;
const LH_NAME: i32 = 18;
const LH_SMALL: i32 = 14;

// ── C4 colour palette ────────────────────────────────────────────────────────

const COLOR_PERSON: &str = "#08427B";
const COLOR_SYSTEM: &str = "#1168BD";
const COLOR_SYSTEM_EXT: &str = "#999999";
const COLOR_CONTAINER: &str = "#438DD5";
const COLOR_TEXT_LIGHT: &str = "#ffffff";
const COLOR_BOUNDARY_FILL: &str = "#ffffff";
const COLOR_BOUNDARY_STROKE: &str = "#bbbbbb";
const COLOR_ARROW: &str = "#555555";
const COLOR_TITLE: &str = "#333333";
const COLOR_BG: &str = "#f8f8f8";

// ── Internal node/edge types ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Node {
    id: String,
    fill: String,
    stroke: String,
    text_color: String,
    is_person: bool,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    /// Height of the wrapped text block, used to vertically centre it in the box.
    content_h: i32,
    name_lines: Vec<String>,
    meta_lines: Vec<String>,
    desc_lines: Vec<String>,
    /// stroke-dasharray for sketchy/uncertain elements (spec §4.2 theming).
    dash: Option<&'static str>,
}

impl Node {
    fn cx(&self) -> i32 {
        self.x + self.w / 2
    }
    fn cy(&self) -> i32 {
        self.y + self.h / 2
    }
    /// Vertical extent above `y` (person head protrusion).
    fn top_overhang(&self) -> i32 {
        if self.is_person {
            PERSON_HEAD_RADIUS + PERSON_HEAD_OVERLAP
        } else {
            0
        }
    }
}

#[derive(Debug, Clone)]
struct Edge {
    rel_id: String,
    src_id: String,
    dst_id: String,
    label: String,
    technology: String,
    /// stroke-dasharray: async-family kinds and uncertain relationships render dashed.
    dash: Option<&'static str>,
    /// Port ids from the relationship (resolved to names by annotate_edge_ports).
    src_port_id: Option<String>,
    dst_port_id: Option<String>,
    src_port_name: Option<String>,
    dst_port_name: Option<String>,
    /// Stroke colour override (delta views mark added/removed edges).
    color: Option<String>,
}

/// Default dash theming: ideas/drafts and placeholders render sketchy (spec §4.2).
fn dash_for(status: Option<structurizr_model::Status>, tags: Option<&str>) -> Option<&'static str> {
    let has_tag = |t: &str| {
        tags.map(|ts| ts.split(',').any(|x| x.trim() == t)).unwrap_or(false)
    };
    if has_tag("Placeholder") || has_tag("Uncertain") {
        return Some("3,4");
    }
    match status {
        Some(structurizr_model::Status::Idea) | Some(structurizr_model::Status::Draft) => Some("8,5"),
        _ => None,
    }
}

/// Async-family relationship kinds render dashed, matching common C4 practice.
fn edge_dash(rel: &Relationship) -> Option<&'static str> {
    if rel.tags.as_deref().map(|ts| ts.split(',').any(|x| x.trim() == "Uncertain")).unwrap_or(false) {
        return Some("3,4");
    }
    use structurizr_model::RelationshipKind::*;
    match rel.kind {
        Some(Async) | Some(Publish) | Some(Subscribe) | Some(Dataflow) => Some("7,5"),
        _ => None,
    }
}

// ── Text measurement & wrapping ──────────────────────────────────────────────

/// Approximate advance width of a character as a fraction of the font size
/// (Arial-like proportions — close enough for box sizing and label halos).
fn char_w(c: char) -> f64 {
    match c {
        'i' | 'l' | 'j' | '!' | '\'' | '|' | '.' | ',' | ':' | ';' => 0.30,
        'f' | 't' | 'r' | 'I' | '(' | ')' | '[' | ']' | '{' | '}' | '/' | '\\' | ' ' => 0.40,
        'm' | 'w' => 0.82,
        'M' | 'W' => 0.95,
        'A'..='Z' => 0.68,
        '0'..='9' => 0.56,
        _ => 0.54,
    }
}

fn text_width(s: &str, font_size: f64) -> f64 {
    s.chars().map(char_w).sum::<f64>() * font_size
}

/// Greedy word wrap to `max_w` pixels; words wider than a line are hard-split.
fn wrap_text(text: &str, max_w: f64, font_size: f64) -> Vec<String> {
    // First break oversized words into fragments that fit on a line.
    let mut words: Vec<String> = Vec::new();
    for w in text.split_whitespace() {
        if text_width(w, font_size) <= max_w {
            words.push(w.to_string());
            continue;
        }
        let mut piece = String::new();
        for ch in w.chars() {
            piece.push(ch);
            if text_width(&piece, font_size) > max_w && piece.chars().count() > 1 {
                let last = piece.pop().unwrap();
                words.push(std::mem::take(&mut piece));
                piece.push(last);
            }
        }
        if !piece.is_empty() {
            words.push(piece);
        }
    }

    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for w in words {
        if cur.is_empty() {
            cur = w;
            continue;
        }
        let cand = format!("{cur} {w}");
        if text_width(&cand, font_size) <= max_w {
            cur = cand;
        } else {
            lines.push(std::mem::take(&mut cur));
            cur = w;
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Truncate to `max` lines, ellipsizing the last kept line.
fn clamp_lines(mut lines: Vec<String>, max: usize) -> Vec<String> {
    if lines.len() > max {
        lines.truncate(max);
        if let Some(last) = lines.last_mut() {
            last.push('…');
        }
    }
    lines
}

// ── Node construction ────────────────────────────────────────────────────────

/// Build a sized node: wrap the name/type/description and derive box height.
fn make_node(
    id: &str,
    name: &str,
    type_label: &str,
    description: Option<&str>,
    style: ResolvedNodeStyle,
    is_person: bool,
    dash: Option<&'static str>,
) -> Node {
    let inner = NODE_W as f64 - 2.0 * PAD_X;
    let name_lines = clamp_lines(wrap_text(name, inner, FS_NAME), 3);
    let meta_lines = clamp_lines(wrap_text(&format!("[{type_label}]"), inner, FS_META), 2);
    let desc_lines = match description {
        Some(d) if !d.trim().is_empty() => clamp_lines(wrap_text(d, inner, FS_DESC), 3),
        _ => Vec::new(),
    };
    let mut content_h =
        name_lines.len() as i32 * LH_NAME + meta_lines.len() as i32 * LH_SMALL;
    if !desc_lines.is_empty() {
        content_h += 6 + desc_lines.len() as i32 * LH_SMALL;
    }
    let h = (content_h + 26).max(NODE_MIN_H);
    Node {
        id: id.to_string(),
        fill: style.fill,
        stroke: style.stroke,
        text_color: style.text_color,
        is_person,
        x: 0,
        y: 0,
        w: NODE_W,
        h,
        content_h,
        name_lines,
        meta_lines,
        desc_lines,
        dash,
    }
}

fn person_node(p: &Person, styles: Option<&Styles>) -> Node {
    let s = resolve_node_style(p.tags.as_deref(), "Person", styles, COLOR_PERSON, COLOR_TEXT_LIGHT);
    make_node(
        &p.id,
        &p.name,
        "Person",
        p.description.as_deref(),
        s,
        true,
        dash_for(p.status, p.tags.as_deref()),
    )
}

fn system_node(ss: &SoftwareSystem, styles: Option<&Styles>, default_fill: &str) -> Node {
    let s = resolve_node_style(ss.tags.as_deref(), "Software System", styles, default_fill, COLOR_TEXT_LIGHT);
    make_node(
        &ss.id,
        &ss.name,
        "Software System",
        ss.description.as_deref(),
        s,
        false,
        dash_for(ss.status, ss.tags.as_deref()),
    )
}

fn container_node(c: &Container, styles: Option<&Styles>) -> Node {
    let type_label = match c.technology.as_deref().filter(|t| !t.is_empty()) {
        Some(t) => format!("Container: {t}"),
        None => "Container".to_string(),
    };
    let s = resolve_node_style(c.tags.as_deref(), "Container", styles, COLOR_CONTAINER, COLOR_TEXT_LIGHT);
    make_node(
        &c.id,
        &c.name,
        &type_label,
        c.description.as_deref(),
        s,
        false,
        dash_for(c.status, c.tags.as_deref()),
    )
}

fn component_node(comp: &Component, styles: Option<&Styles>) -> Node {
    let type_label = match comp.technology.as_deref().filter(|t| !t.is_empty()) {
        Some(t) => format!("Component: {t}"),
        None => "Component".to_string(),
    };
    let s = resolve_node_style(comp.tags.as_deref(), "Component", styles, COLOR_CONTAINER, COLOR_TEXT_LIGHT);
    make_node(
        &comp.id,
        &comp.name,
        &type_label,
        comp.description.as_deref(),
        s,
        false,
        dash_for(comp.status, comp.tags.as_deref()),
    )
}

fn custom_node(ce: &CustomElement, styles: Option<&Styles>) -> Node {
    let s = resolve_node_style(ce.tags.as_deref(), "Element", styles, COLOR_SYSTEM_EXT, COLOR_TEXT_LIGHT);
    make_node(
        &ce.id,
        &ce.name,
        "Element",
        ce.description.as_deref(),
        s,
        false,
        dash_for(ce.status, ce.tags.as_deref()),
    )
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

// ── View scoping helpers ─────────────────────────────────────────────────────

/// Every relationship in the model, at any nesting level.
fn all_relationships(model: &Model) -> Vec<&Relationship> {
    let mut out = Vec::new();
    for p in model.people.iter().flatten() {
        out.extend(p.relationships.iter().flatten());
    }
    for ss in model.software_systems.iter().flatten() {
        out.extend(ss.relationships.iter().flatten());
        for c in ss.containers.iter().flatten() {
            out.extend(c.relationships.iter().flatten());
            for comp in c.components.iter().flatten() {
                out.extend(comp.relationships.iter().flatten());
            }
        }
    }
    for ce in model.custom_elements.iter().flatten() {
        out.extend(ce.relationships.iter().flatten());
    }
    out
}

/// Map every element id to its top-level owner (person / software system / custom element).
fn top_level_owner(model: &Model) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for p in model.people.iter().flatten() {
        map.insert(p.id.clone(), p.id.clone());
    }
    for ss in model.software_systems.iter().flatten() {
        map.insert(ss.id.clone(), ss.id.clone());
        for c in ss.containers.iter().flatten() {
            map.insert(c.id.clone(), ss.id.clone());
            for comp in c.components.iter().flatten() {
                map.insert(comp.id.clone(), ss.id.clone());
            }
        }
    }
    for ce in model.custom_elements.iter().flatten() {
        map.insert(ce.id.clone(), ce.id.clone());
    }
    map
}

/// Elements in scope for a system context view without an explicit element list:
/// the focal system plus every top-level element related to it (directly or via
/// a relationship involving one of its containers/components).
fn context_scope(model: &Model, focal_id: &str) -> HashSet<String> {
    let owner = top_level_owner(model);
    let mut scope = HashSet::new();
    scope.insert(focal_id.to_string());
    for r in all_relationships(model) {
        if let (Some(src), Some(dst)) = (owner.get(&r.source_id), owner.get(&r.destination_id)) {
            if src == focal_id && dst != focal_id {
                scope.insert(dst.clone());
            } else if dst == focal_id && src != focal_id {
                scope.insert(src.clone());
            }
        }
    }
    scope
}

/// Elements in scope for a container view without an explicit element list:
/// the focal system's containers plus related people/external systems.
fn container_scope(model: &Model, focal_id: &str) -> HashSet<String> {
    let mut scope = context_scope(model, focal_id);
    scope.remove(focal_id);
    for ss in model.software_systems.iter().flatten() {
        if ss.id == focal_id {
            for c in ss.containers.iter().flatten() {
                scope.insert(c.id.clone());
            }
        }
    }
    scope
}

/// Map each container/component id to its parent (component → container,
/// container → software system).  Top-level elements have no entry.
fn child_parent_map(model: &Model) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for ss in model.software_systems.iter().flatten() {
        for c in ss.containers.iter().flatten() {
            map.insert(c.id.clone(), ss.id.clone());
            for comp in c.components.iter().flatten() {
                map.insert(comp.id.clone(), c.id.clone());
            }
        }
    }
    map
}

/// Lift edge endpoints to their nearest **visible** ancestor — the implied
/// relationships of upstream Structurizr.  A component→system relationship
/// renders as container→system in a container view, and so on.  Edges whose
/// endpoints cannot be lifted into the view, or that collapse onto a single
/// node, are dropped; lifted duplicates (same endpoints, label and technology)
/// are deduplicated.
fn lift_edges(edges: Vec<Edge>, model: &Model, visible: &HashSet<String>) -> Vec<Edge> {
    let parents = child_parent_map(model);
    let lift = |id: &str| -> Option<String> {
        let mut cur = id.to_string();
        loop {
            if visible.contains(&cur) {
                return Some(cur);
            }
            cur = parents.get(&cur)?.clone();
        }
    };
    let mut out = Vec::new();
    let mut seen: HashSet<(String, String, String, String)> = HashSet::new();
    for mut e in edges {
        let (Some(src), Some(dst)) = (lift(&e.src_id), lift(&e.dst_id)) else {
            continue;
        };
        if src == dst {
            continue;
        }
        if !seen.insert((src.clone(), dst.clone(), e.label.clone(), e.technology.clone())) {
            continue;
        }
        // A lifted endpoint is no longer the element the port belongs to.
        if src != e.src_id {
            e.src_port_id = None;
            e.src_port_name = None;
        }
        if dst != e.dst_id {
            e.dst_port_id = None;
            e.dst_port_name = None;
        }
        e.src_id = src;
        e.dst_id = dst;
        out.push(e);
    }
    out
}

// ── Per-view renderers ────────────────────────────────────────────────────────

fn render_landscape(title: &str, view: &SystemLandscapeView, workspace: &Workspace) -> String {
    let model = &workspace.model;
    let styles = get_styles(workspace);
    let (elem_filter, elem_pos) = build_element_filter(view.element_views.as_deref());
    let rel_filter = build_rel_filter(view.relationship_views.as_deref());
    let mut nodes: Vec<Node> = Vec::new();

    for p in model.people.iter().flatten() {
        if elem_allowed(&elem_filter, &p.id) {
            nodes.push(person_node(p, styles));
        }
    }
    for ss in model.software_systems.iter().flatten() {
        if elem_allowed(&elem_filter, &ss.id) {
            nodes.push(system_node(ss, styles, COLOR_SYSTEM));
        }
    }

    // Generated views (focus/slice/layer/lint/…) are landscape-shaped but may
    // reference containers, components, or custom elements. When the view has
    // an explicit element list, include those too; without a filter the
    // landscape keeps its classic people+systems scope.
    if elem_filter.is_some() {
        for ss in model.software_systems.iter().flatten() {
            for c in ss.containers.iter().flatten() {
                if elem_allowed(&elem_filter, &c.id) {
                    nodes.push(container_node(c, styles));
                }
                for comp in c.components.iter().flatten() {
                    if elem_allowed(&elem_filter, &comp.id) {
                        nodes.push(component_node(comp, styles));
                    }
                }
            }
        }
        for ce in model.custom_elements.iter().flatten() {
            if elem_allowed(&elem_filter, &ce.id) {
                nodes.push(custom_node(ce, styles));
            }
        }
    }

    let edges = if elem_filter.is_some() {
        collect_all_edges_with_containers(model, rel_filter.as_ref())
    } else {
        collect_all_edges(model, rel_filter.as_ref())
    };
    let visible: HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();
    let mut edges = lift_edges(edges, model, &visible);
    apply_delta_styling(view.properties.as_ref(), &mut nodes, &mut edges);

    // Only use auto-layout when no stored positions are available.  Running layout
    // when some nodes already have explicit coordinates would overwrite those values.
    let positioned = apply_stored_positions(&mut nodes, &elem_pos);
    if positioned == 0 {
        layout(&mut nodes, &edges, None);
    }
    render_svg(title, &nodes, &edges, None)
}

fn render_system_context(title: &str, view: &SystemContextView, workspace: &Workspace) -> String {
    let model = &workspace.model;
    let styles = get_styles(workspace);
    let focal_id = &view.software_system_id;
    let (mut elem_filter, elem_pos) = build_element_filter(view.element_views.as_deref());
    let rel_filter = build_rel_filter(view.relationship_views.as_deref());

    // Without an explicit element list, scope the view to the focal system and
    // its direct neighbours rather than dumping the whole model in.
    if elem_filter.is_none() {
        elem_filter = Some(context_scope(model, focal_id));
    }

    let mut nodes: Vec<Node> = Vec::new();
    for p in model.people.iter().flatten() {
        if elem_allowed(&elem_filter, &p.id) {
            nodes.push(person_node(p, styles));
        }
    }
    for ss in model.software_systems.iter().flatten() {
        if !elem_allowed(&elem_filter, &ss.id) {
            continue;
        }
        let fill = if &ss.id == focal_id { COLOR_SYSTEM } else { COLOR_SYSTEM_EXT };
        nodes.push(system_node(ss, styles, fill));
    }

    let edges = collect_all_edges_with_containers(model, rel_filter.as_ref());
    let visible: HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();
    let mut edges = lift_edges(edges, model, &visible);
    apply_delta_styling(view.properties.as_ref(), &mut nodes, &mut edges);
    let positioned = apply_stored_positions(&mut nodes, &elem_pos);
    if positioned == 0 {
        layout(&mut nodes, &edges, None);
    }
    render_svg(title, &nodes, &edges, None)
}

fn render_container_view(title: &str, view: &ContainerView, workspace: &Workspace) -> String {
    let model = &workspace.model;
    let styles = get_styles(workspace);
    let focal_id = &view.software_system_id;
    let (mut elem_filter, elem_pos) = build_element_filter(view.element_views.as_deref());
    let rel_filter = build_rel_filter(view.relationship_views.as_deref());

    if elem_filter.is_none() {
        elem_filter = Some(container_scope(model, focal_id));
    }

    let mut nodes: Vec<Node> = Vec::new();
    let mut container_ids: HashSet<String> = HashSet::new();
    let mut focal_system_name = String::new();

    for p in model.people.iter().flatten() {
        if elem_allowed(&elem_filter, &p.id) {
            nodes.push(person_node(p, styles));
        }
    }
    for ss in model.software_systems.iter().flatten() {
        if &ss.id == focal_id {
            focal_system_name = ss.name.clone();
            for c in ss.containers.iter().flatten() {
                if elem_allowed(&elem_filter, &c.id) {
                    container_ids.insert(c.id.clone());
                    nodes.push(container_node(c, styles));
                }
            }
        } else if elem_allowed(&elem_filter, &ss.id) {
            nodes.push(system_node(ss, styles, COLOR_SYSTEM_EXT));
        }
    }

    let edges = collect_all_edges_with_containers(model, rel_filter.as_ref());
    let visible: HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();
    let mut edges = lift_edges(edges, model, &visible);
    apply_delta_styling(view.properties.as_ref(), &mut nodes, &mut edges);
    let positioned = apply_stored_positions(&mut nodes, &elem_pos);
    if positioned == 0 {
        layout(&mut nodes, &edges, Some(&container_ids));
    }

    let boundary = if container_ids.is_empty() {
        None
    } else {
        let c_nodes: Vec<&Node> = nodes.iter().filter(|n| container_ids.contains(&n.id)).collect();
        Some(boundary_rect(&c_nodes, &focal_system_name))
    };

    render_svg(title, &nodes, &edges, boundary.as_ref())
}

// ── Layout ───────────────────────────────────────────────────────────────────

/// Effective height including the person head protrusion.
fn eff_h(n: &Node) -> i32 {
    n.h + n.top_overhang()
}

/// Total width of a horizontal run of nodes, including gaps.
fn group_w(nodes: &[Node], idxs: &[usize]) -> i32 {
    if idxs.is_empty() {
        return 0;
    }
    idxs.iter().map(|&i| nodes[i].w).sum::<i32>() + (idxs.len() as i32 - 1) * H_GAP
}

/// Place a run of nodes left-to-right starting at `start_x`, vertically centred
/// in a row of height `row_h` whose top edge is at `top_y`.
fn place_row(nodes: &mut [Node], idxs: &[usize], start_x: i32, top_y: i32, row_h: i32) {
    let mut x = start_x;
    for &i in idxs {
        let overhang = nodes[i].top_overhang();
        let eh = eff_h(&nodes[i]);
        nodes[i].x = x;
        nodes[i].y = top_y + (row_h - eh) / 2 + overhang;
        x += nodes[i].w + H_GAP;
    }
}

/// Find back edges via iterative DFS so cycles can be ignored during layering.
fn find_back_edges(n: usize, adj: &[Vec<usize>]) -> HashSet<(usize, usize)> {
    let mut state = vec![0u8; n]; // 0 = unvisited, 1 = on stack, 2 = done
    let mut back = HashSet::new();
    for start in 0..n {
        if state[start] != 0 {
            continue;
        }
        let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
        state[start] = 1;
        while let Some(&(u, i)) = stack.last() {
            if i < adj[u].len() {
                stack.last_mut().unwrap().1 += 1;
                let v = adj[u][i];
                match state[v] {
                    0 => {
                        state[v] = 1;
                        stack.push((v, 0));
                    }
                    1 => {
                        back.insert((u, v));
                    }
                    _ => {}
                }
            } else {
                state[u] = 2;
                stack.pop();
            }
        }
    }
    back
}

/// Assign x/y positions using a **hierarchical (layered) layout**:
///
/// 1. **Cycle breaking** — back edges found by DFS are ignored for layering,
///    so bidirectional relationships cannot inflate layer numbers.
/// 2. **Longest-path layering** over the remaining DAG.
/// 3. **Barycentric ordering** — alternating down/up sweeps reduce crossings.
/// 4. **Coordinate assignment** — rows are vertically stacked (row height =
///    tallest node) and horizontally centred on a common axis.  When
///    `boundary_ids` is given (container views), boundary members are grouped
///    contiguously per row and non-members that share a row with them are
///    pushed to the right of the boundary box.
///
/// Nodes without any visible edge are laid out in a grid below the layered part.
fn layout(nodes: &mut [Node], edges: &[Edge], boundary_ids: Option<&HashSet<String>>) {
    let n = nodes.len();
    if n == 0 {
        return;
    }

    let id_to_idx: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, node)| (node.id.as_str(), i))
        .collect();

    // Deduplicated directed pairs between visible nodes (self-loops excluded).
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    for e in edges {
        if let (Some(&s), Some(&d)) = (id_to_idx.get(e.src_id.as_str()), id_to_idx.get(e.dst_id.as_str())) {
            if s != d && seen.insert((s, d)) {
                pairs.push((s, d));
            }
        }
    }

    if pairs.is_empty() {
        layout_grid(nodes, GRID_COLS);
        return;
    }

    let mut connected = vec![false; n];
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for &(s, d) in &pairs {
        connected[s] = true;
        connected[d] = true;
        adj[s].push(d);
    }

    // 1. Cycle breaking + 2. longest-path layering over the DAG edges.
    let back = find_back_edges(n, &adj);
    let dag: Vec<(usize, usize)> = pairs.iter().copied().filter(|p| !back.contains(p)).collect();
    let mut layer = vec![0usize; n];
    for _ in 0..n {
        for &(s, d) in &dag {
            if layer[s] + 1 > layer[d] {
                layer[d] = layer[s] + 1;
            }
        }
    }

    let num_layers = (0..n)
        .filter(|&i| connected[i])
        .map(|i| layer[i])
        .max()
        .unwrap_or(0)
        + 1;
    let mut layers: Vec<Vec<usize>> = vec![Vec::new(); num_layers];
    for i in 0..n {
        if connected[i] {
            layers[layer[i]].push(i);
        }
    }

    // 3. Barycentric ordering: alternating down (predecessor) / up (successor) sweeps.
    let mut pos = vec![0usize; n];
    for row in &layers {
        for (p, &i) in row.iter().enumerate() {
            pos[i] = p;
        }
    }
    let mut preds: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut succs: Vec<Vec<usize>> = vec![Vec::new(); n];
    for &(s, d) in &pairs {
        preds[d].push(s);
        succs[s].push(d);
    }
    for sweep in 0..4 {
        let down = sweep % 2 == 0;
        let rows: Vec<usize> = if down {
            (1..num_layers).collect()
        } else {
            (0..num_layers.saturating_sub(1)).rev().collect()
        };
        for r in rows {
            if layers[r].len() <= 1 {
                continue;
            }
            let neighbours = if down { &preds } else { &succs };
            let mut scored: Vec<(usize, f64)> = layers[r]
                .iter()
                .map(|&i| {
                    let ns = &neighbours[i];
                    let score = if ns.is_empty() {
                        pos[i] as f64
                    } else {
                        ns.iter().map(|&p| pos[p] as f64).sum::<f64>() / ns.len() as f64
                    };
                    (i, score)
                })
                .collect();
            scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            layers[r] = scored.into_iter().map(|(i, _)| i).collect();
            for (p, &i) in layers[r].iter().enumerate() {
                pos[i] = p;
            }
        }
    }

    // 4. Coordinate assignment.
    let row_h: Vec<i32> = layers
        .iter()
        .map(|row| row.iter().map(|&i| eff_h(&nodes[i])).max().unwrap_or(0))
        .collect();
    let mut row_y = Vec::with_capacity(num_layers);
    let mut acc = 0;
    for &h in &row_h {
        row_y.push(acc);
        acc += h + V_GAP;
    }

    let has_boundary_nodes = boundary_ids
        .map(|b| layers.iter().flatten().any(|&i| b.contains(&nodes[i].id)))
        .unwrap_or(false);

    let axis_w = if has_boundary_nodes {
        let bids = boundary_ids.unwrap();
        let parts: Vec<(Vec<usize>, Vec<usize>)> = layers
            .iter()
            .map(|row| row.iter().copied().partition(|&i| bids.contains(&nodes[i].id)))
            .collect();
        let b_max = parts.iter().map(|(b, _)| group_w(nodes, b)).max().unwrap_or(0);

        // Place boundary members centred on a common axis; remember the box extent.
        let mut b_right = 0;
        let mut first_b_row = usize::MAX;
        let mut last_b_row = 0;
        for (r, (b, _)) in parts.iter().enumerate() {
            if b.is_empty() {
                continue;
            }
            first_b_row = first_b_row.min(r);
            last_b_row = last_b_row.max(r);
            let bw = group_w(nodes, b);
            let start = (b_max - bw) / 2;
            place_row(nodes, b, start, row_y[r], row_h[r]);
            b_right = b_right.max(start + bw);
        }

        // Non-members: rows that overlap the boundary vertically go to its right;
        // rows fully above/below it stay centred on the same axis.
        for (r, (_, ext)) in parts.iter().enumerate() {
            if ext.is_empty() {
                continue;
            }
            let start = if r >= first_b_row && r <= last_b_row {
                b_right + 2 * BOUNDARY_PAD + H_GAP
            } else {
                (b_max - group_w(nodes, ext)) / 2
            };
            place_row(nodes, ext, start, row_y[r], row_h[r]);
        }
        b_max
    } else {
        let max_w = layers.iter().map(|row| group_w(nodes, row)).max().unwrap_or(0);
        for (r, row) in layers.iter().enumerate() {
            let lw = group_w(nodes, row);
            place_row(nodes, row, (max_w - lw) / 2, row_y[r], row_h[r]);
        }
        max_w
    };

    // Isolated nodes: grid rows below the layered part, centred on the same
    // axis.  Boundary members come first so that externals end up below the
    // boundary box instead of inside it.
    let isolated: Vec<usize> = (0..n).filter(|&i| !connected[i]).collect();
    let (iso_boundary, iso_external): (Vec<usize>, Vec<usize>) = match boundary_ids {
        Some(bids) => isolated.iter().copied().partition(|&i| bids.contains(&nodes[i].id)),
        None => (Vec::new(), isolated),
    };
    let mut y = acc;
    for group in [iso_boundary, iso_external] {
        for chunk in group.chunks(GRID_COLS) {
            let ch = chunk.iter().map(|&i| eff_h(&nodes[i])).max().unwrap_or(0);
            let cw = group_w(nodes, chunk);
            place_row(nodes, chunk, (axis_w - cw) / 2, y, ch);
            y += ch + V_GAP;
        }
    }
}

/// Left-to-right, top-to-bottom grid — the fallback when no edges connect
/// any of the diagram's nodes.
fn layout_grid(nodes: &mut [Node], cols: usize) {
    let idxs: Vec<usize> = (0..nodes.len()).collect();
    let mut y = 0;
    for chunk in idxs.chunks(cols.max(1)) {
        let ch = chunk.iter().map(|&i| eff_h(&nodes[i])).max().unwrap_or(0);
        place_row(nodes, chunk, 0, y, ch);
        y += ch + V_GAP;
    }
}

// ── Edge collection ───────────────────────────────────────────────────────────

const COLOR_DELTA_ADDED: &str = "#2e7d32";
const COLOR_DELTA_REMOVED: &str = "#c62828";

/// Delta views (spec §8.3) carry added/removed ids in view properties;
/// style them green (added) and red + dashed (removed).
fn apply_delta_styling(
    properties: Option<&HashMap<String, String>>,
    nodes: &mut [Node],
    edges: &mut [Edge],
) {
    let Some(props) = properties else { return };
    let ids = |key: &str| -> HashSet<String> {
        props
            .get(key)
            .map(|v| v.split(',').filter(|s| !s.is_empty()).map(str::to_string).collect())
            .unwrap_or_default()
    };
    let added_e = ids("delta.addedElements");
    let removed_e = ids("delta.removedElements");
    let added_r = ids("delta.addedRelationships");
    let removed_r = ids("delta.removedRelationships");
    if added_e.is_empty() && removed_e.is_empty() && added_r.is_empty() && removed_r.is_empty() {
        return;
    }
    for n in nodes.iter_mut() {
        if added_e.contains(&n.id) {
            n.stroke = COLOR_DELTA_ADDED.to_string();
        } else if removed_e.contains(&n.id) {
            n.stroke = COLOR_DELTA_REMOVED.to_string();
            n.dash = Some("4,4");
        }
    }
    for e in edges.iter_mut() {
        if added_r.contains(&e.rel_id) {
            e.color = Some(COLOR_DELTA_ADDED.to_string());
        } else if removed_r.contains(&e.rel_id) {
            e.color = Some(COLOR_DELTA_REMOVED.to_string());
            e.dash = Some("4,4");
        }
    }
}

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
    annotate_edge_ports(&mut edges, model);
    edges
}

/// Resolve edge port ids to port names for glyph rendering (spec §5.1).
fn annotate_edge_ports(edges: &mut [Edge], model: &Model) {
    let mut names: HashMap<(String, String), String> = HashMap::new();
    let mut record = |elem_id: &str, ports: &Option<Vec<structurizr_model::Port>>| {
        for p in ports.iter().flatten() {
            names.insert((elem_id.to_string(), p.id.clone()), p.name.clone());
        }
    };
    for p in model.people.iter().flatten() {
        record(&p.id, &p.ports);
    }
    for ss in model.software_systems.iter().flatten() {
        record(&ss.id, &ss.ports);
        for c in ss.containers.iter().flatten() {
            record(&c.id, &c.ports);
            for comp in c.components.iter().flatten() {
                record(&comp.id, &comp.ports);
            }
        }
    }
    for ce in model.custom_elements.iter().flatten() {
        record(&ce.id, &ce.ports);
    }
    for e in edges.iter_mut() {
        if let Some(pid) = &e.src_port_id {
            e.src_port_name = names.get(&(e.src_id.clone(), pid.clone())).cloned();
        }
        if let Some(pid) = &e.dst_port_id {
            e.dst_port_name = names.get(&(e.dst_id.clone(), pid.clone())).cloned();
        }
    }
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
    annotate_edge_ports(&mut edges, model);
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
                rel_id: r.id.clone(),
                src_id: r.source_id.clone(),
                dst_id: r.destination_id.clone(),
                label: r.description.clone().unwrap_or_default(),
                technology: r.technology.clone().unwrap_or_default(),
                dash: edge_dash(r),
                src_port_id: r.source_port_id.clone(),
                dst_port_id: r.destination_port_id.clone(),
                src_port_name: None,
                dst_port_name: None,
                color: None,
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

fn boundary_rect(nodes: &[&Node], label: &str) -> BoundaryRect {
    let min_x = nodes.iter().map(|n| n.x).min().unwrap_or(0) - BOUNDARY_PAD;
    let min_y = nodes.iter().map(|n| n.y - n.top_overhang()).min().unwrap_or(0)
        - BOUNDARY_PAD
        - BOUNDARY_LABEL_HEIGHT;
    let max_x = nodes.iter().map(|n| n.x + n.w).max().unwrap_or(0) + BOUNDARY_PAD;
    let max_y = nodes.iter().map(|n| n.y + n.h).max().unwrap_or(0) + BOUNDARY_PAD;
    BoundaryRect {
        x: min_x,
        y: min_y,
        w: max_x - min_x,
        h: max_y - min_y,
        label: label.to_string(),
    }
}

// ── SVG renderer ──────────────────────────────────────────────────────────────

fn marker_for(color: &str) -> &'static str {
    match color {
        COLOR_DELTA_ADDED => "arrow-added",
        COLOR_DELTA_REMOVED => "arrow-removed",
        _ => "arrow",
    }
}

fn render_svg(
    title: &str,
    nodes: &[Node],
    edges: &[Edge],
    boundary: Option<&BoundaryRect>,
) -> String {
    let pos: HashMap<&str, &Node> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    // Bounding box over nodes (including person head overhang) and the boundary.
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;
    for n in nodes {
        min_x = min_x.min(n.x);
        min_y = min_y.min(n.y - n.top_overhang());
        max_x = max_x.max(n.x + n.w);
        max_y = max_y.max(n.y + n.h);
    }
    if let Some(b) = boundary {
        min_x = min_x.min(b.x);
        min_y = min_y.min(b.y);
        max_x = max_x.max(b.x + b.w);
        max_y = max_y.max(b.y + b.h);
    }
    if nodes.is_empty() {
        min_x = 0;
        min_y = 0;
        max_x = 200;
        max_y = 100;
    }

    // Shift everything so content starts at (MARGIN, MARGIN) below the title.
    let dx = MARGIN - min_x;
    let dy = MARGIN - min_y;
    let mut width = max_x + dx + MARGIN;
    let height = max_y + dy + MARGIN + TITLE_H;
    width = width.max(text_width(title, FS_TITLE) as i32 + 2 * MARGIN);

    let mut svg = String::new();

    svg.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" font-family="Arial, sans-serif">"##,
    ));
    svg.push('\n');

    // Defs: arrowhead markers (default + delta colours).
    svg.push_str("  <defs>\n");
    for (id, color) in [
        ("arrow", COLOR_ARROW),
        ("arrow-added", COLOR_DELTA_ADDED),
        ("arrow-removed", COLOR_DELTA_REMOVED),
    ] {
        svg.push_str(&format!(
            r##"    <marker id="{id}" markerWidth="10" markerHeight="7" refX="9" refY="3.5" orient="auto"><polygon points="0 0, 10 3.5, 0 7" fill="{color}"/></marker>
"##,
        ));
    }
    svg.push_str("  </defs>\n");

    // Background
    svg.push_str(&format!(
        "  <rect width=\"{width}\" height=\"{height}\" fill=\"{COLOR_BG}\"/>\n",
    ));

    // Title
    svg.push_str(&format!(
        r##"  <text x="{}" y="30" font-size="{FS_TITLE}" font-weight="bold" fill="{COLOR_TITLE}" text-anchor="middle">{}</text>
"##,
        width / 2,
        xml_escape(title)
    ));

    svg.push_str(&format!("  <g transform=\"translate({dx},{})\">\n", dy + TITLE_H));

    // System boundary (if any)
    if let Some(b) = boundary {
        svg.push_str(&format!(
            r##"    <rect x="{}" y="{}" width="{}" height="{}" fill="{COLOR_BOUNDARY_FILL}" stroke="{COLOR_BOUNDARY_STROKE}" stroke-width="1" stroke-dasharray="6,4" rx="6"/>
"##,
            b.x, b.y, b.w, b.h
        ));
        svg.push_str(&format!(
            r##"    <text x="{}" y="{}" font-size="11" fill="#888888" font-style="italic">{}</text>
"##,
            b.x + 10,
            b.y + 16,
            xml_escape(&b.label)
        ));
    }

    // Edges (drawn before nodes so nodes appear on top).
    // Parallel edges between the same pair of nodes are spread perpendicular
    // to the connecting line so they do not overlap.
    let mut pair_count: HashMap<(&str, &str), usize> = HashMap::new();
    for e in edges {
        if pos.contains_key(e.src_id.as_str()) && pos.contains_key(e.dst_id.as_str()) {
            *pair_count.entry(canon_pair(&e.src_id, &e.dst_id)).or_insert(0) += 1;
        }
    }
    let mut pair_used: HashMap<(&str, &str), usize> = HashMap::new();

    // Labels and port glyphs are buffered and emitted after the nodes so a box
    // can never hide them.
    let mut overlay = String::new();

    for edge in edges {
        let src = match pos.get(edge.src_id.as_str()) {
            Some(n) => *n,
            None => continue,
        };
        let dst = match pos.get(edge.dst_id.as_str()) {
            Some(n) => *n,
            None => continue,
        };

        let stroke = edge.color.as_deref().unwrap_or(COLOR_ARROW);
        let marker = marker_for(stroke);
        let dash_attr = edge
            .dash
            .map(|d| format!(r#" stroke-dasharray="{}""#, d))
            .unwrap_or_default();

        // Self-loop: a small arc on the right side of the node.
        if src.id == dst.id {
            let x0 = (src.x + src.w) as f64;
            let cy = src.cy() as f64;
            svg.push_str(&format!(
                r##"    <path d="M {x0} {} C {} {}, {} {}, {x0} {}" fill="none" stroke="{stroke}" stroke-width="1.4"{dash_attr} marker-end="url(#{marker})"/>
"##,
                cy - 14.0,
                x0 + 52.0,
                cy - 20.0,
                x0 + 52.0,
                cy + 20.0,
                cy + 14.0,
            ));
            draw_edge_label(&mut overlay, edge, x0 + 46.0, cy, stroke);
            continue;
        }

        let key = canon_pair(&edge.src_id, &edge.dst_id);
        let count = pair_count.get(&key).copied().unwrap_or(1);
        let slot = pair_used.entry(key).or_insert(0);
        let idx = *slot;
        *slot += 1;

        let mut off = (idx as f64 - (count as f64 - 1.0) / 2.0) * EDGE_SPREAD;
        let lim = (src.w.min(src.h).min(dst.w).min(dst.h) as f64) / 2.0 - 10.0;
        off = off.clamp(-lim.max(0.0), lim.max(0.0));

        // Perpendicular computed from the canonical direction so that an A→B
        // edge and its B→A counterpart land on distinct parallel lines.
        let (a, b) = if src.id.as_str() <= dst.id.as_str() { (src, dst) } else { (dst, src) };
        let vx = (b.cx() - a.cx()) as f64;
        let vy = (b.cy() - a.cy()) as f64;
        let len = (vx * vx + vy * vy).sqrt().max(1.0);
        let (px, py) = (-vy / len, vx / len);

        let scx = src.cx() as f64 + px * off;
        let scy = src.cy() as f64 + py * off;
        let dcx = dst.cx() as f64 + px * off;
        let dcy = dst.cy() as f64 + py * off;

        let (x1, y1) = rect_exit_point(src, scx, scy, dcx, dcy);
        let (x2, y2) = rect_exit_point(dst, dcx, dcy, scx, scy);

        svg.push_str(&format!(
            r##"    <line x1="{x1:.1}" y1="{y1:.1}" x2="{x2:.1}" y2="{y2:.1}" stroke="{stroke}" stroke-width="1.4"{dash_attr} marker-end="url(#{marker})"/>
"##,
        ));

        // Port glyphs: a small square on the element border where the
        // relationship attaches through a declared port (spec §5.1).
        for (gx, gy, pname) in [
            (x1, y1, edge.src_port_name.as_deref()),
            (x2, y2, edge.dst_port_name.as_deref()),
        ] {
            if let Some(pname) = pname {
                overlay.push_str(&format!(
                    r##"    <rect x="{:.1}" y="{:.1}" width="8" height="8" fill="#ffffff" stroke="{stroke}" stroke-width="1.2"/>
"##,
                    gx - 4.0,
                    gy - 4.0,
                ));
                overlay.push_str(&format!(
                    r##"    <text x="{:.1}" y="{:.1}" font-size="8" fill="{stroke}" text-anchor="middle">{}</text>
"##,
                    gx,
                    gy - 7.0,
                    xml_escape(pname)
                ));
            }
        }

        // Edge label at the midpoint, nudged along the same perpendicular so
        // parallel edges keep their labels apart.
        let extra = if count > 1 { off.signum() * 10.0 } else { 0.0 };
        let lx = (x1 + x2) / 2.0 + px * extra;
        let ly = (y1 + y2) / 2.0 + py * extra;
        draw_edge_label(&mut overlay, edge, lx, ly, stroke);
    }

    // Nodes
    for node in nodes {
        render_node(&mut svg, node);
    }

    svg.push_str(&overlay);
    svg.push_str("  </g>\n");
    svg.push_str("</svg>\n");
    svg
}

fn canon_pair<'a>(a: &'a str, b: &'a str) -> (&'a str, &'a str) {
    if a <= b { (a, b) } else { (b, a) }
}

/// Draw a wrapped edge label centred at (lx, ly) with a background halo so it
/// stays readable where it crosses lines.
fn draw_edge_label(svg: &mut String, edge: &Edge, lx: f64, ly: f64, color: &str) {
    if edge.label.is_empty() && edge.technology.is_empty() {
        return;
    }
    let label_text = if edge.technology.is_empty() {
        edge.label.clone()
    } else if edge.label.is_empty() {
        format!("[{}]", edge.technology)
    } else {
        format!("{} [{}]", edge.label, edge.technology)
    };
    let lines = clamp_lines(wrap_text(&label_text, EDGE_LABEL_W, FS_EDGE), 3);
    let line_h = 13.0;
    let first_baseline = ly - (lines.len() as f64 - 1.0) * line_h / 2.0 + 3.5;
    for (i, line) in lines.iter().enumerate() {
        let baseline = first_baseline + i as f64 * line_h;
        let bw = text_width(line, FS_EDGE) + 8.0;
        svg.push_str(&format!(
            r##"    <rect x="{:.1}" y="{:.1}" width="{:.1}" height="{line_h}" fill="{COLOR_BG}" opacity="0.88"/>
"##,
            lx - bw / 2.0,
            baseline - 10.0,
            bw,
        ));
        svg.push_str(&format!(
            r##"    <text x="{lx:.1}" y="{baseline:.1}" font-size="{FS_EDGE}" fill="{color}" text-anchor="middle">{}</text>
"##,
            xml_escape(line)
        ));
    }
}

/// Render a single node box (person or system/container/component).
fn render_node(svg: &mut String, node: &Node) {
    let dash_attr = node
        .dash
        .map(|d| format!(r#" stroke-dasharray="{}""#, d))
        .unwrap_or_default();

    if node.is_person {
        // Head circle drawn first so the box covers its lower part — the
        // classic C4 person glyph.  Its centre sits PERSON_HEAD_OVERLAP above
        // the box top edge.
        svg.push_str(&format!(
            r##"    <circle cx="{}" cy="{}" r="{}" fill="{}" stroke="{}" stroke-width="1.5"{dash_attr}/>
"##,
            node.cx(),
            node.y - PERSON_HEAD_OVERLAP,
            PERSON_HEAD_RADIUS,
            node.fill,
            node.stroke,
        ));
    }

    svg.push_str(&format!(
        r##"    <rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" stroke-width="1.5"{dash_attr} rx="{}"/>
"##,
        node.x,
        node.y,
        node.w,
        node.h,
        node.fill,
        node.stroke,
        if node.is_person { 12 } else { 4 },
    ));

    // Text block, vertically centred inside the box.
    let cx = node.cx();
    let mut cursor = node.y + (node.h - node.content_h) / 2;
    for line in &node.name_lines {
        svg.push_str(&format!(
            r##"    <text x="{cx}" y="{}" font-size="{FS_NAME}" font-weight="bold" fill="{}" text-anchor="middle">{}</text>
"##,
            cursor + 13,
            node.text_color,
            xml_escape(line)
        ));
        cursor += LH_NAME;
    }
    for line in &node.meta_lines {
        svg.push_str(&format!(
            r##"    <text x="{cx}" y="{}" font-size="{FS_META}" font-style="italic" fill="{}" opacity="0.85" text-anchor="middle">{}</text>
"##,
            cursor + 10,
            node.text_color,
            xml_escape(line)
        ));
        cursor += LH_SMALL;
    }
    if !node.desc_lines.is_empty() {
        cursor += 6;
        for line in &node.desc_lines {
            svg.push_str(&format!(
                r##"    <text x="{cx}" y="{}" font-size="{FS_DESC}" fill="{}" opacity="0.9" text-anchor="middle">{}</text>
"##,
                cursor + 10,
                node.text_color,
                xml_escape(line)
            ));
            cursor += LH_SMALL;
        }
    }
}

// ── Geometry helpers ──────────────────────────────────────────────────────────

/// Point where the ray from (px, py) toward (tx, ty) exits `node`'s box.
/// (px, py) is expected to be inside the box; if the ray never exits (degenerate
/// direction), (px, py) itself is returned.
fn rect_exit_point(node: &Node, px: f64, py: f64, tx: f64, ty: f64) -> (f64, f64) {
    let dx = tx - px;
    let dy = ty - py;
    if dx == 0.0 && dy == 0.0 {
        return (px, py);
    }
    let x0 = node.x as f64;
    let y0 = node.y as f64;
    let x1 = (node.x + node.w) as f64;
    let y1 = (node.y + node.h) as f64;
    let mut t = f64::INFINITY;
    if dx > 0.0 {
        t = t.min((x1 - px) / dx);
    } else if dx < 0.0 {
        t = t.min((x0 - px) / dx);
    }
    if dy > 0.0 {
        t = t.min((y1 - py) / dy);
    } else if dy < 0.0 {
        t = t.min((y0 - py) / dy);
    }
    if !t.is_finite() || t < 0.0 {
        return (px, py);
    }
    (px + dx * t, py + dy * t)
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
        // Relate Alice to the system so context scoping includes her.
        workspace.model.people.as_mut().unwrap()[0].relationships = Some(vec![Relationship {
            id: "r1".to_string(),
            source_id: "1".to_string(),
            destination_id: "2".to_string(),
            description: Some("Uses".to_string()),
            ..Default::default()
        }]);
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
        // Focal system should use the primary blue.
        assert!(svg.contains(COLOR_SYSTEM));
    }

    #[test]
    fn system_context_scopes_to_related_elements() {
        // An unrelated system must not appear in the context view.
        let mut workspace = basic_workspace();
        workspace.model.people.as_mut().unwrap()[0].relationships = Some(vec![Relationship {
            id: "r1".to_string(),
            source_id: "1".to_string(),
            destination_id: "2".to_string(),
            ..Default::default()
        }]);
        workspace.model.software_systems.as_mut().unwrap().push(SoftwareSystem {
            id: "99".to_string(),
            name: "Unrelated".to_string(),
            ..Default::default()
        });
        workspace.views.system_context_views = Some(vec![SystemContextView {
            software_system_id: "2".to_string(),
            key: Some("Context".to_string()),
            ..Default::default()
        }]);

        let diagrams = SvgExporter.export_workspace(&workspace);
        let svg = &diagrams[0].content;
        assert!(svg.contains("Alice"));
        assert!(!svg.contains("Unrelated"), "unrelated system must be scoped out");
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
        assert!(svg.contains("My System"), "boundary label should carry the system name");
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
        assert!(svg.contains("url(#arrow)"), "arrowhead marker should be present");
    }

    #[test]
    fn bidirectional_relationships_do_not_explode_layout() {
        // A ↔ B used to escalate longest-path layering until every node sat in
        // a distant layer, producing a huge, mostly blank canvas.
        let mut workspace = Workspace::default();
        workspace.name = "CycleTest".to_string();
        workspace.model.software_systems = Some(vec![
            SoftwareSystem {
                id: "1".to_string(),
                name: "Alpha".to_string(),
                relationships: Some(vec![Relationship {
                    id: "r1".to_string(),
                    source_id: "1".to_string(),
                    destination_id: "2".to_string(),
                    ..Default::default()
                }]),
                ..Default::default()
            },
            SoftwareSystem {
                id: "2".to_string(),
                name: "Beta".to_string(),
                relationships: Some(vec![Relationship {
                    id: "r2".to_string(),
                    source_id: "2".to_string(),
                    destination_id: "1".to_string(),
                    ..Default::default()
                }]),
                ..Default::default()
            },
        ]);
        workspace.views.system_landscape_views = Some(vec![SystemLandscapeView {
            key: Some("Landscape".to_string()),
            ..Default::default()
        }]);

        let diagrams = SvgExporter.export_workspace(&workspace);
        let svg = &diagrams[0].content;
        let height: i32 = svg
            .split("height=\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .and_then(|s| s.parse().ok())
            .unwrap();
        // Two layers of ~90px nodes plus gaps/margins/title — far below 600.
        assert!(height < 600, "cycle must not inflate the canvas (height={height})");
    }

    #[test]
    fn xml_escape_chars() {
        assert_eq!(xml_escape("a&b<c>d\"e'f"), "a&amp;b&lt;c&gt;d&quot;e&#39;f");
    }

    #[test]
    fn rect_exit_point_horizontal() {
        let node = Node {
            id: "n".to_string(),
            fill: String::new(),
            stroke: String::new(),
            text_color: String::new(),
            is_person: false,
            x: 0,
            y: 60,
            w: 200,
            h: 80,
            content_h: 0,
            name_lines: vec![],
            meta_lines: vec![],
            desc_lines: vec![],
            dash: None,
        };
        // Ray from the centre straight right exits at the right edge.
        let (ex, ey) = rect_exit_point(&node, 100.0, 100.0, 500.0, 100.0);
        assert_eq!(ex, 200.0);
        assert_eq!(ey, 100.0);
        // Straight down exits at the bottom edge.
        let (ex, ey) = rect_exit_point(&node, 100.0, 100.0, 100.0, 500.0);
        assert_eq!(ex, 100.0);
        assert_eq!(ey, 140.0);
    }

    #[test]
    fn wrap_text_wraps_and_clamps() {
        let lines = wrap_text("Personal Banking Customer of the bank", 100.0, 14.0);
        assert!(lines.len() > 1, "long text must wrap");
        for l in &lines {
            assert!(text_width(l, 14.0) <= 100.0 + 1e-6, "line too wide: {l}");
        }
        let clamped = clamp_lines(lines, 2);
        assert_eq!(clamped.len(), 2);
        assert!(clamped[1].ends_with('…'));
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
    fn stored_positions_preserved_relatively() {
        // Two nodes with stored x/y 300px apart must stay 300px apart in the
        // output (the canvas is normalised, so only relative offsets survive).
        let mut workspace = Workspace::default();
        workspace.name = "PosTest".to_string();
        workspace.model.software_systems = Some(vec![
            SoftwareSystem { id: "1".to_string(), name: "SysA".to_string(), ..Default::default() },
            SoftwareSystem { id: "2".to_string(), name: "SysB".to_string(), ..Default::default() },
        ]);
        workspace.views.system_landscape_views = Some(vec![SystemLandscapeView {
            key: Some("Landscape".to_string()),
            element_views: Some(vec![
                ElementView { id: "1".to_string(), x: Some(100), y: Some(100) },
                ElementView { id: "2".to_string(), x: Some(400), y: Some(100) },
            ]),
            ..Default::default()
        }]);

        let exporter = SvgExporter;
        let diagrams = exporter.export_workspace(&workspace);
        let svg = &diagrams[0].content;

        // Stored coordinates are kept on the boxes; the outer <g> transform
        // normalises the canvas.  The 300px horizontal offset must survive.
        assert!(svg.contains(r#"x="100""#), "first box keeps stored x=100");
        assert!(svg.contains(r#"x="400""#), "second box keeps stored x=400");
    }

    #[test]
    fn person_head_circle_above_box() {
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
        // Extract the head circle centre and the person box top edge and check
        // the head sits above the box.
        let cy: f64 = svg
            .split("cy=\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .and_then(|s| s.parse().ok())
            .expect("circle cy");
        let circle_pos = svg.find("<circle").unwrap();
        let rect_after = &svg[circle_pos..];
        let y: f64 = rect_after
            .split("<rect")
            .nth(1)
            .and_then(|s| s.split("y=\"").nth(1))
            .and_then(|s| s.split('"').next())
            .and_then(|s| s.parse().ok())
            .expect("person rect y");
        assert!(cy < y, "head circle centre (cy={cy}) must be above the box top (y={y})");
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
