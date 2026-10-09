//! Runtime-agnostic language-server logic.
//!
//! Everything here is synchronous and free of tokio/tower, so the same code
//! backs both the stdio server (`backend.rs`) and the WASM build
//! (`jsonrpc.rs` → `structurizr-lsp-wasm`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;

use ls_types::*;
use structurizr_dsl::lexer::Pos;
use structurizr_dsl::SourceLocation;
use structurizr_model::{DeploymentNode, Port, Relationship, Workspace};

use crate::context::context_at;
use crate::convert::{point_range, pos_to_position, position_to_pos};
use crate::document::{Analyzed, DocumentState};
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
        let path = document_path(&uri);
        let mut documents = self.documents.write().unwrap();
        documents
            .entry(uri)
            .or_insert_with(DocumentState::empty)
            .update(text, path)
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
        let len = word.chars().count();
        if let Some(decl_pos) = doc.declarations.get(&word.to_lowercase()) {
            return Some(Location {
                uri: uri.clone(),
                range: point_range(*decl_pos, len),
            });
        }
        // Declared in an `!include`d file: the parser resolved the identifier
        // to an element, and knows where that element was declared.
        let analyzed = doc.last_ok.as_ref()?;
        let (id, _) = analyzed.identifiers.resolve(word)?;
        let location = analyzed.locations.get(id)?;
        let file = location.file.as_ref().filter(|_| !doc.is_here(location))?;
        Some(Location {
            uri: Uri::from_file_path(file)?,
            range: point_range(
                Pos {
                    line: location.line,
                    col: location.col,
                },
                len,
            ),
        })
    }

    pub fn document_symbol(&self, uri: &Uri) -> Option<Vec<DocumentSymbol>> {
        let documents = self.documents.read().unwrap();
        let doc = documents.get(uri)?;
        let analyzed = doc.last_ok.as_ref()?;
        Some(build_symbols(doc, analyzed))
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
            documents
                .get(uri)?
                .declarations
                .get(&word.to_lowercase())
                .copied()
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

    pub fn document_highlight(
        &self,
        uri: &Uri,
        position: Position,
    ) -> Option<Vec<DocumentHighlight>> {
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
        ranges
            .into_iter()
            .find(|r| {
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

/// The file a document URI names, when the server can read around it: only
/// `file:` URIs, and never in the WASM build, which has no filesystem.
fn document_path(uri: &Uri) -> Option<PathBuf> {
    if cfg!(target_arch = "wasm32") || !uri.as_str().starts_with("file:") {
        return None;
    }
    uri.to_file_path().map(|path| path.into_owned())
}

/// Model items to put in the outline, gathered in one walk: (id, name, kind,
/// detail) per element and port, plus every relationship and each element's
/// name, which a relationship's symbol is named after.
#[derive(Default)]
struct Outline<'a> {
    declared: Vec<(&'a str, String, SymbolKind, Option<String>)>,
    names: HashMap<&'a str, &'a str>,
    relationships: Vec<&'a Relationship>,
}

impl<'a> Outline<'a> {
    fn element(
        &mut self,
        id: &'a str,
        name: &'a str,
        kind: SymbolKind,
        ports: &'a Option<Vec<Port>>,
        relationships: &'a Option<Vec<Relationship>>,
    ) {
        self.names.insert(id, name);
        self.declared.push((id, name.to_string(), kind, None));
        for port in ports.iter().flatten() {
            self.declared
                .push((&port.id, port.name.clone(), SymbolKind::PROPERTY, None));
        }
        self.relationships.extend(relationships.iter().flatten());
    }

    fn deployment_node(&mut self, node: &'a DeploymentNode) {
        self.element(
            &node.id,
            &node.name,
            SymbolKind::NAMESPACE,
            &None,
            &node.relationships,
        );
        for inf in node.infrastructure_nodes.iter().flatten() {
            self.element(
                &inf.id,
                &inf.name,
                SymbolKind::INTERFACE,
                &None,
                &inf.relationships,
            );
        }
        for ci in node.container_instances.iter().flatten() {
            let name = self
                .names
                .get(ci.container_id.as_str())
                .copied()
                .unwrap_or("instance");
            self.element(&ci.id, name, SymbolKind::VARIABLE, &None, &ci.relationships);
        }
        for si in node.software_system_instances.iter().flatten() {
            let name = self
                .names
                .get(si.software_system_id.as_str())
                .copied()
                .unwrap_or("instance");
            self.element(&si.id, name, SymbolKind::VARIABLE, &None, &si.relationships);
        }
        for child in node.children.iter().flatten() {
            self.deployment_node(child);
        }
    }
}

/// The outline: every element, port, relationship and view declared in this
/// document (not in the files it includes), nested by source range. Nesting
/// follows the text rather than the model, so a relationship declared in an
/// element's body sits under that element and one declared at model level
/// doesn't.
fn build_symbols(doc: &DocumentState, analyzed: &Analyzed) -> Vec<DocumentSymbol> {
    let model = &analyzed.workspace.model;
    let mut outline = Outline::default();
    for p in model.people.iter().flatten() {
        outline.element(
            &p.id,
            &p.name,
            SymbolKind::OBJECT,
            &p.ports,
            &p.relationships,
        );
    }
    for s in model.software_systems.iter().flatten() {
        outline.element(
            &s.id,
            &s.name,
            SymbolKind::MODULE,
            &s.ports,
            &s.relationships,
        );
        for c in s.containers.iter().flatten() {
            outline.element(
                &c.id,
                &c.name,
                SymbolKind::CLASS,
                &c.ports,
                &c.relationships,
            );
            for comp in c.components.iter().flatten() {
                outline.element(
                    &comp.id,
                    &comp.name,
                    SymbolKind::STRUCT,
                    &comp.ports,
                    &comp.relationships,
                );
            }
        }
    }
    for e in model.custom_elements.iter().flatten() {
        outline.element(
            &e.id,
            &e.name,
            SymbolKind::OBJECT,
            &e.ports,
            &e.relationships,
        );
    }
    // After the static model, so instances can take their element's name.
    for node in model.deployment_nodes.iter().flatten() {
        outline.deployment_node(node);
    }

    let mut flat = Vec::new();
    let here = |id: &str| {
        analyzed
            .locations
            .get(id)
            .filter(|location| doc.is_here(location))
    };
    for (id, name, kind, detail) in outline.declared {
        if let Some(location) = here(id) {
            flat.push(symbol(doc, location, name, kind, detail));
        }
    }
    let name_of = |id: &str| {
        outline
            .names
            .get(id)
            .map_or_else(|| id.to_string(), |n| n.to_string())
    };
    for r in &outline.relationships {
        if let Some(location) = here(&r.id) {
            let name = format!("{} → {}", name_of(&r.source_id), name_of(&r.destination_id));
            flat.push(symbol(
                doc,
                location,
                name,
                SymbolKind::EVENT,
                r.description.clone(),
            ));
        }
    }
    for (key, location) in analyzed.locations.views() {
        if doc.is_here(location) {
            flat.push(symbol(
                doc,
                location,
                format!("view {key}"),
                SymbolKind::PACKAGE,
                None,
            ));
        }
    }
    nest(flat)
}

/// A symbol spanning its whole declaring statement, from its first token to
/// the end of the line it closes on.
/// (`DocumentSymbol.deprecated` is a deprecated field we still have to set:
/// there is no `Default` impl.)
#[allow(deprecated)]
fn symbol(
    doc: &DocumentState,
    location: &SourceLocation,
    name: String,
    kind: SymbolKind,
    detail: Option<String>,
) -> DocumentSymbol {
    let start = pos_to_position(Pos {
        line: location.line,
        col: location.col,
    });
    let end_line = location.end_line.max(location.line);
    let end_character = doc
        .text
        .lines()
        .nth(end_line - 1)
        .map_or(0, |line| line.chars().count() as u32);
    let end = if end_line == location.line {
        Position {
            line: start.line,
            character: end_character.max(start.character + 1),
        }
    } else {
        Position {
            line: (end_line - 1) as u32,
            character: end_character,
        }
    };
    DocumentSymbol {
        name,
        detail,
        kind,
        tags: None,
        deprecated: None,
        range: Range { start, end },
        selection_range: point_range(
            Pos {
                line: location.line,
                col: location.col,
            },
            1,
        ),
        children: None,
    }
}

/// Nest symbols by range containment, keeping source order among siblings.
fn nest(mut flat: Vec<DocumentSymbol>) -> Vec<DocumentSymbol> {
    let key = |p: Position| (p.line, p.character);
    flat.sort_by_key(|s| (key(s.range.start), std::cmp::Reverse(key(s.range.end))));

    fn attach(
        stack: &mut [DocumentSymbol],
        roots: &mut Vec<DocumentSymbol>,
        symbol: DocumentSymbol,
    ) {
        match stack.last_mut() {
            Some(parent) => parent.children.get_or_insert_with(Vec::new).push(symbol),
            None => roots.push(symbol),
        }
    }

    let mut roots = Vec::new();
    let mut stack: Vec<DocumentSymbol> = Vec::new();
    for symbol in flat {
        while let Some(top) = stack.last() {
            if key(top.range.start) <= key(symbol.range.start)
                && key(symbol.range.end) <= key(top.range.end)
            {
                break;
            }
            let done = stack.pop().expect("non-empty");
            attach(&mut stack, &mut roots, done);
        }
        stack.push(symbol);
    }
    while let Some(done) = stack.pop() {
        attach(&mut stack, &mut roots, done);
    }
    roots
}
