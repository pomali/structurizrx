//! Evaluator for selector expressions.
//!
//! The entry point is [`eval`], which builds an index over the workspace model
//! in a single pass and then recursively evaluates the [`Expr`] AST.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use structurizr_model::{RelationshipKind, Status, Workspace};

use crate::{CompOp, Expr, QueryError, Selection};

// ---------------------------------------------------------------------------
// Index entries
// ---------------------------------------------------------------------------

/// Everything about one model element needed for query evaluation.
#[derive(Debug)]
pub(crate) struct ElemEntry {
    pub(crate) id: String,
    pub(crate) name: String,
    /// Structural kind: "person", "softwareSystem", "container", "component", "custom".
    pub(crate) kind: &'static str,
    pub(crate) tags: Vec<String>,
    pub(crate) group: Option<String>,
    pub(crate) technology: Option<String>,
    /// Lowercase serde name of the status variant ("idea", "draft", …).
    pub(crate) status: Option<String>,
    pub(crate) properties: HashMap<String, String>,
    /// Names of perspectives carried by this element.
    pub(crate) perspectives: Vec<String>,
    /// Direct parent element id (None for top-level elements).
    pub(crate) parent_id: Option<String>,
    /// All ancestor ids from closest to farthest (for `parent^`).
    pub(crate) ancestors: Vec<String>,
    /// Names of the corresponding ancestors.
    pub(crate) ancestor_names: Vec<String>,
    /// Lifecycle milestone names (spec §8).
    pub(crate) introduced: Option<String>,
    pub(crate) retired: Option<String>,
    /// Declared ports as (port id, port name).
    pub(crate) ports: Vec<(String, String)>,
}

/// Everything about one relationship needed for query evaluation.
#[derive(Debug)]
pub(crate) struct RelEntry {
    pub(crate) id: String,
    pub(crate) source_id: String,
    pub(crate) dest_id: String,
    /// Lowercase serde name ("sync", "async", …).
    pub(crate) kind: Option<String>,
    /// Lowercase serde name.
    pub(crate) status: Option<String>,
    pub(crate) tags: Vec<String>,
    pub(crate) perspectives: Vec<String>,
    pub(crate) properties: HashMap<String, String>,
    pub(crate) introduced: Option<String>,
    pub(crate) retired: Option<String>,
    pub(crate) source_port_id: Option<String>,
    pub(crate) dest_port_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Index
// ---------------------------------------------------------------------------

pub(crate) struct Index {
    pub(crate) elements: Vec<ElemEntry>,
    pub(crate) relationships: Vec<RelEntry>,
    /// Maps element id → index into `elements`.
    pub(crate) by_id: HashMap<String, usize>,
    /// Maps lowercase element name → index into `elements`.
    pub(crate) by_name: HashMap<String, usize>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn status_str(s: Status) -> &'static str {
    match s {
        Status::Idea => "idea",
        Status::Draft => "draft",
        Status::Specified => "specified",
        Status::Implemented => "implemented",
        Status::Deprecated => "deprecated",
    }
}

fn rel_kind_str(k: RelationshipKind) -> &'static str {
    match k {
        RelationshipKind::Sync => "sync",
        RelationshipKind::Async => "async",
        RelationshipKind::Publish => "publish",
        RelationshipKind::Subscribe => "subscribe",
        RelationshipKind::Dataflow => "dataflow",
        RelationshipKind::Dependency => "dependency",
        RelationshipKind::Deploy => "deploy",
    }
}

fn split_tags(tags: &Option<String>) -> Vec<String> {
    match tags {
        Some(t) => t
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        None => vec![],
    }
}

fn persp_names(ps: &Option<Vec<structurizr_model::Perspective>>) -> Vec<String> {
    match ps {
        Some(v) => v.iter().map(|p| p.name.clone()).collect(),
        None => vec![],
    }
}

fn props(m: &Option<HashMap<String, String>>) -> HashMap<String, String> {
    m.clone().unwrap_or_default()
}

fn port_pairs(ports: &Option<Vec<structurizr_model::Port>>) -> Vec<(String, String)> {
    ports
        .iter()
        .flatten()
        .map(|p| (p.id.clone(), p.name.clone()))
        .collect()
}

// ---------------------------------------------------------------------------
// Index construction
// ---------------------------------------------------------------------------

