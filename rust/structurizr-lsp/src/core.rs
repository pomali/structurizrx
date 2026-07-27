//! Runtime-agnostic language-server logic.
//!
//! Everything here is synchronous and free of tokio/tower, so the same code
//! backs both the stdio server (`backend.rs`) and the WASM build
//! (`jsonrpc.rs` → `structurizr-lsp-wasm`).

use std::collections::HashMap;
use std::sync::RwLock;

use ls_types::*;
use structurizr_dsl::lexer::Pos;
use structurizr_model::Workspace;

use crate::convert::{point_range, position_to_pos};
use crate::document::DocumentState;

#[derive(Default)]
pub struct Core {
    documents: RwLock<HashMap<Uri, DocumentState>>,
}

impl Core {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn capabilities() -> ServerCapabilities {
        ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            completion_provider: Some(CompletionOptions::default()),
            definition_provider: Some(OneOf::Left(true)),
            document_symbol_provider: Some(OneOf::Left(true)),
            ..ServerCapabilities::default()
        }
    }

    pub fn initialize_result() -> InitializeResult {
        InitializeResult {
            server_info: Some(ServerInfo {
                name: "structurizr-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
            capabilities: Self::capabilities(),
            ..InitializeResult::default()
        }
    }

    /// Parses `text` as the current contents of `uri` and returns the
    /// diagnostics to publish for it. Used for both `didOpen` and `didChange`.
    pub fn set_document(&self, uri: Uri, text: String) -> Vec<Diagnostic> {
        let mut documents = self.documents.write().unwrap();
        documents
            .entry(uri)
            .or_insert_with(DocumentState::empty)
            .update(text)
    }

    pub fn close_document(&self, uri: &Uri) {
        self.documents.write().unwrap().remove(uri);
    }

    pub fn hover(&self, uri: &Uri, position: Position) -> Option<Hover> {
        let documents = self.documents.read().unwrap();
        let doc = documents.get(uri)?;
        let word = doc.word_at(position_to_pos(position))?;
        let analyzed = doc.last_ok.as_ref()?;
        let (id, _kind) = analyzed.identifiers.resolve(word)?;
        let markdown = hover_markdown(&analyzed.workspace, id)?;
        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: markdown,
            }),
            range: None,
        })
    }

    pub fn completion(&self, uri: &Uri) -> Vec<CompletionItem> {
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
        let documents = self.documents.read().unwrap();
        if let Some(analyzed) = documents.get(uri).and_then(|doc| doc.last_ok.as_ref()) {
            for (ident, (id, kind)) in &analyzed.identifiers.identifiers {
                items.push(CompletionItem {
                    label: ident.clone(),
                    kind: Some(CompletionItemKind::VARIABLE),
                    detail: Some(format!("{:?} ({})", kind, id)),
                    ..CompletionItem::default()
                });
            }
        }
        items
    }

    pub fn goto_definition(&self, uri: &Uri, position: Position) -> Option<Location> {
        let documents = self.documents.read().unwrap();
        let doc = documents.get(uri)?;
        let word = doc.word_at(position_to_pos(position))?;
        let decl_pos = *doc.declarations.get(&word.to_lowercase())?;
        Some(Location {
            uri: uri.clone(),
            range: point_range(decl_pos, word.chars().count()),
        })
    }

    pub fn document_symbol(&self, uri: &Uri) -> Option<Vec<DocumentSymbol>> {
        let documents = self.documents.read().unwrap();
        let analyzed = documents.get(uri)?.last_ok.as_ref()?;
        Some(build_symbols(&analyzed.workspace, &analyzed.id_to_pos))
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
