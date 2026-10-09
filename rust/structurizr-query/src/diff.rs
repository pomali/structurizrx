//! Structural comparison of two versions of the same workspace.
//!
//! This is the read model behind the web version-comparison page: what a
//! change to the architecture *is*, rather than what a change to the text of
//! the DSL looks like. `git diff` answers the second question well and the
//! first one badly — moving a container between two software systems is a
//! two-hunk textual change that says nothing about the model, while adding one
//! relationship can be a one-word edit with a large architectural meaning.
//!
//! # Identity: paths, not ids
//!
//! Element ids are assigned by the DSL parser in parse order, so the *same*
//! container has a different id in two revisions as soon as anything is
//! declared before it. Nothing here may key on an id. Instead every element is
//! identified by its **canonical path** — its ancestors' names followed by its
//! own, joined with `/` (`Internet Banking System/API Application/Sign In
//! Controller`) — matched case-insensitively, and a relationship by the paths
//! of its two ends.
//!
//! The consequences are worth knowing:
//!
//! - A rename is reported as one removal and one addition, because from the
//!   model's point of view that is indistinguishable from a replacement. The
//!   [`Diff::renames`] list pairs up the likely candidates so a reader is not
//!   left to spot them, but the two entries stay in place.
//! - Moving an element to a different parent is likewise an add plus a remove,
//!   and shows up in [`Diff::renames`] as a move (same name, different parent).
//! - Two relationships between the same pair of elements are distinguished by
//!   the order they appear in the model, so swapping the order of two parallel
//!   relationships reads as two modifications. That is rare and harmless.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;
use structurizr_model::Workspace;

use crate::index::{build_index, Index};

/// How one thing changed between the two versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Change {
    /// Present in the later version only.
    Added,
    /// Present in the earlier version only.
    Removed,
    /// Present in both, with at least one differing field.
    Modified,
}

impl Change {
    /// Lowercase name, for callers that need it outside of JSON.
    pub fn name(self) -> &'static str {
        match self {
            Change::Added => "added",
            Change::Removed => "removed",
            Change::Modified => "modified",
        }
    }
}

/// One field that differs between the two versions of the same thing.
///
/// `before`/`after` are rendered strings rather than typed values: everything
/// downstream displays them, and a tag list or a property map has no useful
/// typed form to compare against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldChange {
    /// Field name as a reader would say it (`description`, `technology`,
    /// `tags`, or `property "owner"`).
    pub field: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

/// An element that was added, removed or changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementChange {
    /// Canonical path (`System/Container/Component`) — the identity this diff
    /// is keyed on.
    pub path: String,
    pub name: String,
    /// Path of the parent element, if any.
    pub parent: Option<String>,
    /// `person`, `softwareSystem`, `container`, `component` or `custom`. On a
    /// modified element this is the *later* kind; a kind change is also listed
    /// in `fields`.
    pub kind: String,
    pub change: Change,
    /// Empty for additions and removals, non-empty for modifications.
    pub fields: Vec<FieldChange>,
    /// Id of the element in whichever version it exists in (the later one for
    /// a modification), so a caller can link to it in that workspace.
    pub id: String,
}

/// A relationship that was added, removed or changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipChange {
    pub source: String,
    pub destination: String,
    /// Description in the later version, falling back to the earlier one — the
    /// most useful single label for the relationship.
    pub description: Option<String>,
    pub change: Change,
    pub fields: Vec<FieldChange>,
    pub id: String,
}

/// A view that was added, removed or changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewChange {
    pub key: String,
    pub name: String,
    pub kind: String,
    pub change: Change,
    pub fields: Vec<FieldChange>,
    /// Paths of elements the view gained.
    pub elements_added: Vec<String>,
    /// Paths of elements the view no longer shows.
    pub elements_removed: Vec<String>,
}

/// A decision record that was added, removed or changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionChange {
    pub id: String,
    pub title: String,
    pub status: String,
    pub change: Change,
    pub fields: Vec<FieldChange>,
}

