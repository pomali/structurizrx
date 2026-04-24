use std::collections::HashMap;

/// Type of an identifier's element.
#[derive(Debug, Clone, PartialEq)]
pub enum ElementType {
    Person,
    SoftwareSystem,
    Container,
    Component,
    DeploymentNode,
    ContainerInstance,
    SoftwareSystemInstance,
    InfrastructureNode,
    CustomElement,
    DeploymentEnvironment,
    Group,
}

/// Registry of identifier → element id mappings.
#[derive(Debug, Clone, Default)]
pub struct IdentifierRegister {
    pub identifiers: HashMap<String, (String, ElementType)>,
    pub mode: IdentifierMode,
}

/// Whether identifiers are hierarchical or flat.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum IdentifierMode {
    #[default]
    Flat,
    Hierarchical,
}

impl IdentifierRegister {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, identifier: &str, id: String, kind: ElementType) {
        self.identifiers.insert(identifier.to_lowercase(), (id, kind));
    }

    pub fn resolve(&self, identifier: &str) -> Option<&(String, ElementType)> {
        self.identifiers.get(&identifier.to_lowercase())
    }

    pub fn resolve_id(&self, identifier: &str) -> Option<String> {
        self.identifiers
            .get(&identifier.to_lowercase())
            .map(|(id, _)| id.clone())
    }

    /// Return all element IDs whose registered identifier has `prefix.` as a prefix.
    /// Used to expand group identifiers to their children in hierarchical mode.
    pub fn children_of(&self, prefix: &str) -> Vec<String> {
        let lower_prefix = format!("{}.", prefix.to_lowercase());
        self.identifiers
            .iter()
            .filter(|(k, _)| k.starts_with(&lower_prefix))
            .map(|(_, (id, _))| id.clone())
            .collect()
    }
}
