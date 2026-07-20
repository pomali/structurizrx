//! View generation (spec §6.3): materialize `AutoViewSpec`s — and the
//! zero-config default zoom set — into concrete views appended to the
//! workspace's `ViewSet`.
//!
//! Generated views (except the default zoom set, which uses the proper typed
//! views) are `SystemLandscapeView`s with deterministic keys, populated with
//! `ElementView`/`RelationshipView` entries. Rendering styling (delta colours,
//! port glyphs) is a later phase; this module decides *what* is in each view.

use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};

use structurizr_model::{
    AutoViewSpec, ComponentView, ContainerView, ElementView, RelationshipView,
    SystemContextView, SystemLandscapeView, Workspace,
};

use crate::eval::{build_index, Index};
use crate::{query, QueryError};

/// Materialize auto-view specs (`workspace.views.auto_views`) into concrete
/// views appended to the ViewSet. If the workspace has no concrete views and
/// no auto specs, the zero-config default set (spec §6.1) is generated.
/// Idempotent: a spec whose key already exists is skipped. Returns the keys
/// of newly generated views.
pub fn generate_views(workspace: &mut Workspace) -> Result<Vec<String>, QueryError> {
    let specs: Vec<AutoViewSpec> = match &workspace.views.auto_views {
        Some(s) if !s.is_empty() => s.clone(),
        _ => {
            if has_any_concrete_view(workspace) {
                return Ok(vec![]);
            }
            vec![AutoViewSpec { generator: "default".to_string(), ..Default::default() }]
        }
    };

    let idx = build_index(workspace);
    let mut generated = Vec::new();

    for spec in &specs {
        match spec.generator.as_str() {
            "default" => gen_default(workspace, &idx, &mut generated),
            "focus" => gen_focus(workspace, &idx, spec, &mut generated)?,
            "perspective" => gen_perspective(workspace, &idx, spec, &mut generated),
            "layer" => gen_layer(workspace, &idx, spec, &mut generated),
            "slice" => gen_slice(workspace, spec, &mut generated)?,
            "paths" => gen_paths(workspace, &idx, spec, &mut generated)?,
            "asof" => gen_asof(workspace, &idx, spec, &mut generated),
            "delta" => gen_delta(workspace, &idx, spec, &mut generated),
            "lint" => gen_lint(workspace, &idx, &mut generated),
            "rollup" => {
                eprintln!(
                    "note: `auto rollup` is not materialized yet (skipped {})",
                    key_for("rollup", spec.target.as_deref(), None, None)
                );
            }
            other => {
                eprintln!("note: unknown auto view generator '{}' skipped", other);
            }
        }
    }

    Ok(generated)
}

// ---------------------------------------------------------------------------
// Key + view plumbing
// ---------------------------------------------------------------------------

fn sanitize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut dash = false;
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    out.trim_end_matches('-').to_string()
}

fn key_for(generator: &str, a: Option<&str>, b: Option<&str>, split: Option<&str>) -> String {
    let mut key = format!("auto-{}", sanitize(generator));
    for part in [a, b, split].into_iter().flatten() {
        let s = sanitize(part);
        if !s.is_empty() {
            key.push('-');
            key.push_str(&s);
        }
    }
    key
}

fn all_view_keys(ws: &Workspace) -> HashSet<String> {
    let mut keys = HashSet::new();
    macro_rules! collect {
        ($views:expr) => {
            for v in $views.iter().flatten() {
                if let Some(k) = &v.key {
                    keys.insert(k.clone());
                }
            }
        };
    }
    collect!(ws.views.system_landscape_views);
    collect!(ws.views.system_context_views);
    collect!(ws.views.container_views);
    collect!(ws.views.component_views);
    keys
}

