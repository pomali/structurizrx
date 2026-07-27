//! A minimal, synchronous JSON-RPC dispatcher over [`Core`].
//!
//! The stdio server gets its JSON-RPC plumbing from tower-lsp, but tower-lsp
//! is built on tokio and doesn't run on `wasm32-unknown-unknown`. This module
//! is the equivalent for the WASM build: feed it one decoded LSP message at a
//! time, get back the messages to send to the client.

use ls_types::*;
use serde_json::{json, Value};

use crate::core::Core;

/// Dispatches LSP messages against a single [`Core`].
#[derive(Default)]
pub struct Dispatcher {
    core: Core,
}

impl Dispatcher {
    pub fn new() -> Self {
        Self::default()
    }

    /// Handles one incoming JSON-RPC message and returns the messages to send
    /// back, each as a serialized JSON object. A notification usually produces
    /// nothing; document syncs produce a `textDocument/publishDiagnostics`
    /// notification; requests produce exactly one response.
    pub fn handle(&self, message: &str) -> Vec<String> {
        let Ok(value) = serde_json::from_str::<Value>(message) else {
            return vec![error_response(
                Value::Null,
                PARSE_ERROR,
                "invalid JSON-RPC message",
            )];
        };
        let method = value.get("method").and_then(Value::as_str).unwrap_or("");
        let params = value.get("params").cloned().unwrap_or(Value::Null);
        // Responses to server->client requests come back with no `method`; we
        // never send such requests, so there is nothing to correlate them to.
        if method.is_empty() {
            return Vec::new();
        }

        match value.get("id").cloned() {
            Some(id) => vec![self.request(id, method, params)],
            None => self.notification(method, params),
        }
    }

    fn request(&self, id: Value, method: &str, params: Value) -> String {
        let result = match method {
            "initialize" => Ok(json!(Core::initialize_result())),
            "shutdown" => Ok(Value::Null),
            "textDocument/hover" => decode::<HoverParams>(params).map(|p| {
                let p = p.text_document_position_params;
                json!(self.core.hover(&p.text_document.uri, p.position))
            }),
            "textDocument/completion" => decode::<CompletionParams>(params).map(|p| {
                let uri = p.text_document_position.text_document.uri;
                json!(self.core.completion(&uri))
            }),
            "textDocument/definition" => decode::<GotoDefinitionParams>(params).map(|p| {
                let p = p.text_document_position_params;
                json!(self.core.goto_definition(&p.text_document.uri, p.position))
            }),
            "textDocument/documentSymbol" => decode::<DocumentSymbolParams>(params)
                .map(|p| json!(self.core.document_symbol(&p.text_document.uri))),
            _ => {
                return error_response(
                    id,
                    METHOD_NOT_FOUND,
                    &format!("unsupported method: {method}"),
                )
            }
        };

        match result {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string(),
            Err(err) => error_response(id, INVALID_PARAMS, &err),
        }
    }

    fn notification(&self, method: &str, params: Value) -> Vec<String> {
        match method {
            "textDocument/didOpen" => {
                let Ok(p) = decode::<DidOpenTextDocumentParams>(params) else {
                    return Vec::new();
                };
                let uri = p.text_document.uri;
                let diags = self.core.set_document(uri.clone(), p.text_document.text);
                vec![publish_diagnostics(&uri, diags)]
            }
            "textDocument/didChange" => {
                let Ok(mut p) = decode::<DidChangeTextDocumentParams>(params) else {
                    return Vec::new();
                };
                let Some(change) = p.content_changes.pop() else {
                    return Vec::new();
                };
                let uri = p.text_document.uri;
                let diags = self.core.set_document(uri.clone(), change.text);
                vec![publish_diagnostics(&uri, diags)]
            }
            "textDocument/didClose" => {
                if let Ok(p) = decode::<DidCloseTextDocumentParams>(params) {
                    self.core.close_document(&p.text_document.uri);
                }
                Vec::new()
            }
            // `initialized`, `exit`, `$/cancelRequest`, `$/setTrace`, …
            _ => Vec::new(),
        }
    }
}

const PARSE_ERROR: i32 = -32700;
const METHOD_NOT_FOUND: i32 = -32601;
const INVALID_PARAMS: i32 = -32602;

fn decode<T: serde::de::DeserializeOwned>(params: Value) -> Result<T, String> {
    serde_json::from_value(params).map_err(|e| e.to_string())
}

fn error_response(id: Value, code: i32, message: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
    .to_string()
}

fn publish_diagnostics(uri: &Uri, diagnostics: Vec<Diagnostic>) -> String {
    json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": { "uri": uri, "diagnostics": diagnostics },
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DSL: &str = "workspace {\n  model {\n    u = person \"User\"\n  }\n}\n";

    fn open(dispatcher: &Dispatcher, text: &str) -> Vec<String> {
        dispatcher.handle(
            &json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": "file:///w.dsl",
                        "languageId": "structurizr-dsl",
                        "version": 1,
                        "text": text,
                    }
                }
            })
            .to_string(),
        )
    }

    #[test]
    fn initialize_advertises_capabilities() {
        let out = Dispatcher::new()
            .handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"capabilities": {}}}).to_string());
        let response: Value = serde_json::from_str(&out[0]).unwrap();
        assert_eq!(response["id"], 1);
        assert!(response["result"]["capabilities"]["hoverProvider"] == true);
    }

    #[test]
    fn did_open_publishes_diagnostics() {
        let dispatcher = Dispatcher::new();
        let out = open(&dispatcher, DSL);
        let notification: Value = serde_json::from_str(&out[0]).unwrap();
        assert_eq!(notification["method"], "textDocument/publishDiagnostics");
        assert_eq!(notification["params"]["diagnostics"], json!([]));
    }

    #[test]
    fn syntax_error_reports_a_diagnostic() {
        let dispatcher = Dispatcher::new();
        let out = open(&dispatcher, "workspace {\n  model {\n");
        let notification: Value = serde_json::from_str(&out[0]).unwrap();
        assert_eq!(
            notification["params"]["diagnostics"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn hover_resolves_an_identifier() {
        let dispatcher = Dispatcher::new();
        open(&dispatcher, DSL);
        let out = dispatcher.handle(
            &json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "textDocument/hover",
                "params": {
                    "textDocument": { "uri": "file:///w.dsl" },
                    "position": { "line": 2, "character": 4 },
                }
            })
            .to_string(),
        );
        let response: Value = serde_json::from_str(&out[0]).unwrap();
        let value = response["result"]["contents"]["value"].as_str().unwrap();
        assert!(value.contains("Person"), "unexpected hover: {value}");
    }

    #[test]
    fn document_symbols_list_the_model() {
        let dispatcher = Dispatcher::new();
        open(&dispatcher, DSL);
        let out = dispatcher.handle(
            &json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "textDocument/documentSymbol",
                "params": { "textDocument": { "uri": "file:///w.dsl" } }
            })
            .to_string(),
        );
        let response: Value = serde_json::from_str(&out[0]).unwrap();
        assert_eq!(response["result"][0]["name"], "User");
    }

    #[test]
    fn unknown_request_gets_a_method_not_found_error() {
        let out = Dispatcher::new()
            .handle(&json!({"jsonrpc": "2.0", "id": 9, "method": "textDocument/formatting"}).to_string());
        let response: Value = serde_json::from_str(&out[0]).unwrap();
        assert_eq!(response["error"]["code"], METHOD_NOT_FOUND);
    }

    #[test]
    fn unknown_notification_is_silently_ignored() {
        assert!(Dispatcher::new()
            .handle(&json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}).to_string())
            .is_empty());
    }
}
