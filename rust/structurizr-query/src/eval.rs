//! Evaluator for selector expressions.
//!
//! The entry point is [`eval`], which builds an index over the workspace model
//! in a single pass and then recursively evaluates the [`Expr`] AST.

use std::collections::{BTreeSet, HashSet, VecDeque};

use structurizr_model::Workspace;

use crate::{CompOp, Expr, QueryError, Selection};

/// The other modules in this crate reached for these through `eval` before the
/// index was split out; re-export so those paths keep working.
pub(crate) use crate::index::{
    build_index, ElementEntry as ElemEntry, Index, RelationshipEntry as RelEntry,
};

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
            elem.parent_id
                .as_ref()
                .is_some_and(|pid| pid.to_lowercase() == v)
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

        "technology" => elem
            .technology
            .as_ref()
            .is_some_and(|t| t.to_lowercase() == v),

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

    let elem_ids: BTreeSet<String> = visited
        .iter()
        .map(|&i| idx.elements[i].id.clone())
        .collect();

    // Induced subgraph: include relationships whose both endpoints are in the
    // element set (consistent with the §6.1 induced-subgraph rule).
    let rel_ids: BTreeSet<String> = idx
        .relationships
        .iter()
        .filter(|r| elem_ids.contains(&r.source_id) && elem_ids.contains(&r.dest_id))
        .map(|r| r.id.clone())
        .collect();

    Ok(Selection {
        elements: elem_ids,
        relationships: rel_ids,
    })
}

// ---------------------------------------------------------------------------
// Recursive evaluator
// ---------------------------------------------------------------------------

fn eval_expr(expr: &Expr, idx: &Index) -> Result<Selection, QueryError> {
    match expr {
        Expr::Star => Ok(Selection {
            elements: all_elem_ids(idx),
            relationships: all_rel_ids(idx),
        }),

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
            Ok(Selection {
                elements,
                relationships: BTreeSet::new(),
            })
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
            Ok(Selection {
                elements: BTreeSet::new(),
                relationships,
            })
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
            Ok(Selection {
                elements,
                relationships,
            })
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