/// A removal and an addition that look like the same element under a new name
/// or a new parent.
///
/// This is a *hint*, not a resolution: the pair still appears as one removal
/// and one addition in [`Diff::elements`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameHint {
    pub from: String,
    pub to: String,
    /// Why the two were paired: `renamed` (same parent, new name), `moved`
    /// (same name, new parent) or `renamed and moved`.
    pub reason: &'static str,
}

/// Counts per category, for the headline of a comparison.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryCounts {
    pub added: usize,
    pub removed: usize,
    pub modified: usize,
}

impl CategoryCounts {
    fn record(&mut self, change: Change) {
        match change {
            Change::Added => self.added += 1,
            Change::Removed => self.removed += 1,
            Change::Modified => self.modified += 1,
        }
    }

    /// True when nothing in this category changed.
    pub fn is_empty(&self) -> bool {
        self.added == 0 && self.removed == 0 && self.modified == 0
    }
}

/// Headline counts for a whole comparison.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffSummary {
    pub elements: CategoryCounts,
    pub relationships: CategoryCounts,
    pub views: CategoryCounts,
    pub decisions: CategoryCounts,
    /// Element count in each version, so a reader can see the model's size
    /// move even when the changes are mostly modifications.
    pub element_count_before: usize,
    pub element_count_after: usize,
    pub relationship_count_before: usize,
    pub relationship_count_after: usize,
}

impl DiffSummary {
    /// True when the two versions describe the same architecture.
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
            && self.relationships.is_empty()
            && self.views.is_empty()
            && self.decisions.is_empty()
    }
}

/// Everything that differs between two versions of a workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diff {
    pub summary: DiffSummary,
    /// Workspace-level fields (`name`, `description`).
    pub workspace: Vec<FieldChange>,
    pub elements: Vec<ElementChange>,
    pub relationships: Vec<RelationshipChange>,
    pub views: Vec<ViewChange>,
    pub decisions: Vec<DecisionChange>,
    pub renames: Vec<RenameHint>,
}

/// Compare two versions of a workspace, `before` being the earlier one.
///
/// Output order is deterministic: additions and removals follow the model
/// order of the version they come from, modifications follow the later
/// version's model order. The same pair of inputs always produces the same
/// diff, so it can be used as a CI artefact.
pub fn diff(before: &Workspace, after: &Workspace) -> Diff {
    let a = Side::new(before);
    let b = Side::new(after);

    let mut summary = DiffSummary {
        element_count_before: a.idx.elements.len(),
        element_count_after: b.idx.elements.len(),
        relationship_count_before: a.idx.relationships.len(),
        relationship_count_after: b.idx.relationships.len(),
        ..Default::default()
    };

    let elements = diff_elements(&a, &b, &mut summary.elements);
    let relationships = diff_relationships(&a, &b, &mut summary.relationships);
    let views = diff_views(&a, &b, &mut summary.views);
    let decisions = diff_decisions(before, after, &mut summary.decisions);

    Diff {
        renames: rename_hints(&a, &b, &elements),
        summary,
        workspace: workspace_fields(before, after),
        elements,
        relationships,
        views,
        decisions,
    }
}

// ---------------------------------------------------------------------------
// One side of the comparison
// ---------------------------------------------------------------------------

/// One version, indexed and keyed by path.
struct Side {
    idx: Index,
    /// Lowercase canonical path → index into `idx.elements`.
    by_path: HashMap<String, usize>,
    /// Canonical path per element, in `idx.elements` order.
    paths: Vec<String>,
}

impl Side {
    fn new(workspace: &Workspace) -> Self {
        let idx = build_index(workspace);
        let mut paths = Vec::with_capacity(idx.elements.len());
        let mut by_path = HashMap::with_capacity(idx.elements.len());
        for (i, e) in idx.elements.iter().enumerate() {
            // `ancestor_names` runs closest-first; a path reads root-first.
            let mut parts: Vec<&str> = e.ancestor_names.iter().rev().map(String::as_str).collect();
            parts.push(&e.name);
            let path = parts.join("/");
            by_path.insert(key(&path), i);
            paths.push(path);
        }
        Side {
            idx,
            by_path,
            paths,
        }
    }

    fn element(&self, path: &str) -> Option<&crate::index::ElementEntry> {
        self.by_path.get(&key(path)).map(|&i| &self.idx.elements[i])
    }

