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

use crate::context::context_at;
use crate::convert::{point_range, pos_to_position, position_to_pos};
use crate::document::DocumentState;
use crate::semantic;

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
            references_provider: Some(OneOf::Left(true)),
            document_highlight_provider: Some(OneOf::Left(true)),
            rename_provider: Some(OneOf::Right(RenameOptions {
                prepare_provider: Some(true),
                work_done_progress_options: WorkDoneProgressOptions::default(),
            })),
            semantic_tokens_provider: Some(
                SemanticTokensServerCapabilities::SemanticTokensOptions(SemanticTokensOptions {
                    legend: semantic::legend(),
                    full: Some(SemanticTokensFullOptions::Bool(true)),
                    range: Some(false),
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                }),
            ),
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

    /// Completions legal at `position`: the keywords of the enclosing block,
    /// plus the elements declared so far.
    ///
    /// Scoping matters more here than it looks. The DSL reuses the same words
    /// at different depths (`container` declares an element in a
    /// `softwareSystem` body but opens a view under `views`), so an unscoped
    /// list is mostly wrong suggestions.
    pub fn completion(&self, uri: &Uri, position: Position) -> Vec<CompletionItem> {
        let documents = self.documents.read().unwrap();
        let Some(doc) = documents.get(uri) else {
            return Vec::new();
        };
        let ctx = context_at(&doc.tokens, position_to_pos(position));

        let mut items = Vec::new();
        // Right after `->` the only thing that can follow is the destination
        // element, so offering keywords would only get in the way.
        if !ctx.after_arrow {
            let wanted = ctx.keyword_set();
            let mut seen = std::collections::HashSet::new();
            for &(block, keywords) in structurizr_dsl::keyword_sets() {
                // An unrecognised block falls back to every keyword: a noisy
                // list is still better than an empty one.
                if wanted.is_some_and(|w| w != block) {
                    continue;
                }
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
        }

        if ctx.wants_identifiers() {
            if let Some(analyzed) = doc.last_ok.as_ref() {
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

    pub fn references(
        &self,
        uri: &Uri,
        position: Position,
        include_declaration: bool,
    ) -> Option<Vec<Location>> {
        let (word, ranges) = self.identifier_ranges(uri, position)?;
        let decl_pos = if include_declaration {
            None
        } else {
            let documents = self.documents.read().unwrap();
            documents.get(uri)?.declarations.get(&word.to_lowercase()).copied()
        };
        let decl_start = decl_pos.map(pos_to_position);
        Some(
            ranges
                .into_iter()
                .filter(|range| Some(range.start) != decl_start)
                .map(|range| Location {
                    uri: uri.clone(),
                    range,
                })
                .collect(),
        )
    }

    pub fn document_highlight(&self, uri: &Uri, position: Position) -> Option<Vec<DocumentHighlight>> {
        let (_, ranges) = self.identifier_ranges(uri, position)?;
        Some(
            ranges
                .into_iter()
                .map(|range| DocumentHighlight {
                    range,
                    kind: Some(DocumentHighlightKind::TEXT),
                })
                .collect(),
        )
    }

    /// The range the editor should pre-fill in its rename box, or `None` if
    /// this position isn't a renameable identifier — which is how the editor
    /// knows to refuse the rename up front rather than after the fact.
    pub fn prepare_rename(&self, uri: &Uri, position: Position) -> Option<Range> {
        let (word, ranges) = self.identifier_ranges(uri, position)?;
        let cursor = position_to_pos(position);
        // The occurrence under the cursor, i.e. the one the editor will
        // highlight and pre-fill.
        ranges.into_iter().find(|r| {
            r.start.line as usize + 1 == cursor.line
                && (r.start.character..=r.end.character).contains(&(cursor.col as u32 - 1))
        })
        .or_else(|| Some(point_range(cursor, word.chars().count())))
    }

    pub fn rename(&self, uri: &Uri, position: Position, new_name: &str) -> Option<WorkspaceEdit> {
        let (_, ranges) = self.identifier_ranges(uri, position)?;
        let edits: Vec<TextEdit> = ranges
            .into_iter()
            .map(|range| TextEdit {
                range,
                new_text: new_name.to_string(),
            })
            .collect();
        Some(WorkspaceEdit {
            changes: Some(HashMap::from([(uri.clone(), edits)])),
            ..WorkspaceEdit::default()
        })
    }

    pub fn semantic_tokens(&self, uri: &Uri) -> Option<SemanticTokens> {
        let documents = self.documents.read().unwrap();
        let doc = documents.get(uri)?;
        Some(SemanticTokens {
            result_id: None,
            data: semantic::encode(&doc.tokens, &doc.declarations),
        })
    }

    /// The identifier at `position` and the ranges of every token referring to
    /// it. `None` unless the word is a *declared* identifier, which keeps
    /// references and rename off keywords and quoted text.
    fn identifier_ranges(&self, uri: &Uri, position: Position) -> Option<(String, Vec<Range>)> {
        let documents = self.documents.read().unwrap();
        let doc = documents.get(uri)?;
        let word = doc.word_at(position_to_pos(position))?.to_string();
        if !doc.declarations.contains_key(&word.to_lowercase()) {
            return None;
        }
        let len = word.chars().count();
        let ranges = crate::index::find_references(&doc.tokens, &word)
            .into_iter()
            .map(|pos| point_range(pos, len))
            .collect();
        Some((word, ranges))
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
