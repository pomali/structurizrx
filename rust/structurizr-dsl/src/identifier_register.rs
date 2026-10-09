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
    Relationship,
}

/// Registry of identifier → element id mappings.
#[derive(Debug, Clone, Default)]
pub struct IdentifierRegister {
    pub identifiers: HashMap<String, (String, ElementType)>,
    pub mode: IdentifierMode,
    /// Lowercased identifier -> the identifier exactly as written in the DSL
    /// source. Used by the emitter to keep original casing (`WebApp` rather
    /// than `webapp`) when re-emitting DSL for an identifier the workspace
    /// was originally parsed with.
    pub spellings: HashMap<String, String>,
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
        let lower = identifier.to_lowercase();
        self.spellings.insert(lower.clone(), identifier.to_string());
        self.identifiers.insert(lower, (id, kind));
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
