//! `structurizr-web` — local HTTP server for browsing Structurizr workspaces.
//!
//! # Usage
//!
//! ```no_run
//! use std::path::PathBuf;
//! use structurizr_web::ServeOptions;
//!
//! #[tokio::main]
//! async fn main() {
//!     let opts = ServeOptions {
//!         path: PathBuf::from("."),
//!         port: 3000,
//!         open_browser: false,
//!     };
//!     structurizr_web::serve(opts).await.unwrap();
//! }
//! ```

pub mod assets;
pub mod resolver;
pub mod server;
pub mod state;
pub mod watcher;

use std::path::PathBuf;

use anyhow::Result;

/// Options for [`serve`].
pub struct ServeOptions {
    /// Path to a workspace file or directory.
    pub path: PathBuf,
    /// TCP port to listen on.
    pub port: u16,
    /// Open the browser after binding.
    pub open_browser: bool,
}

/// Resolve workspaces, start the file watcher, and serve the web UI.
///
/// This function runs until the process is killed.
pub async fn serve(opts: ServeOptions) -> Result<()> {
    let path = opts.path.canonicalize().unwrap_or(opts.path.clone());

    // Initial workspace load
    let workspaces = resolver::resolve(&path)?;
    println!(
        "Found {} workspace(s):",
        workspaces.len()
    );
    for w in &workspaces {
        println!("  • {} ({})", w.display_name, w.source_path.display());
    }

    let state = state::AppState::new(workspaces);

    // Start file watcher
    watcher::start(path, state.clone())?;

    // Build router
    let app = server::build_router(state);

    let addr = format!("0.0.0.0:{}", opts.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let url = format!("http://localhost:{}", opts.port);
    println!("Serving at {}", url);

    if opts.open_browser {
        let url_clone = url.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            if let Err(e) = open::that(&url_clone) {
                eprintln!("Could not open browser: {}", e);
            }
        });
    }

    axum::serve(listener, app).await?;
    Ok(())
}