fn has_any_concrete_view(ws: &Workspace) -> bool {
    let v = &ws.views;
    [
        v.system_landscape_views.as_ref().map_or(0, |x| x.len()),
        v.system_context_views.as_ref().map_or(0, |x| x.len()),
        v.container_views.as_ref().map_or(0, |x| x.len()),
        v.component_views.as_ref().map_or(0, |x| x.len()),
        v.dynamic_views.as_ref().map_or(0, |x| x.len()),
        v.deployment_views.as_ref().map_or(0, |x| x.len()),
        v.custom_views.as_ref().map_or(0, |x| x.len()),
    ]
    .iter()
    .sum::<usize>()
        > 0
}

fn element_views(ids: &BTreeSet<String>) -> Option<Vec<ElementView>> {
    if ids.is_empty() {
        return None;
    }
    Some(ids.iter().map(|id| ElementView { id: id.clone(), ..Default::default() }).collect())
}

fn relationship_views(ids: &BTreeSet<String>) -> Option<Vec<RelationshipView>> {
    if ids.is_empty() {
        return None;
    }
    Some(ids.iter().map(|id| RelationshipView { id: id.clone(), ..Default::default() }).collect())
}

/// Relationships whose endpoints are both in `elems` (the induced-subgraph rule, §6.1).
fn induced_rels(idx: &Index, elems: &BTreeSet<String>) -> BTreeSet<String> {
    idx.relationships
        .iter()
        .filter(|r| elems.contains(&r.source_id) && elems.contains(&r.dest_id))
        .map(|r| r.id.clone())
        .collect()
}

/// Push a generated landscape-shaped view unless its key already exists.
#[allow(clippy::too_many_arguments)]
fn push_generated(
    ws: &mut Workspace,
    generated: &mut Vec<String>,
    key: String,
    title: String,
    description: Option<String>,
    elems: &BTreeSet<String>,
    rels: &BTreeSet<String>,
) {
    if all_view_keys(ws).contains(&key) {
        return;
    }
    let view = SystemLandscapeView {
        key: Some(key.clone()),
        title: Some(title),
        description,
        element_views: element_views(elems),
        relationship_views: relationship_views(rels),
        ..Default::default()
    };
    ws.views.system_landscape_views.get_or_insert_with(Vec::new).push(view);
    generated.push(key);
}

/// Resolve a target reference by element id, then name (case-insensitive).
fn resolve_target(idx: &Index, target: &str) -> Result<usize, QueryError> {
    if let Some(i) = idx.by_id.get(target) {
        return Ok(*i);
    }
    if let Some(i) = idx.by_name.get(&target.to_lowercase()) {
        return Ok(*i);
    }
    Err(QueryError::UnknownTarget(target.to_string()))
}

// ---------------------------------------------------------------------------
// Milestones (spec §8)
// ---------------------------------------------------------------------------

/// Milestone order: implicit "now" precedes all declared milestones.
fn milestone_order(ws: &Workspace) -> Vec<String> {
    let mut order = vec!["now".to_string()];
    for m in ws.milestones.iter().flatten() {
        order.push(m.name.clone());
    }
    order
}

fn ms_pos(order: &[String], name: &str) -> Option<usize> {
    order.iter().position(|m| m.eq_ignore_ascii_case(name))
}

/// An item exists at milestone `at` iff introduced ≤ at < retired.
/// Unknown milestone names are treated as "always" (validation flags them).
fn exists_at(
    order: &[String],
    at: usize,
    introduced: &Option<String>,
    retired: &Option<String>,
) -> bool {
    if let Some(i) = introduced {
        if let Some(pos) = ms_pos(order, i) {
            if at < pos {
                return false;
            }
        }
    }
    if let Some(r) = retired {
        if let Some(pos) = ms_pos(order, r) {
            if at >= pos {
                return false;
            }
        }
    }
    true
}

