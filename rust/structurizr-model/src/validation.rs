use std::collections::{HashMap, HashSet};

use thiserror::Error;

use crate::{DeploymentNode, Port, Relationship, Workspace};

/// Validation error.
#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("workspace name is empty")]
    EmptyName,
    #[error("duplicate element id: {0}")]
    DuplicateId(String),
    #[error("relationship references unknown element: {0}")]
    UnknownElement(String),
    /// A relationship references a port id that does not exist on the source or destination element.
    #[error("relationship references unknown port: {0}")]
    UnknownPort(String),
    /// An element or relationship references a milestone name not declared in workspace.milestones.
    #[error("unknown milestone referenced: {0}")]
    UnknownMilestone(String),
}

impl ValidationError {
    /// Stable machine-readable code for this error kind.
    pub fn code(&self) -> &'static str {
        match self {
            ValidationError::EmptyName => "empty-name",
            ValidationError::DuplicateId(_) => "duplicate-id",
            ValidationError::UnknownElement(_) => "unknown-element",
            ValidationError::UnknownPort(_) => "unknown-port",
            ValidationError::UnknownMilestone(_) => "unknown-milestone",
        }
    }
}

fn check_milestone(
    errors: &mut Vec<ValidationError>,
    milestone_names: &HashSet<&str>,
    value: Option<&String>,
    what: &str,
    id: &str,
) {
    if let Some(ms) = value {
        if !milestone_names.contains(ms.as_str()) {
            errors.push(ValidationError::UnknownMilestone(format!(
                "unknown milestone '{}' on {} '{}'",
                ms, what, id
            )));
        }
    }
}

fn add_ports<'a>(
    port_map: &mut HashMap<&'a str, HashSet<&'a str>>,
    element_id: &'a str,
    ports: &'a Option<Vec<Port>>,
) {
    if let Some(ports) = ports {
        port_map
            .entry(element_id)
            .or_default()
            .extend(ports.iter().map(|p| p.id.as_str()));
    }
}

