//! Model-hygiene lint (spec §6.3), exposed as structured findings so tools
//! (CLI `validate --strict/--json`, the web server) can report code,
//! element id, and message rather than a prose blob.

use structurizr_model::Workspace;

use crate::eval::build_index;

/// A single lint finding about one element (or one port on an element).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintFinding {
    /// Stable machine-readable code: `placeholder`, `uncertain`, `orphan`,
    /// or `unbound-port`.
    pub code: &'static str,
    /// Id of the element the finding is about.
    pub element_id: String,
    /// Element name (`Element.port` for `unbound-port`).
    pub name: String,
    /// One-line human-readable description.
    pub message: String,
}

/// Run the model-hygiene checks and return all findings in deterministic
/// order (by check, then model order).
pub fn lint(workspace: &Workspace) -> Vec<LintFinding> {
    use std::collections::HashSet;

    let idx = build_index(workspace);
    let mut findings: Vec<LintFinding> = Vec::new();

    for e in idx.elements.iter().filter(|e| e.tags.iter().any(|t| t == "Placeholder")) {
        findings.push(LintFinding {
            code: "placeholder",
            element_id: e.id.clone(),
            name: e.name.clone(),
            message: format!("'{}' is a placeholder auto-created in sketch mode; declare it properly", e.name),
        });
    }

    for e in idx.elements.iter().filter(|e| e.tags.iter().any(|t| t == "Uncertain")) {
        findings.push(LintFinding {
            code: "uncertain",
            element_id: e.id.clone(),
            name: e.name.clone(),
            message: format!("'{}' is marked uncertain (?)", e.name),
        });
    }

    // Orphans: leaf-level elements with no relationships in either direction
    // and no children (a bare system with containers is a boundary, not an orphan).
    let connected: HashSet<&String> = idx
        .relationships
        .iter()
        .flat_map(|r| [&r.source_id, &r.dest_id])
        .collect();
    let has_children: HashSet<&String> = idx
        .elements
        .iter()
        .filter_map(|e| e.parent_id.as_ref())
        .collect();
    for e in idx
        .elements
        .iter()
        .filter(|e| !connected.contains(&e.id) && !has_children.contains(&e.id))
    {
        findings.push(LintFinding {
            code: "orphan",
            element_id: e.id.clone(),
            name: e.name.clone(),
            message: format!("'{}' has no relationships and no children", e.name),
        });
    }

    // Unbound ports: declared but never referenced by any relationship.
    let used_ports: HashSet<(String, String)> = idx
        .relationships
        .iter()
        .flat_map(|r| {
            [
                r.source_port_id.as_ref().map(|p| (r.source_id.clone(), p.clone())),
                r.dest_port_id.as_ref().map(|p| (r.dest_id.clone(), p.clone())),
            ]
        })
        .flatten()
        .collect();
    for e in &idx.elements {
        for (pid, pname) in &e.ports {
            if !used_ports.contains(&(e.id.clone(), pid.clone())) {
                findings.push(LintFinding {
                    code: "unbound-port",
                    element_id: e.id.clone(),
                    name: format!("{}.{}", e.name, pname),
                    message: format!("port '{}' on '{}' is never used by a relationship", pname, e.name),
                });
            }
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use structurizr_model::{Model, SoftwareSystem, Workspace};

    #[test]
    fn orphan_and_placeholder_reported_with_codes() {
        let mut ws = Workspace::default();
        ws.name = "T".to_string();
        ws.model = Model {
            software_systems: Some(vec![
                SoftwareSystem {
                    id: "1".into(),
                    name: "Lonely".into(),
                    ..Default::default()
                },
                SoftwareSystem {
                    id: "2".into(),
                    name: "ghost".into(),
                    tags: Some("Element,Software System,Placeholder".into()),
                    ..Default::default()
                },
            ]),
            ..Default::default()
        };

        let findings = lint(&ws);
        assert!(findings.iter().any(|f| f.code == "orphan" && f.element_id == "1"));
        assert!(findings.iter().any(|f| f.code == "placeholder" && f.element_id == "2"));
        for f in &findings {
            assert!(!f.message.is_empty());
        }
    }
}