/// Element + relationship id sets existing at a milestone position.
fn asof_sets(idx: &Index, order: &[String], at: usize) -> (BTreeSet<String>, BTreeSet<String>) {
    let elems: BTreeSet<String> = idx
        .elements
        .iter()
        .filter(|e| exists_at(order, at, &e.introduced, &e.retired))
        .map(|e| e.id.clone())
        .collect();
    let rels: BTreeSet<String> = idx
        .relationships
        .iter()
        .filter(|r| {
            exists_at(order, at, &r.introduced, &r.retired)
                && elems.contains(&r.source_id)
                && elems.contains(&r.dest_id)
        })
        .map(|r| r.id.clone())
        .collect();
    (elems, rels)
}

// ---------------------------------------------------------------------------
// default — the zero-config zoom set (spec §6.1)
// ---------------------------------------------------------------------------

fn gen_default(ws: &mut Workspace, idx: &Index, generated: &mut Vec<String>) {
    let existing = all_view_keys(ws);

    // Landscape: all people + software systems.
    let top: BTreeSet<String> = idx
        .elements
        .iter()
        .filter(|e| e.kind == "person" || e.kind == "softwareSystem")
        .map(|e| e.id.clone())
        .collect();
    let key = "auto-landscape".to_string();
    if !existing.contains(&key) && !top.is_empty() {
        let rels = induced_rels(idx, &top);
        let view = SystemLandscapeView {
            key: Some(key.clone()),
            title: Some(format!("{} — landscape", ws.name)),
            element_views: element_views(&top),
            relationship_views: relationship_views(&rels),
            ..Default::default()
        };
        ws.views.system_landscape_views.get_or_insert_with(Vec::new).push(view);
        generated.push(key);
    }

    // Direct-neighbor helper over the index.
    let neighbors = |id: &str| -> BTreeSet<String> {
        idx.relationships
            .iter()
            .filter_map(|r| {
                if r.source_id == id {
                    Some(r.dest_id.clone())
                } else if r.dest_id == id {
                    Some(r.source_id.clone())
                } else {
                    None
                }
            })
            .collect()
    };

    let systems: Vec<(String, String)> = idx
        .elements
        .iter()
        .filter(|e| e.kind == "softwareSystem")
        .map(|e| (e.id.clone(), e.name.clone()))
        .collect();

    // Context view per software system.
    for (sys_id, sys_name) in &systems {
        let key = format!("auto-context-{}", sanitize(sys_name));
        if existing.contains(&key) {
            continue;
        }
        let mut elems: BTreeSet<String> = neighbors(sys_id)
            .into_iter()
            .filter(|n| {
                idx.by_id.get(n).is_some_and(|i| {
                    matches!(idx.elements[*i].kind, "person" | "softwareSystem")
                })
            })
            .collect();
        elems.insert(sys_id.clone());
        let rels = induced_rels(idx, &elems);
        let view = SystemContextView {
            software_system_id: sys_id.clone(),
            key: Some(key.clone()),
            title: Some(format!("{} — context", sys_name)),
            element_views: element_views(&elems),
            relationship_views: relationship_views(&rels),
            ..Default::default()
        };
        ws.views.system_context_views.get_or_insert_with(Vec::new).push(view);
        generated.push(key);
    }

    // Container view per system that has containers.
    for (sys_id, sys_name) in &systems {
        let containers: BTreeSet<String> = idx
            .elements
            .iter()
            .filter(|e| e.kind == "container" && e.parent_id.as_deref() == Some(sys_id))
            .map(|e| e.id.clone())
            .collect();
        if containers.is_empty() {
            continue;
        }
        let key = format!("auto-container-{}", sanitize(sys_name));
        if existing.contains(&key) {
            continue;
        }
        let mut elems = containers.clone();
        for c in &containers {
            for n in neighbors(c) {
                // external context: anything directly connected, except the parent system
                if n != *sys_id {
                    elems.insert(n);
                }
            }
        }
        let rels = induced_rels(idx, &elems);
        let view = ContainerView {
            software_system_id: sys_id.clone(),
            key: Some(key.clone()),
            title: Some(format!("{} — containers", sys_name)),
            element_views: element_views(&elems),
            relationship_views: relationship_views(&rels),
            ..Default::default()
        };
        ws.views.container_views.get_or_insert_with(Vec::new).push(view);
        generated.push(key);
    }

    // Component view per container that has components.
    let containers: Vec<(String, String)> = idx
        .elements
        .iter()
        .filter(|e| e.kind == "container")
        .map(|e| (e.id.clone(), e.name.clone()))
        .collect();
    for (cont_id, cont_name) in &containers {
        let components: BTreeSet<String> = idx
            .elements
            .iter()
            .filter(|e| e.kind == "component" && e.parent_id.as_deref() == Some(cont_id))
            .map(|e| e.id.clone())
            .collect();
        if components.is_empty() {
            continue;
        }
        let key = format!("auto-component-{}", sanitize(cont_name));
        if existing.contains(&key) {
            continue;
        }
        let mut elems = components.clone();
        for c in &components {
            for n in neighbors(c) {
                if n != *cont_id {
                    elems.insert(n);
                }
            }
        }
        let rels = induced_rels(idx, &elems);
        let view = ComponentView {
            container_id: cont_id.clone(),
            key: Some(key.clone()),
            title: Some(format!("{} — components", cont_name)),
            element_views: element_views(&elems),
            relationship_views: relationship_views(&rels),
            ..Default::default()
        };
        ws.views.component_views.get_or_insert_with(Vec::new).push(view);
        generated.push(key);
    }
}

