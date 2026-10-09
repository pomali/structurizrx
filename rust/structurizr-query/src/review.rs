//! Element-by-element review data (the walkthrough / audit surface).
//!
//! This is the read model behind the web review page: every element with its
//! neighbourhood, the views it appears in, and the hygiene findings that apply
//! to it. It is a projection over [`crate::index::Index`] — no traversal of the
//! workspace happens here.
//!
//! # Why not `lint`
//!
//! [`crate::lint`] is the *gate*: `validate --strict` fails on its findings, so
//! its codes are deliberately few and unambiguous. Review adds softer checks —
//! a missing description is worth showing a reviewer but is not a reason to
//! fail a build. Keeping them apart means adding a review check can never
//! change the exit status of an existing workspace. `lint`'s findings are
//! folded in here so the reviewer still sees everything in one place.

use std::collections::HashMap;

use serde::Serialize;
use structurizr_model::Workspace;

use crate::index::{build_index, Index};
use crate::lint::lint;

/// One hygiene observation about one element.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewFinding {
    /// Stable machine-readable code. Review-only codes are
    /// `missing-description`, `missing-technology`, `not-in-any-view`,
    /// `no-relationships`, `duplicate-name` and `relationship-undescribed`;
    /// the rest come from [`crate::lint`].
    pub code: &'static str,
    pub message: String,
    /// True when the finding also fails `validate --strict`.
    pub blocking: bool,
}

/// A view an element appears in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewViewRef {
    pub key: String,
    pub name: String,
    pub kind: &'static str,
}

/// One end of a relationship, from the reviewed element's point of view.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewRelationship {
    pub id: String,
    /// Id of the element at the other end.
    pub other_id: String,
    /// Name of the element at the other end, or the raw id if it is not a
    /// model element (a deployment instance, say).
    pub other_name: String,
    pub description: Option<String>,
    pub technology: Option<String>,
    pub kind: Option<String>,
}

/// Everything the review pane shows for one element.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewElement {
    pub id: String,
    pub name: String,
    pub kind: &'static str,
    pub description: Option<String>,
    pub technology: Option<String>,
    pub group: Option<String>,
    pub tags: Vec<String>,
    pub parent_id: Option<String>,
    pub parent_name: Option<String>,
    pub children: Vec<String>,
    pub incoming: Vec<ReviewRelationship>,
    pub outgoing: Vec<ReviewRelationship>,
    pub views: Vec<ReviewViewRef>,
    pub findings: Vec<ReviewFinding>,
}

/// The whole workspace, element by element.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    pub elements: Vec<ReviewElement>,
}

