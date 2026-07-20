//! The `LanguageServer` implementation: wires LSP notifications/requests to
//! `DocumentState` and formats responses.

use std::collections::HashMap;

use dashmap::DashMap;
use structurizr_dsl::lexer::Pos;
use structurizr_model::Workspace;
use tower_lsp_server::jsonrpc::Result as RpcResult;
use tower_lsp_server::ls_types::*;
use tower_lsp_server::{Client, LanguageServer};

use crate::convert::{point_range, position_to_pos};
use crate::document::DocumentState;

pub struct Backend {
    client: Client,
    documents: DashMap<Uri, DocumentState>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: DashMap::new(),
        }
    }

    async fn republish(&self, uri: Uri, diagnostics: Vec<Diagnostic>) {
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }
}

impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> RpcResult<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "structurizr-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions::default()),
                definition_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                ..ServerCapabilities::default()
            },
            ..InitializeResult::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "structurizr-lsp ready")
            .await;
    }

    async fn shutdown(&self) -> RpcResult<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let diags = self
            .documents
            .entry(uri.clone())
            .or_insert_with(DocumentState::empty)
            .update(params.text_document.text);
        self.republish(uri, diags).await;
    }

    async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
        let Some(change) = params.content_changes.pop() else {
            return;
        };
        let uri = params.text_document.uri;
        let diags = self
            .documents
            .entry(uri.clone())
            .or_insert_with(DocumentState::empty)
            .update(change.text);
        self.republish(uri, diags).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.documents.remove(&params.text_document.uri);
    }

    async fn hover(&self, params: HoverParams) -> RpcResult<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = position_to_pos(params.text_document_position_params.position);
        let Some(doc) = self.documents.get(&uri) else {
            return Ok(None);
        };
        let Some(word) = doc.word_at(pos) else {
            return Ok(None);
        };
        let Some(analyzed) = doc.last_ok.as_ref() else {
            return Ok(None);
        };
        let Some((id, _kind)) = analyzed.identifiers.resolve(word) else {
            return Ok(None);
        };
        let Some(markdown) = hover_markdown(&analyzed.workspace, id) else {
            return Ok(None);
        };
        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: markdown,
            }),
            range: None,
        }))
    }

    async fn completion(&self, params: CompletionParams) -> RpcResult<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let mut items = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for &(_, keywords) in structurizr_dsl::keyword_sets() {
            for &kw in keywords {
                if seen.insert(kw) {
                    items.push(CompletionItem {
                        label: kw.to_string(),
                        kind: Some(CompletionItemKind::KEYWORD),
                        ..CompletionItem::default()
                    });
                }
            }
        }
        if let Some(doc) = self.documents.get(&uri) {
            if let Some(analyzed) = &doc.last_ok {
                for (ident, (id, kind)) in &analyzed.identifiers.identifiers {
                    items.push(CompletionItem {
                        label: ident.clone(),
                        kind: Some(CompletionItemKind::VARIABLE),
                        detail: Some(format!("{:?} ({})", kind, id)),
                        ..CompletionItem::default()
                    });
                }
            }
        }
        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> RpcResult<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = position_to_pos(params.text_document_position_params.position);
        let Some(doc) = self.documents.get(&uri) else {
            return Ok(None);
        };
        let Some(word) = doc.word_at(pos) else {
            return Ok(None);
        };
        let Some(&decl_pos) = doc.declarations.get(&word.to_lowercase()) else {
            return Ok(None);
        };
        let range = point_range(decl_pos, word.chars().count());
        Ok(Some(GotoDefinitionResponse::Scalar(Location {
            uri,
            range,
        })))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> RpcResult<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let Some(doc) = self.documents.get(&uri) else {
            return Ok(None);
        };
        let Some(analyzed) = doc.last_ok.as_ref() else {
            return Ok(None);
        };
        Ok(Some(DocumentSymbolResponse::Nested(build_symbols(
            &analyzed.workspace,
            &analyzed.id_to_pos,
        ))))
    }
}

/// Finds the element with the given id anywhere in the model tree and
/// formats a Markdown hover for it.
fn hover_markdown(workspace: &Workspace, id: &str) -> Option<String> {
    for p in workspace.model.people.iter().flatten() {
        if p.id == id {
            return Some(format_hover(
                "Person",
                &p.name,
                p.description.as_deref(),
                None,
                p.tags.as_deref(),
            ));
        }
    }
    for s in workspace.model.software_systems.iter().flatten() {
        if s.id == id {
            return Some(format_hover(
                "Software System",
                &s.name,
                s.description.as_deref(),
                None,
                s.tags.as_deref(),
            ));
        }
        for c in s.containers.iter().flatten() {
            if c.id == id {
                return Some(format_hover(
                    "Container",
                    &c.name,
                    c.description.as_deref(),
                    c.technology.as_deref(),
                    c.tags.as_deref(),
                ));
            }
            for comp in c.components.iter().flatten() {
                if comp.id == id {
                    return Some(format_hover(
                        "Component",
                        &comp.name,
                        comp.description.as_deref(),
                        comp.technology.as_deref(),
                        comp.tags.as_deref(),
                    ));
                }
            }
        }
    }
    None
}

fn format_hover(
    kind: &str,
    name: &str,
    description: Option<&str>,
    technology: Option<&str>,
    tags: Option<&str>,
) -> String {
    let mut md = format!("**{}**: {}", kind, name);
    if let Some(t) = technology {
        md.push_str(&format!("  \n_{}_", t));
    }
    if let Some(d) = description {
        md.push_str(&format!("\n\n{}", d));
    }
    if let Some(t) = tags {
        md.push_str(&format!("\n\ntags: `{}`", t));
    }
    md
}

/// `DocumentSymbol.deprecated` is a deprecated field we still have to set
/// (no `Default` impl on `DocumentSymbol`).
#[allow(deprecated)]
fn make_symbol(
    name: &str,
    kind: SymbolKind,
    pos: Option<Pos>,
    children: Vec<DocumentSymbol>,
) -> Option<DocumentSymbol> {
    let range = point_range(pos?, 1);
    Some(DocumentSymbol {
        name: name.to_string(),
        detail: None,
        kind,
        tags: None,
        deprecated: None,
        range,
        selection_range: range,
        children: (!children.is_empty()).then_some(children),
    })
}

fn build_symbols(workspace: &Workspace, id_to_pos: &HashMap<String, Pos>) -> Vec<DocumentSymbol> {
    let mut out = Vec::new();
    for p in workspace.model.people.iter().flatten() {
        out.extend(make_symbol(
            &p.name,
            SymbolKind::OBJECT,
            id_to_pos.get(&p.id).copied(),
            vec![],
        ));
    }
    for s in workspace.model.software_systems.iter().flatten() {
        let mut containers = Vec::new();
        for c in s.containers.iter().flatten() {
            let mut components = Vec::new();
            for comp in c.components.iter().flatten() {
                components.extend(make_symbol(
                    &comp.name,
                    SymbolKind::STRUCT,
                    id_to_pos.get(&comp.id).copied(),
                    vec![],
                ));
            }
            containers.extend(make_symbol(
                &c.name,
                SymbolKind::CLASS,
                id_to_pos.get(&c.id).copied(),
                components,
            ));
        }
        out.extend(make_symbol(
            &s.name,
            SymbolKind::MODULE,
            id_to_pos.get(&s.id).copied(),
            containers,
        ));
    }
    out
}