pub(crate) fn build_index(workspace: &Workspace) -> Index {
    let mut elements: Vec<ElemEntry> = Vec::new();
    let mut relationships: Vec<RelEntry> = Vec::new();

    // Helper: push one relationship entry.
    let mut push_rel = |r: &structurizr_model::Relationship| {
        relationships.push(RelEntry {
            id: r.id.clone(),
            source_id: r.source_id.clone(),
            dest_id: r.destination_id.clone(),
            kind: r.kind.map(|k| rel_kind_str(k).to_string()),
            status: r.status.map(|s| status_str(s).to_string()),
            tags: split_tags(&r.tags),
            perspectives: persp_names(&r.perspectives),
            properties: props(&r.properties),
            introduced: r.introduced.clone(),
            retired: r.retired.clone(),
            source_port_id: r.source_port_id.clone(),
            dest_port_id: r.destination_port_id.clone(),
        });
    };

    let model = &workspace.model;

    // --- People ---
    for p in model.people.as_deref().unwrap_or(&[]) {
        elements.push(ElemEntry {
            id: p.id.clone(),
            name: p.name.clone(),
            kind: "person",
            tags: split_tags(&p.tags),
            group: p.group.clone(),
            technology: None,
            status: p.status.map(|s| status_str(s).to_string()),
            properties: props(&p.properties),
            perspectives: persp_names(&p.perspectives),
            parent_id: None,
            ancestors: vec![],
            ancestor_names: vec![],
            introduced: p.introduced.clone(),
            retired: p.retired.clone(),
            ports: port_pairs(&p.ports),
        });
        for r in p.relationships.as_deref().unwrap_or(&[]) {
            push_rel(r);
        }
    }

    // --- Software systems → containers → components ---
    for sys in model.software_systems.as_deref().unwrap_or(&[]) {
        elements.push(ElemEntry {
            id: sys.id.clone(),
            name: sys.name.clone(),
            kind: "softwareSystem",
            tags: split_tags(&sys.tags),
            group: sys.group.clone(),
            technology: None,
            status: sys.status.map(|s| status_str(s).to_string()),
            properties: props(&sys.properties),
            perspectives: persp_names(&sys.perspectives),
            parent_id: None,
            ancestors: vec![],
            ancestor_names: vec![],
            introduced: sys.introduced.clone(),
            retired: sys.retired.clone(),
            ports: port_pairs(&sys.ports),
        });
        for r in sys.relationships.as_deref().unwrap_or(&[]) {
            push_rel(r);
        }

        for cont in sys.containers.as_deref().unwrap_or(&[]) {
            elements.push(ElemEntry {
                id: cont.id.clone(),
                name: cont.name.clone(),
                kind: "container",
                tags: split_tags(&cont.tags),
                group: cont.group.clone(),
                technology: cont.technology.clone(),
                status: cont.status.map(|s| status_str(s).to_string()),
                properties: props(&cont.properties),
                perspectives: persp_names(&cont.perspectives),
                parent_id: Some(sys.id.clone()),
                ancestors: vec![sys.id.clone()],
                ancestor_names: vec![sys.name.clone()],
                introduced: cont.introduced.clone(),
                retired: cont.retired.clone(),
                ports: port_pairs(&cont.ports),
            });
            for r in cont.relationships.as_deref().unwrap_or(&[]) {
                push_rel(r);
            }

            for comp in cont.components.as_deref().unwrap_or(&[]) {
                elements.push(ElemEntry {
                    id: comp.id.clone(),
                    name: comp.name.clone(),
                    kind: "component",
                    tags: split_tags(&comp.tags),
                    group: comp.group.clone(),
                    technology: comp.technology.clone(),
                    status: comp.status.map(|s| status_str(s).to_string()),
                    properties: props(&comp.properties),
                    perspectives: persp_names(&comp.perspectives),
                    parent_id: Some(cont.id.clone()),
                    ancestors: vec![cont.id.clone(), sys.id.clone()],
                    ancestor_names: vec![cont.name.clone(), sys.name.clone()],
                    introduced: comp.introduced.clone(),
                    retired: comp.retired.clone(),
                    ports: port_pairs(&comp.ports),
                });
                for r in comp.relationships.as_deref().unwrap_or(&[]) {
                    push_rel(r);
                }
            }
        }
    }

    // --- Custom elements ---
    for c in model.custom_elements.as_deref().unwrap_or(&[]) {
        elements.push(ElemEntry {
            id: c.id.clone(),
            name: c.name.clone(),
            kind: "custom",
            tags: split_tags(&c.tags),
            group: c.group.clone(),
            technology: None,
            status: c.status.map(|s| status_str(s).to_string()),
            properties: props(&c.properties),
            perspectives: persp_names(&c.perspectives),
            parent_id: None,
            ancestors: vec![],
            ancestor_names: vec![],
            introduced: c.introduced.clone(),
            retired: c.retired.clone(),
            ports: port_pairs(&c.ports),
        });
        for r in c.relationships.as_deref().unwrap_or(&[]) {
            push_rel(r);
        }
    }

    // Build look-up maps.
    let mut by_id = HashMap::new();
    let mut by_name = HashMap::new();
    for (i, e) in elements.iter().enumerate() {
        by_id.insert(e.id.clone(), i);
        by_name.insert(e.name.to_lowercase(), i);
    }

    Index { elements, relationships, by_id, by_name }
}

