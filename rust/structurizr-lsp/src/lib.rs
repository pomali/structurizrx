//! Language Server for the Structurizr DSL, built on the existing
//! `structurizr-dsl` lexer/parser rather than a separate grammar.
//!
//! The logic lives in [`core::Core`], which is synchronous and
//! runtime-agnostic. Two front ends drive it: [`run_stdio`] (tower-lsp over
//! stdin/stdout, behind the default `stdio` feature) and
//! [`jsonrpc::Dispatcher`] (message in → messages out), which the
//! `structurizr-lsp-wasm` crate exposes to JavaScript.

#[cfg(feature = "stdio")]
mod backend;
mod convert;
pub mod core;
mod diagnostics;
mod document;
mod index;
pub mod jsonrpc;

/// Runs the language server over stdio. Intended to be spawned by an editor
/// (e.g. `structurizrx lsp`), not run interactively.
#[cfg(feature = "stdio")]
pub async fn run_stdio() -> anyhow::Result<()> {
    use backend::Backend;
    use tower_lsp_server::{LspService, Server};

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
    Ok(())
}