    /// The canonical path of `id`, or the raw id when it names something the
    /// index does not carry (a deployment instance, say).
    fn path_of(&self, id: &str) -> String {
        match self.idx.by_id.get(id) {
            Some(&i) => self.paths[i].clone(),
            None => id.to_string(),
        }
    }

    fn parent_path(&self, i: usize) -> Option<String> {
        let e = &self.idx.elements[i];
        e.parent_id.as_ref().map(|id| self.path_of(id))
    }
}

/// The identity key: paths differing only in case are the same element, since
/// the DSL resolves identifiers case-insensitively.
fn key(path: &str) -> String {
    path.to_lowercase()
}

// ---------------------------------------------------------------------------
// Elements
// ---------------------------------------------------------------------------

fn diff_elements(a: &Side, b: &Side, counts: &mut CategoryCounts) -> Vec<ElementChange> {
    let mut out = Vec::new();

    // Removals first, in the earlier version's model order, then additions and
    // modifications in the later version's. Grouping by version keeps a long
    // list readable: everything that disappeared is in one block.
    for (i, path) in a.paths.iter().enumerate() {
        if b.by_path.contains_key(&key(path)) {
            continue;
        }
        let e = &a.idx.elements[i];
        out.push(ElementChange {
            path: path.clone(),
            name: e.name.clone(),
            parent: a.parent_path(i),
            kind: e.kind.to_string(),
            change: Change::Removed,
            fields: Vec::new(),
            id: e.id.clone(),
        });
    }

    for (i, path) in b.paths.iter().enumerate() {
        let e = &b.idx.elements[i];
        let change = match a.element(path) {
            None => ElementChange {
                path: path.clone(),
                name: e.name.clone(),
                parent: b.parent_path(i),
                kind: e.kind.to_string(),
                change: Change::Added,
                fields: Vec::new(),
                id: e.id.clone(),
            },
            Some(old) => {
                let fields = element_fields(old, e);
                if fields.is_empty() {
                    continue;
                }
                ElementChange {
                    path: path.clone(),
                    name: e.name.clone(),
                    parent: b.parent_path(i),
                    kind: e.kind.to_string(),
                    change: Change::Modified,
                    fields,
                    id: e.id.clone(),
                }
            }
        };
        out.push(change);
    }

    for c in &out {
        counts.record(c.change);
    }
    out
}

fn element_fields(
    old: &crate::index::ElementEntry,
    new: &crate::index::ElementEntry,
) -> Vec<FieldChange> {
    let mut fields = Vec::new();
    // The path key is case-insensitive, so a pure change of capitalisation is
    // a modification rather than a replacement — worth reporting.
    compare("name", Some(&old.name), Some(&new.name), &mut fields);
    compare("kind", Some(old.kind), Some(new.kind), &mut fields);
    compare(
        "description",
        old.description.as_deref(),
        new.description.as_deref(),
        &mut fields,
    );
    compare(
        "technology",
        old.technology.as_deref(),
        new.technology.as_deref(),
        &mut fields,
    );
    compare(
        "group",
        old.group.as_deref(),
        new.group.as_deref(),
        &mut fields,
    );
    compare(
        "status",
        old.status.as_deref(),
        new.status.as_deref(),
        &mut fields,
    );
    compare(
        "introduced",
        old.introduced.as_deref(),
        new.introduced.as_deref(),
        &mut fields,
    );
    compare(
        "retired",
        old.retired.as_deref(),
        new.retired.as_deref(),
        &mut fields,
    );
    compare_list("tags", &old.tags, &new.tags, &mut fields);
    compare_list(
        "ports",
        &port_names(&old.ports),
        &port_names(&new.ports),
        &mut fields,
    );
    compare_properties(&old.properties, &new.properties, &mut fields);
    fields
}

fn port_names(ports: &[(String, String)]) -> Vec<String> {
    ports.iter().map(|(_, name)| name.clone()).collect()
}

// ---------------------------------------------------------------------------
// Relationships
// ---------------------------------------------------------------------------

