//! `structurizr-lsp-wasm` — the Structurizr DSL language server, compiled to
//! WebAssembly.
//!
//! Editors normally spawn `structurizrx lsp` as a subprocess. That needs the
//! binary installed; this crate is the alternative, letting a host (the VS
//! Code extension) run the same server in-process with no native dependency.
//!
//! There is no transport here — the host owns the message loop and pumps
//! decoded JSON-RPC messages through [`LspServer::handle`]:
//!
//! ```js
//! import init, { LspServer } from './structurizr_lsp_wasm.js';
//!
//! await init();
//! const server = new LspServer();
//! for (const outgoing of JSON.parse(server.handle(JSON.stringify(message)))) {
//!     client.send(JSON.parse(outgoing));
//! }
//! ```

use structurizr_lsp::jsonrpc::Dispatcher;
use wasm_bindgen::prelude::*;

/// A language server instance holding the open documents. Create one per
/// client session.
#[wasm_bindgen]
pub struct LspServer {
    dispatcher: Dispatcher,
}

impl Default for LspServer {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl LspServer {
    #[wasm_bindgen(constructor)]
    pub fn new() -> LspServer {
        LspServer {
            dispatcher: Dispatcher::new(),
        }
    }

    /// Handles one JSON-RPC message and returns the messages to send back to
    /// the client, as a JSON array of serialized objects (`[]` when the
    /// message needs no reply).
    pub fn handle(&self, message: &str) -> String {
        let outgoing = self.dispatcher.handle(message);
        // The elements are already serialized JSON, so splice them together
        // rather than re-encoding them as strings-in-a-string.
        format!("[{}]", outgoing.join(","))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    #[test]
    fn handle_returns_a_json_array_of_messages() {
        let server = LspServer::new();

        let empty = server.handle(&json!({"jsonrpc": "2.0", "method": "initialized"}).to_string());
        assert_eq!(empty, "[]");

        let out = server.handle(
            &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"capabilities": {}}})
                .to_string(),
        );
        let messages: Vec<Value> = serde_json::from_str(&out).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["id"], 1);
        assert!(messages[0]["result"]["capabilities"].is_object());
    }

    #[test]
    fn documents_persist_across_calls() {
        let server = LspServer::new();
        server.handle(
            &json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": { "textDocument": {
                    "uri": "file:///w.dsl",
                    "languageId": "structurizr-dsl",
                    "version": 1,
                    "text": "workspace {\n  model {\n    u = person \"User\"\n  }\n}\n",
                }}
            })
            .to_string(),
        );
        let out = server.handle(
            &json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "textDocument/documentSymbol",
                "params": { "textDocument": { "uri": "file:///w.dsl" } }
            })
            .to_string(),
        );
        let messages: Vec<Value> = serde_json::from_str(&out).unwrap();
        assert_eq!(messages[0]["result"][0]["name"], "User");
    }
}
