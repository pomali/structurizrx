use thiserror::Error;

use crate::Workspace;

/// Validation error.
#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("workspace name is empty")]
    EmptyName,
    #[error("duplicate element id: {0}")]
    DuplicateId(String),
    #[error("relationship references unknown element: {0}")]
    UnknownElement(String),
}

/// Validate a workspace, returning a list of validation errors.
pub fn validate(workspace: &Workspace) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    if workspace.name.trim().is_empty() {
        errors.push(ValidationError::EmptyName);
    }

    errors
}
