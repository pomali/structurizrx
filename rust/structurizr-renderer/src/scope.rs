//! View-scoping helpers shared by the exporters.
//!
//! A view names a focal element (or an explicit element list); these helpers
//! work out which model elements belong in it, and how relationships between
//! elements that are *not* in the view get lifted onto the ones that are.

use std::collections::{HashMap, HashSet};

use structurizr_model::*;

/// Every relationship in the model, at any nesting level.
pub(crate) fn all_relationships(model: &Model) -> Vec<&Relationship> {
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
pub(crate) fn top_level_owner(model: &Model) -> HashMap<String, String> {
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
pub(crate) fn context_scope(model: &Model, focal_id: &str) -> HashSet<String> {
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
pub(crate) fn container_scope(model: &Model, focal_id: &str) -> HashSet<String> {
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

/// Elements in scope for a component view without an explicit element list:
/// the focal container's components plus the elements they relate to (sibling
/// containers collapsed from their components, external systems, people).
pub(crate) fn component_scope(model: &Model, focal_container_id: &str) -> HashSet<String> {
    // component id → owning container id
    let mut comp_container: HashMap<String, String> = HashMap::new();
    let mut components: HashSet<String> = HashSet::new();
    for ss in model.software_systems.iter().flatten() {
        for c in ss.containers.iter().flatten() {
            for comp in c.components.iter().flatten() {
                comp_container.insert(comp.id.clone(), c.id.clone());
                if c.id == focal_container_id {
                    components.insert(comp.id.clone());
                }
            }
        }
    }

    // Collapse an endpoint to how it should appear in this component view: a
    // focal component stays itself; another container's component collapses to
    // that container; anything else stays as-is.
    let collapse = |id: &str| -> String {
        if components.contains(id) {
            return id.to_string();
        }
        match comp_container.get(id) {
            Some(container) => container.clone(),
            None => id.to_string(),
        }
    };

    let mut scope = components.clone();
    for r in all_relationships(model) {
        let s_in = components.contains(&r.source_id);
        let d_in = components.contains(&r.destination_id);
        if s_in && !d_in {
            scope.insert(collapse(&r.destination_id));
        } else if d_in && !s_in {
            scope.insert(collapse(&r.source_id));
        }
    }
    scope
}

/// Map each container/component id to its parent (component → container,
/// container → software system).  Top-level elements have no entry.
pub(crate) fn child_parent_map(model: &Model) -> HashMap<String, String> {
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

/// Lift an element id to its nearest **visible** ancestor, or `None` when no
/// ancestor is in the view.  This is what produces the implied relationships of
/// upstream Structurizr: a component→system relationship becomes container→system
/// in a container view, and so on.
pub(crate) fn lift_to_visible(
    id: &str,
    parents: &HashMap<String, String>,
    visible: &HashSet<String>,
) -> Option<String> {
    let mut cur = id.to_string();
    loop {
        if visible.contains(&cur) {
            return Some(cur);
        }
        cur = parents.get(&cur)?.clone();
    }
}

/// Build an element ID allowlist plus stored positions from a view's `element_views`.
///
/// Returns `(None, _)` when absent or empty (meaning "no explicit element list").
pub(crate) fn build_element_filter(
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
pub(crate) fn build_rel_filter(rel_views: Option<&[RelationshipView]>) -> Option<HashSet<String>> {
    let rvs = rel_views?;
    if rvs.is_empty() {
        return None;
    }
    Some(rvs.iter().map(|rv| rv.id.clone()).collect())
}

/// Returns `true` if `id` is allowed by the element filter.
///
/// When the filter is `None` (no `element_views` present), all elements are allowed.
pub(crate) fn elem_allowed(filter: &Option<HashSet<String>>, id: &str) -> bool {
    match filter {
        None => true,
        Some(set) => set.contains(id),
    }
}