/// Relationships keyed by `source path -> destination path`, with an occurrence
/// index so parallel relationships between the same pair stay distinct.
fn relationship_keys(side: &Side) -> Vec<(String, String, String)> {
    let mut seen: HashMap<(String, String), usize> = HashMap::new();
    side.idx
        .relationships
        .iter()
        .map(|r| {
            let source = side.path_of(&r.source_id);
            let destination = side.path_of(&r.dest_id);
            let pair = (key(&source), key(&destination));
            let n = seen.entry(pair.clone()).or_insert(0);
            let k = format!("{}\u{1f}{}\u{1f}{}", pair.0, pair.1, n);
            *n += 1;
            (k, source, destination)
        })
        .collect()
}

fn diff_relationships(a: &Side, b: &Side, counts: &mut CategoryCounts) -> Vec<RelationshipChange> {
    let a_keys = relationship_keys(a);
    let b_keys = relationship_keys(b);
    let b_index: HashMap<&str, usize> = b_keys
        .iter()
        .enumerate()
        .map(|(i, (k, _, _))| (k.as_str(), i))
        .collect();
    let a_index: HashMap<&str, usize> = a_keys
        .iter()
        .enumerate()
        .map(|(i, (k, _, _))| (k.as_str(), i))
        .collect();

    let mut out = Vec::new();

    for (i, (k, source, destination)) in a_keys.iter().enumerate() {
        if b_index.contains_key(k.as_str()) {
            continue;
        }
        let r = &a.idx.relationships[i];
        out.push(RelationshipChange {
            source: source.clone(),
            destination: destination.clone(),
            description: r.description.clone(),
            change: Change::Removed,
            fields: Vec::new(),
            id: r.id.clone(),
        });
    }

    for (i, (k, source, destination)) in b_keys.iter().enumerate() {
        let r = &b.idx.relationships[i];
        match a_index.get(k.as_str()) {
            None => out.push(RelationshipChange {
                source: source.clone(),
                destination: destination.clone(),
                description: r.description.clone(),
                change: Change::Added,
                fields: Vec::new(),
                id: r.id.clone(),
            }),
            Some(&j) => {
                let old = &a.idx.relationships[j];
                let mut fields = Vec::new();
                compare(
                    "description",
                    old.description.as_deref(),
                    r.description.as_deref(),
                    &mut fields,
                );
                compare(
                    "technology",
                    old.technology.as_deref(),
                    r.technology.as_deref(),
                    &mut fields,
                );
                compare("kind", old.kind.as_deref(), r.kind.as_deref(), &mut fields);
                compare(
                    "status",
                    old.status.as_deref(),
                    r.status.as_deref(),
                    &mut fields,
                );
                compare(
                    "introduced",
                    old.introduced.as_deref(),
                    r.introduced.as_deref(),
                    &mut fields,
                );
                compare(
                    "retired",
                    old.retired.as_deref(),
                    r.retired.as_deref(),
                    &mut fields,
                );
                compare_list("tags", &old.tags, &r.tags, &mut fields);
                compare_properties(&old.properties, &r.properties, &mut fields);
                if fields.is_empty() {
                    continue;
                }
                out.push(RelationshipChange {
                    source: source.clone(),
                    destination: destination.clone(),
                    description: r.description.clone().or_else(|| old.description.clone()),
                    change: Change::Modified,
                    fields,
                    id: r.id.clone(),
                });
            }
        }
    }

    for c in &out {
        counts.record(c.change);
    }
    out
}

// ---------------------------------------------------------------------------
// Views
// ---------------------------------------------------------------------------