// ---------------------------------------------------------------------------
// Universe helpers
// ---------------------------------------------------------------------------

fn all_elem_ids(idx: &Index) -> BTreeSet<String> {
    idx.elements.iter().map(|e| e.id.clone()).collect()
}

fn all_rel_ids(idx: &Index) -> BTreeSet<String> {
    idx.relationships.iter().map(|r| r.id.clone()).collect()
}

// ---------------------------------------------------------------------------
// Comparison predicates (==, case-insensitive)
// ---------------------------------------------------------------------------

/// Returns true iff `elem` satisfies the path==(eq) value predicate.
fn elem_eq(elem: &ElemEntry, path: &[String], value: &str) -> bool {
    let v = value.to_lowercase();
    match path[0].as_str() {
        "tag" => elem.tags.iter().any(|t| t.to_lowercase() == v),

        "kind" => {
            // structural kind OR the element's `kind` property (kind-alias support)
            elem.kind.to_lowercase() == v
                || elem
                    .properties
                    .get("kind")
                    .is_some_and(|k| k.to_lowercase() == v)
        }

        "status" => elem.status.as_ref() == Some(&v),

        "layer" => {
            // group field OR `layer` property
            elem.group.as_ref().is_some_and(|g| g.to_lowercase() == v)
                || elem
                    .properties
                    .get("layer")
                    .is_some_and(|l| l.to_lowercase() == v)
        }

        "perspective" => elem.perspectives.iter().any(|p| p.to_lowercase() == v),

        "parent" => {
            // direct parent by id or by name
            elem.parent_id.as_ref().is_some_and(|pid| pid.to_lowercase() == v)
                || elem
                    .ancestor_names
                    .first()
                    .is_some_and(|n| n.to_lowercase() == v)
        }

        "parent^" => {
            // any ancestor by id or by name
            elem.ancestors.iter().any(|a| a.to_lowercase() == v)
                || elem.ancestor_names.iter().any(|a| a.to_lowercase() == v)
        }

        "technology" => elem.technology.as_ref().is_some_and(|t| t.to_lowercase() == v),

        "name" => elem.name.to_lowercase() == v,

        "property" if path.len() >= 2 => {
            // property key is exact; value comparison is case-insensitive
            elem.properties
                .get(&path[1])
                .is_some_and(|pv| pv.to_lowercase() == v)
        }

        _ => false,
    }
}

