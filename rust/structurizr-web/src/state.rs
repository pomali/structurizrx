//! Shared application state.

use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::resolver::WorkspaceEntry;

/// Serialisable summary of a workspace for the index page / API.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WorkspaceSummary {
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub diagram_count: usize,
    pub decision_count: usize,
}

impl From<&WorkspaceEntry> for WorkspaceSummary {
    fn from(e: &WorkspaceEntry) -> Self {
        let diagram_count = count_diagrams(&e.workspace);
        let decision_count = count_decisions(&e.workspace);
        WorkspaceSummary {
            name: e.name.clone(),
            display_name: e.display_name.clone(),
            description: e.workspace.description.clone(),
            diagram_count,
            decision_count,
        }
    }
}

fn count_decisions(ws: &structurizr_model::Workspace) -> usize {
    ws.documentation
        .as_ref()
        .and_then(|d| d.decisions.as_ref())
        .map_or(0, |d| d.len())
}

fn count_diagrams(ws: &structurizr_model::Workspace) -> usize {
    let v = &ws.views;
    let mut n = 0;
    n += v.system_landscape_views.as_ref().map_or(0, |x| x.len());
    n += v.system_context_views.as_ref().map_or(0, |x| x.len());
    n += v.container_views.as_ref().map_or(0, |x| x.len());
    n += v.component_views.as_ref().map_or(0, |x| x.len());
    n += v.dynamic_views.as_ref().map_or(0, |x| x.len());
    n += v.deployment_views.as_ref().map_or(0, |x| x.len());
    n += v.filtered_views.as_ref().map_or(0, |x| x.len());
    n += v.image_views.as_ref().map_or(0, |x| x.len());
    n += v.custom_views.as_ref().map_or(0, |x| x.len());
    n
}

/// Message broadcast to all WebSocket clients.
#[derive(Clone, Debug)]
pub enum BroadcastMsg {
    Reload,
}

/// Shared application state (wrapped in `Arc` for clone-ability).
#[derive(Clone)]
pub struct AppState {
    pub workspaces: Arc<Mutex<Vec<WorkspaceEntry>>>,
    pub tx: broadcast::Sender<BroadcastMsg>,
}

impl AppState {
    pub fn new(workspaces: Vec<WorkspaceEntry>) -> Self {
        let (tx, _) = broadcast::channel(64);
        AppState {
            workspaces: Arc::new(Mutex::new(workspaces)),
            tx,
        }
    }
}