fn diff_views(a: &Side, b: &Side, counts: &mut CategoryCounts) -> Vec<ViewChange> {
    let a_views: HashMap<&str, usize> = a
        .idx
        .views
        .iter()
        .enumerate()
        .map(|(i, v)| (v.key.as_str(), i))
        .collect();
    let b_keys: HashSet<&str> = b.idx.views.iter().map(|v| v.key.as_str()).collect();

    let mut out = Vec::new();

    for v in a
        .idx
        .views
        .iter()
        .filter(|v| !b_keys.contains(v.key.as_str()))
    {
        out.push(ViewChange {
            key: v.key.clone(),
            name: v.name.clone(),
            kind: v.kind.to_string(),
            change: Change::Removed,
            fields: Vec::new(),
            elements_added: Vec::new(),
            elements_removed: Vec::new(),
        });
    }

    for v in &b.idx.views {
        match a_views.get(v.key.as_str()) {
            None => out.push(ViewChange {
                key: v.key.clone(),
                name: v.name.clone(),
                kind: v.kind.to_string(),
                change: Change::Added,
                fields: Vec::new(),
                elements_added: Vec::new(),
                elements_removed: Vec::new(),
            }),
            Some(&i) => {
                let old = &a.idx.views[i];
                let mut fields = Vec::new();
                compare("name", Some(&old.name), Some(&v.name), &mut fields);
                compare("kind", Some(old.kind), Some(v.kind), &mut fields);
                compare(
                    "description",
                    old.description.as_deref(),
                    v.description.as_deref(),
                    &mut fields,
                );

                // Membership is compared by path, so a view whose elements
                // merely got new ids reads as unchanged.
                let before: Vec<String> = old.element_ids.iter().map(|id| a.path_of(id)).collect();
                let after: Vec<String> = v.element_ids.iter().map(|id| b.path_of(id)).collect();
                let before_keys: HashSet<String> = before.iter().map(|p| key(p)).collect();
                let after_keys: HashSet<String> = after.iter().map(|p| key(p)).collect();
                let elements_added: Vec<String> = after
                    .iter()
                    .filter(|p| !before_keys.contains(&key(p)))
                    .cloned()
                    .collect();
                let elements_removed: Vec<String> = before
                    .iter()
                    .filter(|p| !after_keys.contains(&key(p)))
                    .cloned()
                    .collect();

                if fields.is_empty() && elements_added.is_empty() && elements_removed.is_empty() {
                    continue;
                }
                out.push(ViewChange {
                    key: v.key.clone(),
                    name: v.name.clone(),
                    kind: v.kind.to_string(),
                    change: Change::Modified,
                    fields,
                    elements_added,
                    elements_removed,
                });
            }
        }
    }

    for c in &out {
        counts.record(c.change);
    }
    out
}

// ---------------------------------------------------------------------------
// Decisions and workspace metadata
// ---------------------------------------------------------------------------

fn decisions_of(ws: &Workspace) -> &[structurizr_model::Decision] {
    ws.documentation
        .as_ref()
        .and_then(|d| d.decisions.as_deref())
        .unwrap_or(&[])
}

fn diff_decisions(
    before: &Workspace,
    after: &Workspace,
    counts: &mut CategoryCounts,
) -> Vec<DecisionChange> {
    let a = decisions_of(before);
    let b = decisions_of(after);
    let b_ids: HashSet<&str> = b.iter().map(|d| d.id.as_str()).collect();
    let a_by_id: HashMap<&str, &structurizr_model::Decision> =
        a.iter().map(|d| (d.id.as_str(), d)).collect();

    let mut out = Vec::new();

    for d in a.iter().filter(|d| !b_ids.contains(d.id.as_str())) {
        out.push(DecisionChange {
            id: d.id.clone(),
            title: d.title.clone(),
            status: d.status.clone(),
            change: Change::Removed,
            fields: Vec::new(),
        });
    }

    for d in b {
        match a_by_id.get(d.id.as_str()) {
            None => out.push(DecisionChange {
                id: d.id.clone(),
                title: d.title.clone(),
                status: d.status.clone(),
                change: Change::Added,
                fields: Vec::new(),
            }),
            Some(old) => {
                let mut fields = Vec::new();
                compare("title", Some(&old.title), Some(&d.title), &mut fields);
                compare("status", Some(&old.status), Some(&d.status), &mut fields);
                compare("date", Some(&old.date), Some(&d.date), &mut fields);
                // The body is shown as "edited" rather than inlined: an ADR is
                // pages long, and a textual diff of prose is what `git diff`
                // is for.
                if old.content != d.content {
                    fields.push(FieldChange {
                        field: "content".to_string(),
                        before: Some(format!("{} characters", old.content.chars().count())),
                        after: Some(format!("{} characters", d.content.chars().count())),
                    });
                }
                if fields.is_empty() {
                    continue;
                }
                out.push(DecisionChange {
                    id: d.id.clone(),
                    title: d.title.clone(),
                    status: d.status.clone(),
                    change: Change::Modified,
                    fields,
                });
            }
        }
    }

    for c in &out {
        counts.record(c.change);
    }
    out
}

