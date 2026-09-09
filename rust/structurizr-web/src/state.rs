//! Shared application state.

use std::collections::HashMap;
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

/// Per-workspace artefacts derived from the model, cached because they cost a
/// full index pass (and, later, graph algorithms) to produce and are requested
/// far more often than the workspace changes.
///
/// Every entry is keyed by workspace name and dropped wholesale by
/// [`AppState::invalidate_derived`] when the watcher reloads — there is no
/// partial invalidation, because a reload replaces the workspaces outright.
#[derive(Default)]
pub struct DerivedCache {
    /// Serialised body of `/api/workspace/{name}/review`.
    pub review_json: Option<Arc<String>>,
    /// Serialised bodies of `/api/workspace/{name}/diff`, keyed by the pair of
    /// revisions being compared. Building one means reading two blobs out of
    /// git and parsing both, so a page that re-requests the same comparison
    /// (a reload, a second reader) must not pay for it twice.
    pub diff_json: HashMap<String, Arc<String>>,
    /// Serialised bodies of `/api/workspace/{name}/clusters`, keyed by the
    /// analysis options — the same workspace yields a different analysis per
    /// level, tag filter and rollup setting.
    pub cluster_json: HashMap<String, Arc<String>>,
}

/// Shared application state (wrapped in `Arc` for clone-ability).
#[derive(Clone)]
pub struct AppState {
    pub workspaces: Arc<Mutex<Vec<WorkspaceEntry>>>,
    pub tx: broadcast::Sender<BroadcastMsg>,
    derived: Arc<Mutex<HashMap<String, DerivedCache>>>,
}

impl AppState {
    pub fn new(workspaces: Vec<WorkspaceEntry>) -> Self {
        let (tx, _) = broadcast::channel(64);
        AppState {
            workspaces: Arc::new(Mutex::new(workspaces)),
            tx,
            derived: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Read a cached artefact, or produce and store it.
    ///
    /// `produce` runs while no lock is held, so a slow computation for one
    /// workspace never blocks requests for another.
    pub fn cached<R, W, F>(&self, name: &str, read: R, write: W, produce: F) -> Arc<String>
    where
        R: FnOnce(&DerivedCache) -> Option<Arc<String>>,
        W: FnOnce(&mut DerivedCache, Arc<String>),
        F: FnOnce() -> String,
    {
        if let Ok(cache) = self.derived.lock() {
            if let Some(hit) = cache.get(name).and_then(read) {
                return hit;
            }
        }

        let value = Arc::new(produce());

        if let Ok(mut cache) = self.derived.lock() {
            write(cache.entry(name.to_string()).or_default(), value.clone());
        }

        value
    }

    /// Drop every derived artefact. Called by the watcher after it swaps in
    /// freshly parsed workspaces; without it the review and graph views would
    /// keep serving data for the previous version of the file.
    pub fn invalidate_derived(&self) {
        if let Ok(mut cache) = self.derived.lock() {
            cache.clear();
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn a_second_read_is_served_from_the_cache() {
        let state = AppState::new(vec![]);
        let builds = AtomicUsize::new(0);

        let mut produce = || {
            builds.fetch_add(1, Ordering::SeqCst);
            "body".to_string()
        };

        let first = state.cached("ws", |c| c.review_json.clone(), |c, v| c.review_json = Some(v), &mut produce);
        let second = state.cached("ws", |c| c.review_json.clone(), |c, v| c.review_json = Some(v), &mut produce);

        assert_eq!(builds.load(Ordering::SeqCst), 1, "the body is built once");
        assert_eq!(*first, *second);
    }

    #[test]
    fn invalidating_forces_a_rebuild() {
        let state = AppState::new(vec![]);
        let builds = AtomicUsize::new(0);

        let mut produce = || {
            builds.fetch_add(1, Ordering::SeqCst);
            "body".to_string()
        };

        state.cached("ws", |c| c.review_json.clone(), |c, v| c.review_json = Some(v), &mut produce);
        state.invalidate_derived();
        state.cached("ws", |c| c.review_json.clone(), |c, v| c.review_json = Some(v), &mut produce);

        assert_eq!(builds.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn workspaces_do_not_share_a_cache_entry() {
        let state = AppState::new(vec![]);

        let a = state.cached("a", |c| c.review_json.clone(), |c, v| c.review_json = Some(v), || "A".to_string());
        let b = state.cached("b", |c| c.review_json.clone(), |c, v| c.review_json = Some(v), || "B".to_string());

        assert_eq!(*a, "A");
        assert_eq!(*b, "B");
    }
}