// ---------------------------------------------------------------------------
// focus (spec §6.3: blast radius / dependencies)
// ---------------------------------------------------------------------------

fn gen_focus(
    ws: &mut Workspace,
    idx: &Index,
    spec: &AutoViewSpec,
    generated: &mut Vec<String>,
) -> Result<(), QueryError> {
    let target = spec.target.as_deref().unwrap_or_default();
    let ti = resolve_target(idx, target)?;
    let target_id = idx.elements[ti].id.clone();
    let target_name = idx.elements[ti].name.clone();
    let depth = spec.depth.unwrap_or(1);
    let direction = spec.direction.as_deref().unwrap_or("both");

    // Optional asof filter on the traversal graph.
    let order = milestone_order(ws);
    let at = spec.asof.as_deref().and_then(|m| ms_pos(&order, m));
    let alive_rel = |r: &crate::eval::RelEntry| match at {
        Some(pos) => exists_at(&order, pos, &r.introduced, &r.retired),
        None => true,
    };

    // BFS.
    let mut visited: BTreeSet<String> = BTreeSet::new();
    let mut rels: BTreeSet<String> = BTreeSet::new();
    let mut frontier = VecDeque::new();
    visited.insert(target_id.clone());
    frontier.push_back((target_id.clone(), 0u32));
    while let Some((id, d)) = frontier.pop_front() {
        if d >= depth {
            continue;
        }
        for r in &idx.relationships {
            if !alive_rel(r) {
                continue;
            }
            let next = if r.source_id == id && direction != "in" {
                Some(&r.dest_id)
            } else if r.dest_id == id && direction != "out" {
                Some(&r.source_id)
            } else {
                None
            };
            if let Some(next) = next {
                rels.insert(r.id.clone());
                if visited.insert(next.clone()) {
                    frontier.push_back((next.clone(), d + 1));
                }
            }
        }
    }

    match spec.split_by.as_deref() {
        None => {
            let key = key_for("focus", Some(&target_name), None, None);
            let title = format!("focus: {}", target_name);
            push_generated(ws, generated, key, title, None, &visited, &rels);
        }
        Some(split) => {
            // Partition the collected relationships into buckets.
            let mut buckets: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
            for r in &idx.relationships {
                if !rels.contains(&r.id) {
                    continue;
                }
                let values: Vec<String> = match split {
                    "kind" => vec![r.kind.clone().unwrap_or_else(|| "unspecified".to_string())],
                    "tag" => {
                        let t: Vec<String> = r
                            .tags
                            .iter()
                            .filter(|t| *t != "Relationship")
                            .cloned()
                            .collect();
                        if t.is_empty() { vec!["untagged".to_string()] } else { t }
                    }
                    _ /* layer */ => {
                        // layer of the far endpoint (the one that isn't the focus target)
                        let far = if r.source_id == target_id { &r.dest_id } else { &r.source_id };
                        let layer = idx.by_id.get(far).and_then(|i| {
                            let e = &idx.elements[*i];
                            e.properties.get("layer").cloned().or_else(|| e.group.clone())
                        });
                        vec![layer.unwrap_or_else(|| "unlayered".to_string())]
                    }
                };
                for v in values {
                    buckets.entry(v).or_default().insert(r.id.clone());
                }
            }
            for (value, bucket_rels) in buckets {
                let mut elems: BTreeSet<String> = BTreeSet::new();
                elems.insert(target_id.clone());
                for r in &idx.relationships {
                    if bucket_rels.contains(&r.id) {
                        elems.insert(r.source_id.clone());
                        elems.insert(r.dest_id.clone());
                    }
                }
                let key = key_for("focus", Some(&target_name), None, Some(&value));
                let title = format!("focus: {} ({})", target_name, value);
                push_generated(ws, generated, key, title, None, &elems, &bucket_rels);
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// perspective / layer / slice
// ---------------------------------------------------------------------------

fn perspective_sets(idx: &Index, name: &str) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut elems: BTreeSet<String> = idx
        .elements
        .iter()
        .filter(|e| e.perspectives.iter().any(|p| p.eq_ignore_ascii_case(name)))
        .map(|e| e.id.clone())
        .collect();
    let mut rels: BTreeSet<String> = BTreeSet::new();
    for r in &idx.relationships {
        if r.perspectives.iter().any(|p| p.eq_ignore_ascii_case(name)) {
            rels.insert(r.id.clone());
            elems.insert(r.source_id.clone());
            elems.insert(r.dest_id.clone());
        }
    }
    rels.extend(induced_rels(idx, &elems));
    (elems, rels)
}

fn gen_perspective(ws: &mut Workspace, idx: &Index, spec: &AutoViewSpec, generated: &mut Vec<String>) {
    let target = spec.target.as_deref().unwrap_or("*");
    let names: BTreeSet<String> = if target == "*" {
        let mut n: BTreeSet<String> = ws
            .perspectives
            .iter()
            .flatten()
            .map(|p| p.name.clone())
            .collect();
        for e in &idx.elements {
            n.extend(e.perspectives.iter().cloned());
        }
        for r in &idx.relationships {
            n.extend(r.perspectives.iter().cloned());
        }
        n
    } else {
        BTreeSet::from([target.to_string()])
    };

    for name in names {
        let (elems, rels) = perspective_sets(idx, &name);
        let key = key_for("perspective", Some(&name), None, None);
        let title = format!("perspective: {}", name);
        push_generated(ws, generated, key, title, None, &elems, &rels);
    }
}

fn gen_layer(ws: &mut Workspace, idx: &Index, spec: &AutoViewSpec, generated: &mut Vec<String>) {
    let target = spec.target.as_deref().unwrap_or_default();
    let elems: BTreeSet<String> = idx
        .elements
        .iter()
        .filter(|e| {
            e.properties
                .get("layer")
                .map(|l| l.eq_ignore_ascii_case(target))
                .unwrap_or(false)
                || e.group.as_deref().map(|g| g.eq_ignore_ascii_case(target)).unwrap_or(false)
        })
        .map(|e| e.id.clone())
        .collect();
    let rels = induced_rels(idx, &elems);
    let key = key_for("layer", Some(target), None, None);
    let title = format!("layer: {}", target);
    push_generated(ws, generated, key, title, None, &elems, &rels);
}

fn gen_slice(ws: &mut Workspace, spec: &AutoViewSpec, generated: &mut Vec<String>) -> Result<(), QueryError> {
    let expr = spec.expression.as_deref().unwrap_or("*");
    let selection = query(expr, ws)?;
    let idx = build_index(ws);
    let mut elems: BTreeSet<String> = selection.elements.clone();
    let mut rels: BTreeSet<String> = selection.relationships.clone();
    for r in &idx.relationships {
        if selection.relationships.contains(&r.id) {
            elems.insert(r.source_id.clone());
            elems.insert(r.dest_id.clone());
        }
    }
    rels.extend(induced_rels(&idx, &elems));
    let key = key_for("slice", Some(expr), None, None);
    let title = format!("slice: {}", expr);
    push_generated(ws, generated, key, title, None, &elems, &rels);
    Ok(())
}

// ---------------------------------------------------------------------------
// paths — everything on any a→b walk (reachability intersection, no enumeration)
// ---------------------------------------------------------------------------

fn gen_paths(
    ws: &mut Workspace,
    idx: &Index,
    spec: &AutoViewSpec,
    generated: &mut Vec<String>,
) -> Result<(), QueryError> {
    let a = resolve_target(idx, spec.target.as_deref().unwrap_or_default())?;
    let b = resolve_target(idx, spec.target2.as_deref().unwrap_or_default())?;
    let a_id = idx.elements[a].id.clone();
    let b_id = idx.elements[b].id.clone();

    let reach = |start: &str, forward: bool| -> BTreeSet<String> {
        let mut seen: BTreeSet<String> = BTreeSet::from([start.to_string()]);
        let mut stack = vec![start.to_string()];
        while let Some(id) = stack.pop() {
            for r in &idx.relationships {
                let next = if forward && r.source_id == id {
                    Some(&r.dest_id)
                } else if !forward && r.dest_id == id {
                    Some(&r.source_id)
                } else {
                    None
                };
                if let Some(n) = next {
                    if seen.insert(n.clone()) {
                        stack.push(n.clone());
                    }
                }
            }
        }
        seen
    };

    let fwd = reach(&a_id, true);
    let bwd = reach(&b_id, false);
    let elems: BTreeSet<String> = fwd.intersection(&bwd).cloned().collect();
    let rels: BTreeSet<String> = idx
        .relationships
        .iter()
        .filter(|r| elems.contains(&r.source_id) && elems.contains(&r.dest_id))
        .map(|r| r.id.clone())
        .collect();

    let a_name = idx.elements[a].name.clone();
    let b_name = idx.elements[b].name.clone();
    let key = key_for("paths", Some(&a_name), Some(&b_name), None);
    let title = format!("paths: {} → {}", a_name, b_name);
    let desc = if elems.len() <= 1 && !fwd.contains(&b_id) {
        Some(format!("no path from {} to {}", a_name, b_name))
    } else {
        None
    };
    push_generated(ws, generated, key, title, desc, &elems, &rels);
    Ok(())
}

// ---------------------------------------------------------------------------
// asof / delta (spec §8.3)
// ---------------------------------------------------------------------------

fn gen_asof(ws: &mut Workspace, idx: &Index, spec: &AutoViewSpec, generated: &mut Vec<String>) {
    let m = spec.target.as_deref().unwrap_or("now");
    let order = milestone_order(ws);
    let Some(at) = ms_pos(&order, m) else {
        eprintln!("note: `auto asof {}` skipped — unknown milestone", m);
        return;
    };
    let (elems, rels) = asof_sets(idx, &order, at);
    let key = key_for("asof", Some(m), None, None);
    let title = format!("as of {}", m);
    push_generated(ws, generated, key, title, None, &elems, &rels);
}

fn gen_delta(ws: &mut Workspace, idx: &Index, spec: &AutoViewSpec, generated: &mut Vec<String>) {
    let m1 = spec.target.as_deref().unwrap_or("now");
    let m2 = spec.target2.as_deref().unwrap_or("now");
    let order = milestone_order(ws);
    let (Some(p1), Some(p2)) = (ms_pos(&order, m1), ms_pos(&order, m2)) else {
        eprintln!("note: `auto delta {} {}` skipped — unknown milestone", m1, m2);
        return;
    };
    let (e1, r1) = asof_sets(idx, &order, p1);
    let (e2, r2) = asof_sets(idx, &order, p2);

    let elems: BTreeSet<String> = e1.union(&e2).cloned().collect();
    let rels: BTreeSet<String> = r1.union(&r2).cloned().collect();

    let name_of = |id: &String| {
        idx.by_id
            .get(id)
            .map(|i| idx.elements[*i].name.clone())
            .unwrap_or_else(|| id.clone())
    };
    let added_elems: Vec<String> = e2.difference(&e1).map(name_of).collect();
    let removed_elems: Vec<String> = e1.difference(&e2).map(name_of).collect();
    let added_rels = r2.difference(&r1).count();
    let removed_rels = r1.difference(&r2).count();

    let description = format!(
        "added: {} elements ({}), {} relationships; removed: {} elements ({}), {} relationships",
        added_elems.len(),
        added_elems.join(", "),
        added_rels,
        removed_elems.len(),
        removed_elems.join(", "),
        removed_rels,
    );

    let key = key_for("delta", Some(m1), Some(m2), None);
    let title = format!("delta: {} → {}", m1, m2);
    if all_view_keys(ws).contains(&key) {
        return;
    }
    // Record added/removed ids so renderers can style them (spec §8.3).
    let join = |ids: Vec<&String>| ids.into_iter().cloned().collect::<Vec<_>>().join(",");
    let mut properties = std::collections::HashMap::new();
    properties.insert("delta.addedElements".to_string(), join(e2.difference(&e1).collect()));
    properties.insert("delta.removedElements".to_string(), join(e1.difference(&e2).collect()));
    properties.insert("delta.addedRelationships".to_string(), join(r2.difference(&r1).collect()));
    properties.insert("delta.removedRelationships".to_string(), join(r1.difference(&r2).collect()));
    let view = SystemLandscapeView {
        key: Some(key.clone()),
        title: Some(title),
        description: Some(description),
        properties: Some(properties),
        element_views: element_views(&elems),
        relationship_views: relationship_views(&rels),
        ..Default::default()
    };
    ws.views.system_landscape_views.get_or_insert_with(Vec::new).push(view);
    generated.push(key);
}

// ---------------------------------------------------------------------------
// lint (spec §6.3: model hygiene)
// ---------------------------------------------------------------------------

fn gen_lint(ws: &mut Workspace, idx: &Index, generated: &mut Vec<String>) {
    let findings = crate::lint::lint(ws);
    let flagged: BTreeSet<String> = findings.iter().map(|f| f.element_id.clone()).collect();

    // Legacy grouped one-line description for the generated view.
    let mut parts: Vec<String> = Vec::new();
    for (code, label) in [
        ("placeholder", "placeholders"),
        ("uncertain", "uncertain"),
        ("orphan", "orphans"),
        ("unbound-port", "unbound ports"),
    ] {
        let names: Vec<&str> = findings
            .iter()
            .filter(|f| f.code == code)
            .map(|f| f.name.as_str())
            .collect();
        if !names.is_empty() {
            parts.push(format!("{}: {}", label, names.join(", ")));
        }
    }
    let description = if parts.is_empty() {
        "no findings".to_string()
    } else {
        parts.join("; ")
    };
    let rels = induced_rels(idx, &flagged);
    push_generated(
        ws,
        generated,
        "auto-lint".to_string(),
        "lint".to_string(),
        Some(description),
        &flagged,
        &rels,
    );
}