/// Validate a workspace, returning a list of validation errors.
pub fn validate(workspace: &Workspace) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    if workspace.name.trim().is_empty() {
        errors.push(ValidationError::EmptyName);
    }

    // Milestone name set (empty when no milestones declared).
    let milestone_names: HashSet<&str> = workspace
        .milestones
        .as_ref()
        .map(|ms| ms.iter().map(|m| m.name.as_str()).collect())
        .unwrap_or_default();

    // Walk the element hierarchy once, collecting ids, ports and relationships
    // and checking element milestone references along the way.
    let mut port_map: HashMap<&str, HashSet<&str>> = HashMap::new();
    let mut all_rels: Vec<&Relationship> = Vec::new();
    let mut element_ids: HashSet<&str> = HashSet::new();

    if let Some(people) = &workspace.model.people {
        for person in people {
            element_ids.insert(&person.id);
            add_ports(&mut port_map, &person.id, &person.ports);
            all_rels.extend(person.relationships.iter().flatten());
            check_milestone(&mut errors, &milestone_names, person.introduced.as_ref(), "person", &person.id);
            check_milestone(&mut errors, &milestone_names, person.retired.as_ref(), "person", &person.id);
        }
    }

    if let Some(systems) = &workspace.model.software_systems {
        for system in systems {
            element_ids.insert(&system.id);
            add_ports(&mut port_map, &system.id, &system.ports);
            all_rels.extend(system.relationships.iter().flatten());
            check_milestone(&mut errors, &milestone_names, system.introduced.as_ref(), "software system", &system.id);
            check_milestone(&mut errors, &milestone_names, system.retired.as_ref(), "software system", &system.id);
            for container in system.containers.iter().flatten() {
                element_ids.insert(&container.id);
                add_ports(&mut port_map, &container.id, &container.ports);
                all_rels.extend(container.relationships.iter().flatten());
                check_milestone(&mut errors, &milestone_names, container.introduced.as_ref(), "container", &container.id);
                check_milestone(&mut errors, &milestone_names, container.retired.as_ref(), "container", &container.id);
                for component in container.components.iter().flatten() {
                    element_ids.insert(&component.id);
                    add_ports(&mut port_map, &component.id, &component.ports);
                    all_rels.extend(component.relationships.iter().flatten());
                    check_milestone(&mut errors, &milestone_names, component.introduced.as_ref(), "component", &component.id);
                    check_milestone(&mut errors, &milestone_names, component.retired.as_ref(), "component", &component.id);
                }
            }
        }
    }

    if let Some(custom) = &workspace.model.custom_elements {
        for elem in custom {
            element_ids.insert(&elem.id);
            add_ports(&mut port_map, &elem.id, &elem.ports);
            all_rels.extend(elem.relationships.iter().flatten());
            check_milestone(&mut errors, &milestone_names, elem.introduced.as_ref(), "custom element", &elem.id);
            check_milestone(&mut errors, &milestone_names, elem.retired.as_ref(), "custom element", &elem.id);
        }
    }

    fn walk_deployment_node<'a>(
        node: &'a DeploymentNode,
        element_ids: &mut HashSet<&'a str>,
        all_rels: &mut Vec<&'a Relationship>,
    ) {
        element_ids.insert(&node.id);
        all_rels.extend(node.relationships.iter().flatten());
        for ci in node.container_instances.iter().flatten() {
            element_ids.insert(&ci.id);
            all_rels.extend(ci.relationships.iter().flatten());
        }
        for ssi in node.software_system_instances.iter().flatten() {
            element_ids.insert(&ssi.id);
            all_rels.extend(ssi.relationships.iter().flatten());
        }
        for inf in node.infrastructure_nodes.iter().flatten() {
            element_ids.insert(&inf.id);
            all_rels.extend(inf.relationships.iter().flatten());
        }
        for child in node.children.iter().flatten() {
            walk_deployment_node(child, element_ids, all_rels);
        }
    }
    for node in workspace.model.deployment_nodes.iter().flatten() {
        walk_deployment_node(node, &mut element_ids, &mut all_rels);
    }

    // Check all relationships for endpoint existence, port validity and
    // milestone references.
    for rel in &all_rels {
        for (element_id, end) in [(&rel.source_id, "source"), (&rel.destination_id, "destination")] {
            if !element_ids.contains(element_id.as_str()) {
                errors.push(ValidationError::UnknownElement(format!(
                    "{} '{}' of relationship '{}' does not exist in the model",
                    end, element_id, rel.id
                )));
            }
        }
        for (port_id, element_id, end) in [
            (&rel.source_port_id, &rel.source_id, "source"),
            (&rel.destination_port_id, &rel.destination_id, "destination"),
        ] {
            if let Some(port_id) = port_id {
                let valid = port_map
                    .get(element_id.as_str())
                    .is_some_and(|ports| ports.contains(port_id.as_str()));
                if !valid {
                    errors.push(ValidationError::UnknownPort(format!(
                        "port '{}' not found on {} element '{}' (relationship '{}')",
                        port_id, end, element_id, rel.id
                    )));
                }
            }
        }
        check_milestone(&mut errors, &milestone_names, rel.introduced.as_ref(), "relationship", &rel.id);
        check_milestone(&mut errors, &milestone_names, rel.retired.as_ref(), "relationship", &rel.id);
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Container, Milestone, Model, Person, Port, PortDirection, Relationship,
        RelationshipKind, SoftwareSystem, Status, Workspace,
    };

    fn make_rel(id: &str, src: &str, dst: &str) -> Relationship {
        Relationship {
            id: id.to_string(),
            source_id: src.to_string(),
            destination_id: dst.to_string(),
            ..Default::default()
        }
    }

    /// Workspace with a source element having a port, a destination element, and a
    /// relationship that references a nonexistent port → UnknownPort error.
    #[test]
    fn unknown_port_yields_error() {
        let mut rel = make_rel("r1", "sys1", "sys2");
        rel.source_port_id = Some("nonexistent-port".to_string());

        let mut ws = Workspace::default();
        ws.name = "Test".to_string();
        ws.model = Model {
            software_systems: Some(vec![
                SoftwareSystem {
                    id: "sys1".to_string(),
                    name: "System 1".to_string(),
                    relationships: Some(vec![rel]),
                    // no ports defined
                    ..Default::default()
                },
                SoftwareSystem {
                    id: "sys2".to_string(),
                    name: "System 2".to_string(),
                    ..Default::default()
                },
            ]),
            ..Default::default()
        };

        let errs = validate(&ws);
        assert!(
            errs.iter().any(|e| matches!(e, ValidationError::UnknownPort(_))),
            "expected UnknownPort error, got: {:?}",
            errs
        );
    }

    /// Element with `introduced` naming an undeclared milestone → UnknownMilestone error.
    #[test]
    fn unknown_milestone_on_element_yields_error() {
        let mut ws = Workspace::default();
        ws.name = "Test".to_string();
        // No milestones declared.
        ws.model = Model {
            people: Some(vec![Person {
                id: "p1".to_string(),
                name: "Alice".to_string(),
                introduced: Some("v1.0".to_string()),
                ..Default::default()
            }]),
            ..Default::default()
        };

        let errs = validate(&ws);
        assert!(
            errs.iter().any(|e| matches!(e, ValidationError::UnknownMilestone(_))),
            "expected UnknownMilestone error, got: {:?}",
            errs
        );
    }

    /// A fully valid workspace with ports and milestones declared correctly → no errors.
    #[test]
    fn valid_workspace_with_ports_and_milestones_no_errors() {
        let port = Port {
            id: "port-a".to_string(),
            name: "Port A".to_string(),
            direction: Some(PortDirection::In),
            ..Default::default()
        };

        let mut rel = make_rel("r1", "c1", "p1");
        rel.destination_port_id = Some("port-a".to_string()); // port on destination person p1
        rel.introduced = Some("v1.0".to_string());
        rel.kind = Some(RelationshipKind::Async);
        rel.status = Some(Status::Draft);

        let mut ws = Workspace::default();
        ws.name = "Valid".to_string();
        ws.milestones = Some(vec![Milestone {
            name: "v1.0".to_string(),
            date: Some("2026-01-01".to_string()),
            description: None,
        }]);

        ws.model = Model {
            people: Some(vec![Person {
                id: "p1".to_string(),
                name: "Alice".to_string(),
                ports: Some(vec![port]),
                ..Default::default()
            }]),
            software_systems: Some(vec![SoftwareSystem {
                id: "sys1".to_string(),
                name: "Sys".to_string(),
                containers: Some(vec![Container {
                    id: "c1".to_string(),
                    name: "Container".to_string(),
                    relationships: Some(vec![rel]),
                    ..Default::default()
                }]),
                ..Default::default()
            }]),
            ..Default::default()
        };

        let errs = validate(&ws);
        assert!(
            errs.is_empty(),
            "expected no errors for valid workspace, got: {:?}",
            errs
        );
    }
}
