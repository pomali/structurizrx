//! Language Server for the Structurizr DSL, built on the existing
//! `structurizr-dsl` lexer/parser rather than a separate grammar.

mod backend;
mod convert;
mod diagnostics;
mod document;
mod index;

use backend::Backend;
use tower_lsp_server::{LspService, Server};

/// Runs the language server over stdio. Intended to be spawned by an editor
/// (e.g. `structurizrx lsp`), not run interactively.
pub async fn run_stdio() -> anyhow::Result<()> {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
    Ok(())
}
