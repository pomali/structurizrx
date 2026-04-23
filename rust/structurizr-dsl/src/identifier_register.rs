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
}