fn workspace_fields(before: &Workspace, after: &Workspace) -> Vec<FieldChange> {
    let mut fields = Vec::new();
    compare("name", Some(&before.name), Some(&after.name), &mut fields);
    compare(
        "description",
        before.description.as_deref(),
        after.description.as_deref(),
        &mut fields,
    );
    fields
}

// ---------------------------------------------------------------------------
// Rename / move hints
// ---------------------------------------------------------------------------

/// Pair removals with additions that look like the same element under another
/// name or another parent.
///
/// Candidates are scored on what a reader would use to recognise the element
/// again — an unchanged description is the strongest signal, then the parent it
/// sits in, then its technology — and a pair is only suggested when one
/// candidate scores strictly higher than every other. A refactor that splits
/// one container into three therefore suggests nothing rather than picking one
/// of the three arbitrarily.
fn rename_hints(a: &Side, b: &Side, elements: &[ElementChange]) -> Vec<RenameHint> {
    /// Below this, the two elements have nothing in common but their kind.
    const MIN_SCORE: u32 = 2;

    let removed: Vec<&ElementChange> = elements
        .iter()
        .filter(|c| c.change == Change::Removed)
        .collect();
    let added: Vec<&ElementChange> = elements
        .iter()
        .filter(|c| c.change == Change::Added)
        .collect();

    let mut hints = Vec::new();
    let mut claimed: HashSet<&str> = HashSet::new();

    for r in &removed {
        let Some(old) = a.element(&r.path) else {
            continue;
        };

        let mut scored: Vec<(u32, &&ElementChange)> = added
            .iter()
            .filter(|c| !claimed.contains(c.path.as_str()) && c.kind == r.kind)
            .filter_map(|c| {
                let new = b.element(&c.path)?;
                let mut score = 0;
                if same_non_empty(old.description.as_deref(), new.description.as_deref()) {
                    score += 3;
                }
                if key_opt(&r.parent) == key_opt(&c.parent) {
                    score += 2;
                }
                if same_non_empty(old.technology.as_deref(), new.technology.as_deref()) {
                    score += 2;
                }
                if key(&r.name) == key(&c.name) {
                    score += 1;
                }
                (score >= MIN_SCORE).then_some((score, c))
            })
            .collect();

        scored.sort_by(|x, y| y.0.cmp(&x.0));
        // A tie means two additions are equally good matches; saying nothing is
        // better than guessing.
        if scored.is_empty() || (scored.len() > 1 && scored[0].0 == scored[1].0) {
            continue;
        }

        let winner = scored[0].1;
        let same_parent = key_opt(&winner.parent) == key_opt(&r.parent);
        let same_name = key(&winner.name) == key(&r.name);
        let reason = match (same_name, same_parent) {
            (true, false) => "moved",
            (false, true) => "renamed",
            _ => "renamed and moved",
        };
        claimed.insert(&winner.path);
        hints.push(RenameHint {
            from: r.path.clone(),
            to: winner.path.clone(),
            reason,
        });
    }

    hints
}

/// True when both sides carry the same non-empty value — two elements that both
/// lack a description have not thereby been shown to be the same element.
fn same_non_empty(before: Option<&str>, after: Option<&str>) -> bool {
    let b = before.unwrap_or("").trim();
    !b.is_empty() && b == after.unwrap_or("").trim()
}

fn key_opt(path: &Option<String>) -> Option<String> {
    path.as_deref().map(key)
}

// ---------------------------------------------------------------------------
// Field comparison helpers
// ---------------------------------------------------------------------------

/// Record a change if the two values differ, treating `None` and an empty
/// string as the same absence — the DSL and the JSON schema disagree about
/// which one an unset field is, and a diff that reported `"" → null` for every
/// element would be noise.
fn compare(field: &str, before: Option<&str>, after: Option<&str>, out: &mut Vec<FieldChange>) {
    let b = before.unwrap_or("");
    let a = after.unwrap_or("");
    if b == a {
        return;
    }
    out.push(FieldChange {
        field: field.to_string(),
        before: present(b),
        after: present(a),
    });
}