/// Returns true iff `rel` satisfies the path==(eq) value predicate.
fn rel_eq(rel: &RelEntry, path: &[String], value: &str) -> bool {
    let v = value.to_lowercase();
    match path[0].as_str() {
        "kind" => rel.kind.as_ref() == Some(&v),
        "status" => rel.status.as_ref() == Some(&v),
        "tag" => rel.tags.iter().any(|t| t.to_lowercase() == v),
        "perspective" => rel.perspectives.iter().any(|p| p.to_lowercase() == v),
        "property" if path.len() >= 2 => rel
            .properties
            .get(&path[1])
            .is_some_and(|pv| pv.to_lowercase() == v),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Neighborhood (BFS)
// ---------------------------------------------------------------------------

fn eval_neighborhood(idx: &Index, target: &str, depth: u32) -> Result<Selection, QueryError> {
    // Resolve target: exact id match first, then case-insensitive name lookup.
    let start = idx
        .by_id
        .get(target)
        .copied()
        .or_else(|| idx.by_name.get(&target.to_lowercase()).copied())
        .ok_or_else(|| QueryError::UnknownTarget(target.to_string()))?;

    let mut visited: HashSet<usize> = HashSet::new();
    visited.insert(start);

    if depth > 0 {
        let mut frontier: VecDeque<(usize, u32)> = VecDeque::new();
        frontier.push_back((start, 0));

        while let Some((ei, d)) = frontier.pop_front() {
            if d >= depth {
                continue;
            }
            let eid = &idx.elements[ei].id;
            for rel in &idx.relationships {
                let other_id = if &rel.source_id == eid {
                    &rel.dest_id
                } else if &rel.dest_id == eid {
                    &rel.source_id
                } else {
                    continue;
                };
                if let Some(&oi) = idx.by_id.get(other_id) {
                    if visited.insert(oi) {
                        frontier.push_back((oi, d + 1));
                    }
                }
            }
        }
    }

    let elem_ids: BTreeSet<String> =
        visited.iter().map(|&i| idx.elements[i].id.clone()).collect();

    // Induced subgraph: include relationships whose both endpoints are in the
    // element set (consistent with the §6.1 induced-subgraph rule).
    let rel_ids: BTreeSet<String> = idx
        .relationships
        .iter()
        .filter(|r| elem_ids.contains(&r.source_id) && elem_ids.contains(&r.dest_id))
        .map(|r| r.id.clone())
        .collect();

    Ok(Selection { elements: elem_ids, relationships: rel_ids })
}

// ---------------------------------------------------------------------------
// Recursive evaluator
// ---------------------------------------------------------------------------

fn eval_expr(expr: &Expr, idx: &Index) -> Result<Selection, QueryError> {
    match expr {
        Expr::Star => Ok(Selection { elements: all_elem_ids(idx), relationships: all_rel_ids(idx) }),

        Expr::Neighborhood { target, depth } => eval_neighborhood(idx, target, *depth),

        Expr::ElementComparison { path, op, value } => {
            let matching: BTreeSet<String> = idx
                .elements
                .iter()
                .filter(|e| elem_eq(e, path, value))
                .map(|e| e.id.clone())
                .collect();

            // `!=` is complement within the elements universe; relationships
            // are unaffected (remain empty).
            let elements = match op {
                CompOp::Eq => matching,
                CompOp::Ne => {
                    let mut all = all_elem_ids(idx);
                    for id in &matching {
                        all.remove(id);
                    }
                    all
                }
            };
            Ok(Selection { elements, relationships: BTreeSet::new() })
        }

        Expr::RelationshipComparison { path, op, value } => {
            let matching: BTreeSet<String> = idx
                .relationships
                .iter()
                .filter(|r| rel_eq(r, path, value))
                .map(|r| r.id.clone())
                .collect();

            // `!=` is complement within the relationships universe; elements
            // are unaffected (remain empty).
            let relationships = match op {
                CompOp::Eq => matching,
                CompOp::Ne => {
                    let mut all = all_rel_ids(idx);
                    for id in &matching {
                        all.remove(id);
                    }
                    all
                }
            };
            Ok(Selection { elements: BTreeSet::new(), relationships })
        }

        Expr::And(l, r) => {
            let ls = eval_expr(l, idx)?;
            let rs = eval_expr(r, idx)?;
            Ok(Selection {
                elements: ls.elements.intersection(&rs.elements).cloned().collect(),
                relationships: ls
                    .relationships
                    .intersection(&rs.relationships)
                    .cloned()
                    .collect(),
            })
        }

        Expr::Or(l, r) => {
            let ls = eval_expr(l, idx)?;
            let rs = eval_expr(r, idx)?;
            Ok(Selection {
                elements: ls.elements.union(&rs.elements).cloned().collect(),
                relationships: ls.relationships.union(&rs.relationships).cloned().collect(),
            })
        }

        Expr::Not(inner) => {
            let is = eval_expr(inner, idx)?;
            let mut elements = all_elem_ids(idx);
            for id in &is.elements {
                elements.remove(id);
            }
            let mut relationships = all_rel_ids(idx);
            for id in &is.relationships {
                relationships.remove(id);
            }
            Ok(Selection { elements, relationships })
        }
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn eval(expr: &Expr, workspace: &Workspace) -> Result<Selection, QueryError> {
    let idx = build_index(workspace);
    eval_expr(expr, &idx)
}