/// Build the review model. Element order is model order, which is stable
/// across runs, so a review position stays meaningful between reloads.
pub fn review(workspace: &Workspace) -> Review {
    let idx = build_index(workspace);

    // Findings from the strict gate, grouped by element so they can be shown
    // alongside the softer ones.
    let mut blocking: HashMap<String, Vec<ReviewFinding>> = HashMap::new();
    for f in lint(workspace) {
        blocking
            .entry(f.element_id.clone())
            .or_default()
            .push(ReviewFinding {
                code: f.code,
                message: f.message,
                blocking: true,
            });
    }

    let duplicates = duplicate_names(&idx);

    let elements = idx
        .elements
        .iter()
        .map(|e| {
            let (in_degree, out_degree) = idx.degree(&e.id);
            let mut findings = blocking.remove(&e.id).unwrap_or_default();

            if e.description.as_deref().unwrap_or("").trim().is_empty() {
                findings.push(ReviewFinding {
                    code: "missing-description",
                    message: format!("'{}' has no description", e.name),
                    blocking: false,
                });
            }

            // Technology is only meaningful for the levels that deploy
            // something; a person or a software system has none by design.
            if matches!(e.kind, "container" | "component")
                && e.technology.as_deref().unwrap_or("").trim().is_empty()
            {
                findings.push(ReviewFinding {
                    code: "missing-technology",
                    message: format!("'{}' has no technology", e.name),
                    blocking: false,
                });
            }

            let views: Vec<ReviewViewRef> = idx
                .views_for(&e.id)
                .into_iter()
                .map(|v| ReviewViewRef {
                    key: v.key.clone(),
                    name: v.name.clone(),
                    kind: v.kind,
                })
                .collect();

            if views.is_empty() {
                findings.push(ReviewFinding {
                    code: "not-in-any-view",
                    message: format!("'{}' is not shown by any view", e.name),
                    blocking: false,
                });
            }

            let children: Vec<String> = idx
                .children(&e.id)
                .into_iter()
                .map(|c| c.id.clone())
                .collect();

            // A parent with children is a boundary, not an orphan: its
            // children carry the relationships.
            if in_degree == 0 && out_degree == 0 && children.is_empty() {
                findings.push(ReviewFinding {
                    code: "no-relationships",
                    message: format!("'{}' has no incoming or outgoing relationships", e.name),
                    blocking: false,
                });
            }

            if let Some(others) = duplicates.get(&normalise(&e.name)) {
                let others: Vec<&str> = others
                    .iter()
                    .filter(|id| *id != &e.id)
                    .filter_map(|id| idx.element(id).map(|o| o.name.as_str()))
                    .collect();
                if !others.is_empty() {
                    findings.push(ReviewFinding {
                        code: "duplicate-name",
                        message: format!(
                            "'{}' is hard to tell apart from {}",
                            e.name,
                            others
                                .iter()
                                .map(|n| format!("'{n}'"))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                        blocking: false,
                    });
                }
            }

            let outgoing = relationships(&idx, idx.outgoing(&e.id), true);
            let incoming = relationships(&idx, idx.incoming(&e.id), false);

            let undescribed = outgoing
                .iter()
                .filter(|r| r.description.as_deref().unwrap_or("").trim().is_empty())
                .count();
            if undescribed > 0 {
                findings.push(ReviewFinding {
                    code: "relationship-undescribed",
                    message: format!(
                        "{undescribed} of {} relationship(s) leaving '{}' have no description",
                        outgoing.len(),
                        e.name
                    ),
                    blocking: false,
                });
            }

            ReviewElement {
                id: e.id.clone(),
                name: e.name.clone(),
                kind: e.kind,
                description: e.description.clone(),
                technology: e.technology.clone(),
                group: e.group.clone(),
                tags: e.tags.clone(),
                parent_id: e.parent_id.clone(),
                parent_name: e
                    .parent_id
                    .as_ref()
                    .and_then(|p| idx.element(p))
                    .map(|p| p.name.clone()),
                children,
                incoming,
                outgoing,
                views,
                findings,
            }
        })
        .collect();

    Review { elements }
}

fn relationships(
    idx: &Index,
    rels: Vec<&crate::index::RelationshipEntry>,
    outgoing: bool,
) -> Vec<ReviewRelationship> {
    rels.into_iter()
        .map(|r| {
            let other_id = if outgoing { &r.dest_id } else { &r.source_id };
            ReviewRelationship {
                id: r.id.clone(),
                other_id: other_id.clone(),
                other_name: idx
                    .element(other_id)
                    .map(|o| o.name.clone())
                    .unwrap_or_else(|| other_id.clone()),
                description: r.description.clone(),
                technology: r.technology.clone(),
                kind: r.kind.clone(),
            }
        })
        .collect()
}

/// Names that collide once case, spacing and punctuation are ignored — the
/// cheap proxy for "two elements a reader would confuse".
fn duplicate_names(idx: &Index) -> HashMap<String, Vec<String>> {
    let mut by_name: HashMap<String, Vec<String>> = HashMap::new();
    for e in &idx.elements {
        by_name
            .entry(normalise(&e.name))
            .or_default()
            .push(e.id.clone());
    }
    by_name.retain(|_, ids| ids.len() > 1);
    by_name
}

fn normalise(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use structurizr_model::{Container, Model, SoftwareSystem, Workspace};

    fn workspace() -> Workspace {
        let mut ws = Workspace {
            name: "T".to_string(),
            ..Default::default()
        };
        ws.model = Model {
            software_systems: Some(vec![
                SoftwareSystem {
                    id: "1".into(),
                    name: "Shop".into(),
                    description: Some("The shop".into()),
                    containers: Some(vec![Container {
                        id: "2".into(),
                        name: "API".into(),
                        ..Default::default()
                    }]),
                    ..Default::default()
                },
                SoftwareSystem {
                    id: "3".into(),
                    name: "shop!".into(),
                    ..Default::default()
                },
            ]),
            ..Default::default()
        };
        ws
    }

    fn findings<'a>(review: &'a Review, id: &str) -> Vec<&'a str> {
        review
            .elements
            .iter()
            .find(|e| e.id == id)
            .expect("element")
            .findings
            .iter()
            .map(|f| f.code)
            .collect()
    }

    #[test]
    fn reports_missing_description_and_technology() {
        let r = review(&workspace());
        let api = findings(&r, "2");
        assert!(api.contains(&"missing-description"));
        assert!(api.contains(&"missing-technology"));

        // The described system is not flagged for a description.
        assert!(!findings(&r, "1").contains(&"missing-description"));
    }

    #[test]
    fn technology_is_only_expected_of_containers_and_components() {
        let r = review(&workspace());
        assert!(!findings(&r, "1").contains(&"missing-technology"));
    }

    #[test]
    fn a_parent_with_children_is_not_flagged_as_unrelated() {
        let r = review(&workspace());
        assert!(!findings(&r, "1").contains(&"no-relationships"));
        assert!(findings(&r, "2").contains(&"no-relationships"));
    }

    #[test]
    fn near_duplicate_names_are_reported_on_both_elements() {
        let r = review(&workspace());
        assert!(findings(&r, "1").contains(&"duplicate-name"));
        assert!(findings(&r, "3").contains(&"duplicate-name"));
    }

    #[test]
    fn elements_with_no_view_are_reported() {
        let r = review(&workspace());
        assert!(findings(&r, "1").contains(&"not-in-any-view"));
    }

    #[test]
    fn lint_findings_are_folded_in_and_marked_blocking() {
        let r = review(&workspace());
        let orphan = r
            .elements
            .iter()
            .find(|e| e.id == "2")
            .unwrap()
            .findings
            .iter()
            .find(|f| f.code == "orphan")
            .expect("lint orphan finding is carried into the review");
        assert!(orphan.blocking);
    }
}