fn present(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

/// Compare a list as a set, reporting it as `a, b (+c, -d)` so a reader sees
/// which entries moved rather than two full lists to eyeball.
fn compare_list(field: &str, before: &[String], after: &[String], out: &mut Vec<FieldChange>) {
    let b: HashSet<String> = before.iter().map(|t| t.to_lowercase()).collect();
    let a: HashSet<String> = after.iter().map(|t| t.to_lowercase()).collect();
    if b == a {
        return;
    }
    out.push(FieldChange {
        field: field.to_string(),
        before: present(&before.join(", ")),
        after: present(&after.join(", ")),
    });
}

/// Compare property maps entry by entry, so a changed `owner` is one field
/// change and not a rewrite of the whole map.
fn compare_properties(
    before: &HashMap<String, String>,
    after: &HashMap<String, String>,
    out: &mut Vec<FieldChange>,
) {
    // BTreeMap: property order in a HashMap is not stable, and a diff whose
    // field order changes run to run cannot be compared as a CI artefact.
    let mut names: BTreeMap<&str, ()> = BTreeMap::new();
    for k in before.keys().chain(after.keys()) {
        names.insert(k.as_str(), ());
    }
    for name in names.keys() {
        compare(
            &format!("property \"{name}\""),
            before.get(*name).map(String::as_str),
            after.get(*name).map(String::as_str),
            out,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use structurizr_model::{
        Container, ElementView, Model, Relationship, SoftwareSystem, SystemContextView, ViewSet,
        Workspace,
    };

    /// Two software systems, the first with one container, related to each
    /// other. Ids are deliberately *different* between the two versions built
    /// from this helper, to prove the diff never keys on them.
    fn workspace(container_name: &str, description: &str, id_base: u32) -> Workspace {
        let id = |n: u32| (id_base + n).to_string();
        Workspace {
            name: "Bank".to_string(),
            model: Model {
                software_systems: Some(vec![
                    SoftwareSystem {
                        id: id(1),
                        name: "Internet Banking".into(),
                        containers: Some(vec![Container {
                            id: id(2),
                            name: container_name.into(),
                            description: Some(description.into()),
                            technology: Some("Java".into()),
                            relationships: Some(vec![Relationship {
                                id: id(3),
                                source_id: id(2),
                                destination_id: id(4),
                                description: Some("Reads from".into()),
                                ..Default::default()
                            }]),
                            ..Default::default()
                        }]),
                        ..Default::default()
                    },
                    SoftwareSystem {
                        id: id(4),
                        name: "Mainframe".into(),
                        ..Default::default()
                    },
                ]),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn identical_models_with_different_ids_produce_an_empty_diff() {
        let d = diff(
            &workspace("API", "The API", 100),
            &workspace("API", "The API", 500),
        );
        assert!(d.summary.is_empty(), "unexpected changes: {d:?}");
        assert_eq!(d.elements, vec![]);
        assert_eq!(d.relationships, vec![]);
    }

    #[test]
    fn a_changed_description_is_one_modification() {
        let d = diff(
            &workspace("API", "The API", 1),
            &workspace("API", "The gateway", 1),
        );
        assert_eq!(d.summary.elements.modified, 1);
        assert_eq!(d.summary.elements.added, 0);
        let change = &d.elements[0];
        assert_eq!(change.path, "Internet Banking/API");
        assert_eq!(change.change, Change::Modified);
        assert_eq!(
            change.fields,
            vec![FieldChange {
                field: "description".to_string(),
                before: Some("The API".to_string()),
                after: Some("The gateway".to_string()),
            }]
        );
    }

    #[test]
    fn a_rename_is_an_add_and_a_remove_with_a_hint() {
        let d = diff(
            &workspace("API", "The API", 1),
            &workspace("Gateway", "The API", 1),
        );
        assert_eq!(d.summary.elements.added, 1);
        assert_eq!(d.summary.elements.removed, 1);
        assert_eq!(
            d.renames,
            vec![RenameHint {
                from: "Internet Banking/API".to_string(),
                to: "Internet Banking/Gateway".to_string(),
                reason: "renamed",
            }]
        );
    }

    /// The common "rename one container and add another in the same commit"
    /// case: the description and technology pick the right partner out of two
    /// additions in the same parent.
    #[test]
    fn a_rename_alongside_an_addition_still_pairs_up() {
        let before = workspace("API", "The API", 1);
        let mut after = workspace("Gateway", "The API", 1);
        after.model.software_systems.as_mut().unwrap()[0]
            .containers
            .as_mut()
            .unwrap()
            .push(Container {
                id: "50".into(),
                name: "Cache".into(),
                technology: Some("Redis".into()),
                ..Default::default()
            });

        let d = diff(&before, &after);
        assert_eq!(d.summary.elements.added, 2);
        assert_eq!(
            d.renames,
            vec![RenameHint {
                from: "Internet Banking/API".to_string(),
                to: "Internet Banking/Gateway".to_string(),
                reason: "renamed",
            }]
        );
    }

    /// Two equally good candidates suggest nothing: a wrong pairing is worse
    /// than none, because a reader trusts the hint.
    #[test]
    fn an_ambiguous_split_suggests_nothing() {
        let before = workspace("API", "The API", 1);
        let mut after = workspace("Read API", "The API", 1);
        after.model.software_systems.as_mut().unwrap()[0]
            .containers
            .as_mut()
            .unwrap()
            .push(Container {
                id: "50".into(),
                name: "Write API".into(),
                description: Some("The API".into()),
                technology: Some("Java".into()),
                ..Default::default()
            });

        assert_eq!(diff(&before, &after).renames, vec![]);
    }

    #[test]
    fn a_relationship_follows_its_ends_paths_not_its_id() {
        let mut before = workspace("API", "The API", 1);
        let mut after = workspace("API", "The API", 900);

        // Same relationship, different description.
        let rel = |ws: &mut Workspace| {
            ws.model.software_systems.as_mut().unwrap()[0]
                .containers
                .as_mut()
                .unwrap()[0]
                .relationships
                .as_mut()
                .unwrap()[0]
                .description = Some("Writes to".into());
        };
        rel(&mut after);

        let d = diff(&before, &after);
        assert_eq!(d.summary.relationships.modified, 1);
        assert_eq!(d.relationships[0].source, "Internet Banking/API");
        assert_eq!(d.relationships[0].destination, "Mainframe");

        // And a removed relationship is reported once.
        before.model.software_systems.as_mut().unwrap()[0]
            .containers
            .as_mut()
            .unwrap()[0]
            .relationships = None;
        let d = diff(&before, &workspace("API", "The API", 1));
        assert_eq!(d.summary.relationships.added, 1);
        assert_eq!(d.summary.relationships.removed, 0);
    }

    #[test]
    fn view_membership_is_compared_by_path() {
        let mut before = workspace("API", "The API", 0);
        let mut after = workspace("API", "The API", 700);

        let view = |ids: Vec<String>| ViewSet {
            system_context_views: Some(vec![SystemContextView {
                key: Some("ctx".into()),
                title: Some("Context".into()),
                element_views: Some(
                    ids.into_iter()
                        .map(|id| ElementView {
                            id,
                            ..Default::default()
                        })
                        .collect(),
                ),
                ..Default::default()
            }]),
            ..Default::default()
        };

        before.views = view(vec!["1".into(), "4".into()]);
        after.views = view(vec!["701".into(), "704".into()]);
        assert!(
            diff(&before, &after).views.is_empty(),
            "same elements, new ids"
        );

        after.views = view(vec!["701".into()]);
        let d = diff(&before, &after);
        assert_eq!(d.summary.views.modified, 1);
        assert_eq!(d.views[0].elements_removed, vec!["Mainframe".to_string()]);
        assert!(d.views[0].elements_added.is_empty());
    }

    #[test]
    fn absent_and_empty_are_the_same_value() {
        let mut before = workspace("API", "The API", 1);
        let mut after = workspace("API", "The API", 1);
        before.model.software_systems.as_mut().unwrap()[1].description = None;
        after.model.software_systems.as_mut().unwrap()[1].description = Some(String::new());
        assert!(diff(&before, &after).summary.is_empty());
    }
}
