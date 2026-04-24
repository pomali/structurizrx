//! File system watcher that triggers workspace reloads on changes.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};

use crate::resolver;
use crate::state::{AppState, BroadcastMsg};

/// Start watching `path` recursively.
///
/// On any file-system event the workspaces are re-resolved from `path` and
/// stored in `state`.  A [`BroadcastMsg::Reload`] is then sent to all
/// connected WebSocket clients.
pub fn start(path: PathBuf, state: AppState) -> Result<()> {
    let (tx, rx) = mpsc::channel();

    let mut watcher = RecommendedWatcher::new(tx, Config::default().with_poll_interval(Duration::from_secs(1)))?;
    watcher.watch(&path, RecursiveMode::Recursive)?;

    // Run in a dedicated OS thread so we don't block the tokio runtime.
    std::thread::spawn(move || {
        // Keep `watcher` alive for the lifetime of this thread.
        let _watcher = watcher;

        // Debounce: collect events until 300 ms of silence.
        while rx.recv().is_ok() {
            // Drain additional events that arrive within 300 ms
            while rx.recv_timeout(Duration::from_millis(300)).is_ok() {}

            // Re-resolve workspaces
            match resolver::resolve(&path) {
                Ok(entries) => {
                    if let Ok(mut ws) = state.workspaces.lock() {
                        *ws = entries;
                    }
                }
                Err(e) => {
                    eprintln!("Warning: failed to reload workspaces: {}", e);
                }
            }

            // Notify all WS clients
            let _ = state.tx.send(BroadcastMsg::Reload);
        }
    });

    Ok(())
}

