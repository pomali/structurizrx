use structurizr_model::Workspace;

use crate::Diagram;

/// Trait for diagram exporters.
pub trait DiagramExporter {
    fn export_workspace(&self, workspace: &Workspace) -> Vec<Diagram>;
}
