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
                let p = p.text_document_position;
                json!(self.core.completion(&p.text_document.uri, p.position))
            }),
            "textDocument/definition" => decode::<GotoDefinitionParams>(params).map(|p| {
                let p = p.text_document_position_params;
                json!(self.core.goto_definition(&p.text_document.uri, p.position))
            }),
            "textDocument/documentSymbol" => decode::<DocumentSymbolParams>(params)
                .map(|p| json!(self.core.document_symbol(&p.text_document.uri))),
            "textDocument/references" => decode::<ReferenceParams>(params).map(|p| {
                let p = p.text_document_position;
                json!(self.core.references(&p.text_document.uri, p.position))
            }),
            "textDocument/documentHighlight" => decode::<DocumentHighlightParams>(params).map(|p| {
                let p = p.text_document_position_params;
                json!(self
                    .core
                    .document_highlight(&p.text_document.uri, p.position))
            }),
            "textDocument/prepareRename" => decode::<TextDocumentPositionParams>(params)
                .map(|p| json!(self.core.prepare_rename(&p.text_document.uri, p.position))),
            "textDocument/rename" => decode::<RenameParams>(params).map(|p| {
                let new_name = p.new_name;
                let p = p.text_document_position;
                json!(self.core.rename(&p.text_document.uri, p.position, &new_name))
            }),
            "textDocument/semanticTokens/full" => decode::<SemanticTokensParams>(params)
                .map(|p| json!(self.core.semantic_tokens(&p.text_document.uri))),
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

    /// `u` is declared on line 3 (LSP line 2, char 4) and used again as the
    /// source of the relationship on line 5.
    const LINKED: &str = "workspace {\n  model {\n    u = person \"User\"\n    s = softwareSystem \"S\"\n    u -> s \"Uses\"\n  }\n}\n";

    fn request(dispatcher: &Dispatcher, method: &str, params: Value) -> Value {
        let out = dispatcher.handle(
            &json!({"jsonrpc": "2.0", "id": 7, "method": method, "params": params}).to_string(),
        );
        serde_json::from_str::<Value>(&out[0]).unwrap()["result"].clone()
    }

    fn at_u() -> Value {
        json!({
            "textDocument": { "uri": "file:///w.dsl" },
            "position": { "line": 2, "character": 4 },
        })
    }

    #[test]
    fn references_include_the_declaration_and_the_relationship() {
        let dispatcher = Dispatcher::new();
        open(&dispatcher, LINKED);
        let mut params = at_u();
        params["context"] = json!({ "includeDeclaration": true });
        let result = request(&dispatcher, "textDocument/references", params);
        let lines: Vec<u64> = result
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["range"]["start"]["line"].as_u64().unwrap())
            .collect();
        assert_eq!(lines, vec![2, 4]);
    }

    #[test]
    fn rename_rewrites_every_occurrence() {
        let dispatcher = Dispatcher::new();
        open(&dispatcher, LINKED);
        let mut params = at_u();
        params["newName"] = json!("customer");
        let result = request(&dispatcher, "textDocument/rename", params);
        let edits = result["changes"]["file:///w.dsl"].as_array().unwrap();
        assert_eq!(edits.len(), 2);
        assert!(edits.iter().all(|e| e["newText"] == "customer"));
    }

    #[test]
    fn rename_refuses_a_keyword() {
        let dispatcher = Dispatcher::new();
        open(&dispatcher, LINKED);
        // `person` on line 3 is a keyword, not a declared identifier.
        let params = json!({
            "textDocument": { "uri": "file:///w.dsl" },
            "position": { "line": 2, "character": 9 },
        });
        assert_eq!(
            request(&dispatcher, "textDocument/prepareRename", params),
            Value::Null
        );
    }

    #[test]
    fn document_highlight_marks_both_occurrences() {
        let dispatcher = Dispatcher::new();
        open(&dispatcher, LINKED);
        let result = request(&dispatcher, "textDocument/documentHighlight", at_u());
        assert_eq!(result.as_array().unwrap().len(), 2);
    }

    #[test]
    fn semantic_tokens_are_returned_as_a_flat_int_array() {
        let dispatcher = Dispatcher::new();
        open(&dispatcher, LINKED);
        let result = request(
            &dispatcher,
            "textDocument/semanticTokens/full",
            json!({ "textDocument": { "uri": "file:///w.dsl" } }),
        );
        let data = result["data"].as_array().unwrap();
        assert!(!data.is_empty());
        assert_eq!(data.len() % 5, 0, "5 ints per token");
    }

    #[test]
    fn completion_is_scoped_to_the_enclosing_block() {
        let dispatcher = Dispatcher::new();
        open(&dispatcher, LINKED);
        // Inside `model { ... }`, on the blank part of the relationship line.
        let result = request(
            &dispatcher,
            "textDocument/completion",
            json!({
                "textDocument": { "uri": "file:///w.dsl" },
                "position": { "line": 3, "character": 4 },
            }),
        );
        let labels: Vec<&str> = result
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["label"].as_str().unwrap())
            .collect();
        assert!(labels.contains(&"person"), "model keyword: {labels:?}");
        assert!(
            !labels.contains(&"systemContext"),
            "views keyword must not leak into model: {labels:?}"
        );
        assert!(labels.contains(&"u"), "declared identifiers: {labels:?}");
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
