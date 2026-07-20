use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use structurizr_model::*;

use crate::error::ParseError;
use crate::identifier_register::{ElementType, IdentifierMode, IdentifierRegister};
use crate::lexer::{tokenize, Spanned, Token};

/// Parse a DSL file from disk.
pub fn parse_file(path: impl AsRef<Path>) -> Result<Workspace, ParseError> {
    let path = path.as_ref();
    let source = std::fs::read_to_string(path)?;
    let base = path.parent().map(|p| p.to_path_buf());
    let (source, source_map) = match &base {
        Some(dir) => {
            let mut map = SourceMap::default();
            let spliced = preprocess_includes(&source, dir, 0, None, &mut map)?;
            (spliced, Some(map))
        }
        None => (source, None),
    };
    let tokens = tokenize(&source);
    let mut parser = Parser::new(tokens);
    parser.base_path = base;
    parser.source_map = source_map;
    parser.parse_workspace_toplevel()
}

/// Maps each line of the post-`!include` spliced source back to
/// `(source file, original line)`. The entry file is `None`.
#[derive(Default)]
struct SourceMap {
    /// Index = spliced line - 1.
    lines: Vec<(Option<String>, usize)>,
}

impl SourceMap {
    /// Resolve a spliced line number to `(file, original line)`.
    fn resolve(&self, spliced_line: usize) -> (Option<&str>, usize) {
        match spliced_line.checked_sub(1).and_then(|i| self.lines.get(i)) {
            Some((file, orig)) => (file.as_deref(), *orig),
            None => (None, spliced_line),
        }
    }

    /// Human-readable location for a spliced line, naming the file when the
    /// line came from an `!include`d file.
    fn describe(&self, spliced_line: usize) -> String {
        match self.resolve(spliced_line) {
            (Some(file), line) => format!("line {} in {}", line, file),
            (None, line) => format!("line {}", line),
        }
    }
}

/// Splice `!include <path>` lines with the referenced file's contents,
/// recursively (paths are relative to the including file), recording each
/// output line's origin in `map`. A depth cap guards against cycles.
fn preprocess_includes(
    source: &str,
    dir: &Path,
    depth: usize,
    file_label: Option<&str>,
    map: &mut SourceMap,
) -> Result<String, ParseError> {
    const MAX_INCLUDE_DEPTH: usize = 16;
    if depth > MAX_INCLUDE_DEPTH {
        return Err(ParseError::syntax(0, 0, "include depth exceeded (cycle?)".to_string()));
    }
    let mut out = String::with_capacity(source.len());
    for (idx, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        let rest = trimmed
            .strip_prefix("!include ")
            .or_else(|| trimmed.strip_prefix("!INCLUDE "));
        if let Some(rest) = rest {
            let rel = rest.trim().trim_matches('"');
            let inc_path = dir.join(rel);
            let inc_src = std::fs::read_to_string(&inc_path).map_err(|e| {
                ParseError::syntax(0, 0, format!("cannot read !include '{}': {}", inc_path.display(), e))
            })?;
            let inc_dir = inc_path.parent().unwrap_or(dir).to_path_buf();
            out.push_str(&preprocess_includes(&inc_src, &inc_dir, depth + 1, Some(rel), map)?);
            out.push('\n');
            map.lines.push((file_label.map(str::to_string), idx + 1));
        } else {
            out.push_str(line);
            out.push('\n');
            map.lines.push((file_label.map(str::to_string), idx + 1));
        }
    }
    Ok(out)
}

/// Parse a DSL string into a Workspace.
pub fn parse_str(source: &str) -> Result<Workspace, ParseError> {
    let tokens = tokenize(source);
    let mut parser = Parser::new(tokens);
    parser.parse_workspace_toplevel()
}

struct Parser {
    tokens: Vec<Spanned>,
    pos: usize,
    id_counter: u64,
    register: IdentifierRegister,
    constants: HashMap<String, String>,
    /// Directory of the DSL file being parsed (used for resolving relative !adrs paths).
    base_path: Option<PathBuf>,
    /// Decisions accumulated while parsing (flushed into workspace.documentation at the end).
    accumulated_decisions: Vec<Decision>,
    /// Maps a lowercase dotted path `"element_path.port_ident"` → (element_id, port_id).
    port_register: HashMap<String, (String, String)>,
    /// Kind aliases declared in a `specification { kind <alias> <base> ... }` block.
    kind_aliases: HashMap<String, KindAlias>,
    /// Sketch mode (spec §4.1): unknown relationship endpoints are auto-created
    /// as placeholder software systems. Set by `!sketch` or a bare sketch file.
    sketch: bool,
    /// Relationship endpoints that did not resolve when first seen. Retried at
    /// the end of the model block (forward references legal); anything still
    /// unresolved is an error outside sketch mode.
    pending_endpoints: Vec<PendingEndpoint>,
    /// Model-level relationships whose source element was not yet declared when
    /// the relationship was parsed; attached to their source in finalize_model.
    deferred_rels: Vec<Relationship>,
    /// Separator for nested group names (model property `structurizr.groupSeparator`).
    group_separator: Option<String>,
    /// Names of the model-level `group` blocks currently being parsed.
    model_group_stack: Vec<String>,
    /// `ident = deploymentGroup "Name"` bindings (lowercased ident → name).
    deployment_group_names: HashMap<String, String>,
    /// Maps spliced line numbers back to their originating `!include`d file.
    source_map: Option<SourceMap>,
}

/// A relationship endpoint identifier that had no binding at parse time.
struct PendingEndpoint {
    ident: String,
    rel_id: String,
    source_side: bool,
    line: usize,
    col: usize,
}

/// Attribute keywords accepted in any element body (person, component, …).
const ELEMENT_BODY_KEYWORDS: [&str; 12] = [
    "description", "technology", "url", "tags", "group", "properties", "perspective",
    "perspectives", "port", "status", "introduced", "retired",
];

/// Keywords accepted in a `softwareSystem { ... }` body.
const SOFTWARE_SYSTEM_BODY_KEYWORDS: [&str; 13] = [
    "container", "group", "description", "technology", "url", "tags", "properties",
    "perspective", "perspectives", "port", "status", "introduced", "retired",
];

/// Keywords accepted in a `container { ... }` body.
const CONTAINER_BODY_KEYWORDS: [&str; 13] = [
    "component", "group", "description", "technology", "url", "tags", "properties",
    "perspective", "perspectives", "port", "status", "introduced", "retired",
];

/// Keywords accepted in a `deploymentNode { ... }` body.
const DEPLOYMENT_NODE_BODY_KEYWORDS: [&str; 13] = [
    "deploymentNode", "containerInstance", "softwareSystemInstance", "infrastructureNode",
    "instanceOf", "instances", "group", "deploymentGroup", "description", "technology",
    "url", "tags", "properties",
];

/// Keywords accepted directly inside `model { ... }`.
const MODEL_KEYWORDS: [&str; 7] = [
    "person", "softwareSystem", "group", "enterprise", "deploymentEnvironment", "element",
    "properties",
];

/// Keywords accepted directly inside `workspace { ... }`.
const WORKSPACE_KEYWORDS: [&str; 11] = [
    "name", "description", "model", "views", "configuration", "documentation",
    "docs", "specification", "milestones", "perspectives", "properties",
];

/// Keywords accepted directly inside `views { ... }`.
const VIEWS_KEYWORDS: [&str; 15] = [
    "auto", "systemLandscape", "systemContext", "container", "component", "dynamic",
    "deployment", "filtered", "image", "custom", "styles", "theme", "themes", "branding",
    "properties",
];

/// A vocabulary alias declared in `specification { kind queue container { ... } }`.
/// Elements declared through an alias are stored as the base kind, with the alias
/// tags merged and a `kind` property recording the alias name (spec §3.1).
#[derive(Debug, Clone)]
struct KindAlias {
    base: String,
    tags: Option<String>,
    technology: Option<String>,
}

/// Extra attributes collected from an element body block (`{ ... }`).
#[derive(Default)]
struct ElementExtras {
    status: Option<Status>,
    introduced: Option<String>,
    retired: Option<String>,
    perspectives: Vec<Perspective>,
    /// Ports parsed from `port <ident> [...]` declarations: (local DSL ident, Port).
    ports: Vec<(String, Port)>,
    description: Option<String>,
    technology: Option<String>,
    url: Option<String>,
    /// Extra tags from a `tags` body keyword, comma-split.
    tags_extra: Vec<String>,
    properties: HashMap<String, String>,
    /// Group membership from a leaf `group "Name"` body keyword.
    group: Option<String>,
}

impl Parser {
    fn new(tokens: Vec<Spanned>) -> Self {
        Self {
            tokens,
            pos: 0,
            id_counter: 1,
            register: IdentifierRegister::new(),
            constants: HashMap::new(),
            base_path: None,
            accumulated_decisions: Vec::new(),
            port_register: HashMap::new(),
            kind_aliases: HashMap::new(),
            sketch: false,
            pending_endpoints: Vec::new(),
            deferred_rels: Vec::new(),
            group_separator: None,
            model_group_stack: Vec::new(),
            deployment_group_names: HashMap::new(),
            source_map: None,
        }
    }

    /// Entry point wrapping [`parse_workspace`] to post-process errors into
    /// actionable locations: include-aware line numbers and unclosed-brace
    /// reporting instead of a bare "unexpected end of input".
    fn parse_workspace_toplevel(&mut self) -> Result<Workspace, ParseError> {
        let unclosed = find_unclosed_brace(&self.tokens);
        match self.parse_workspace() {
            Ok(ws) => Ok(ws),
            Err(ParseError::UnexpectedEof) => match unclosed {
                Some(pos) => Err(ParseError::syntax(
                    pos.line,
                    pos.col,
                    format!(
                        "unexpected end of input: block opened at {} (column {}) is never closed",
                        self.describe_line(pos.line),
                        pos.col
                    ),
                )),
                None => Err(ParseError::UnexpectedEof),
            },
            Err(ParseError::Syntax { line, col, message }) => {
                let (file, orig_line) = match &self.source_map {
                    Some(map) => map.resolve(line),
                    None => (None, line),
                };
                let message = match file {
                    Some(f) => format!("in {}: {}", f, message),
                    None => message,
                };
                Err(ParseError::syntax(orig_line, col, message))
            }
            Err(e) => Err(e),
        }
    }

    /// Include-aware human-readable description of a source line.
    fn describe_line(&self, line: usize) -> String {
        match &self.source_map {
            Some(map) => map.describe(line),
            None => format!("line {}", line),
        }
    }

    fn next_id(&mut self) -> String {
        let id = self.id_counter.to_string();
        self.id_counter += 1;
        id
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|s| &s.token)
    }

    #[allow(dead_code)]
    fn peek_spanned(&self) -> Option<&Spanned> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Spanned> {
        let t = self.tokens.get(self.pos);
        self.pos += 1;
        t
    }

    #[allow(dead_code)]
    fn current_line(&self) -> usize {
        self.tokens
            .get(self.pos.saturating_sub(1))
            .map(|s| s.pos.line)
            .unwrap_or(0)
    }

    fn current_pos(&self) -> (usize, usize) {
        self.tokens
            .get(self.pos)
            .map(|s| (s.pos.line, s.pos.col))
            .unwrap_or((0, 0))
    }

    /// Consume a token if it matches the given word (case-insensitive).
    fn expect_word(&mut self, word: &str) -> Result<(), ParseError> {
        match self.peek() {
            Some(Token::Word(w)) if w.eq_ignore_ascii_case(word) => {
                self.advance();
                Ok(())
            }
            Some(other) => {
                let (line, col) = self.current_pos();
                Err(ParseError::syntax(
                    line,
                    col,
                    format!("expected '{}', got {:?}", word, other),
                ))
            }
            None => Err(ParseError::UnexpectedEof),
        }
    }

    fn expect_open_brace(&mut self) -> Result<(), ParseError> {
        match self.peek() {
            Some(Token::OpenBrace) => {
                self.advance();
                Ok(())
            }
            Some(other) => {
                let (line, col) = self.current_pos();
                Err(ParseError::syntax(
                    line,
                    col,
                    format!("expected '{{', got {:?}", other),
                ))
            }
            None => Err(ParseError::UnexpectedEof),
        }
    }

    fn expect_close_brace(&mut self) -> Result<(), ParseError> {
        match self.peek() {
            Some(Token::CloseBrace) => {
                self.advance();
                Ok(())
            }
            Some(other) => {
                let (line, col) = self.current_pos();
                Err(ParseError::syntax(
                    line,
                    col,
                    format!("expected '}}', got {:?}", other),
                ))
            }
            None => Err(ParseError::UnexpectedEof),
        }
    }

    /// Consume next token as a string value (Word or Quoted or TextBlock).
    fn consume_string(&mut self) -> Option<String> {
        match self.peek() {
            Some(Token::Quoted(_)) | Some(Token::Word(_)) | Some(Token::TextBlock(_)) => {
                match self.advance().map(|s| s.token.clone()) {
                    Some(Token::Quoted(s)) => Some(self.substitute_vars(&s)),
                    Some(Token::Word(s)) => Some(self.substitute_vars(&s)),
                    Some(Token::TextBlock(s)) => Some(self.substitute_vars(&s)),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn resolve_identifier(&self, identifier: &str) -> String {
        if let Some(id) = self.register.resolve_id(identifier) {
            return id;
        }

        if self.register.mode == IdentifierMode::Hierarchical {
            if let Some(last) = identifier.rsplit('.').next() {
                if let Some(id) = self.register.resolve_id(last) {
                    return id;
                }
            }
        }

        identifier.to_string()
    }

    /// Resolve a relationship endpoint identifier to `(element_id, Option<port_id>)`.
    ///
    /// Resolution order:
    /// (a) If the full string resolves to a known element (direct or hierarchical fallback),
    ///     return `(element_id, None)`.
    /// (b) If the string contains '.' and is found in the port register, return
    ///     `(element_id, Some(port_id))`.
    /// (c) Otherwise fall back to `resolve_identifier` (returns the string itself if unknown).
    fn resolve_endpoint(&self, ident: &str) -> (String, Option<String>) {
        // (a) Try full ident as direct element in register.
        if self.register.resolve_id(ident).is_some() {
            return (self.resolve_identifier(ident), None);
        }
        // Also try hierarchical fallback (last segment after last dot).
        if self.register.mode == IdentifierMode::Hierarchical && ident.contains('.') {
            if let Some(last) = ident.rsplit('.').next() {
                if self.register.resolve_id(last).is_some() {
                    return (self.resolve_identifier(ident), None);
                }
            }
        }
        // (b) Try port_register lookup (only makes sense when ident contains '.').
        if ident.contains('.') {
            if let Some((elem_id, port_id)) = self.port_register.get(&ident.to_lowercase()) {
                return (elem_id.clone(), Some(port_id.clone()));
            }
        }
        // (c) Fall back to current resolve_identifier behaviour.
        (self.resolve_identifier(ident), None)
    }

    /// True if `ident` currently resolves to a declared element or port.
    fn endpoint_resolves(&self, ident: &str) -> bool {
        if self.register.resolve_id(ident).is_some() {
            return true;
        }
        if self.register.mode == IdentifierMode::Hierarchical {
            if let Some(last) = ident.rsplit('.').next() {
                if self.register.resolve_id(last).is_some() {
                    return true;
                }
            }
        }
        ident.contains('.') && self.port_register.contains_key(&ident.to_lowercase())
    }

    /// Resolve a relationship endpoint, recording it for the end-of-model retry
    /// when it has no binding yet (forward reference or typo).
    fn resolve_endpoint_tracked(
        &mut self,
        ident: &str,
        rel_id: &str,
        source_side: bool,
        pos: (usize, usize),
    ) -> (String, Option<String>) {
        let resolved = self.resolve_endpoint(ident);
        if !ident.is_empty() && !self.endpoint_resolves(ident) {
            self.pending_endpoints.push(PendingEndpoint {
                ident: ident.to_string(),
                rel_id: rel_id.to_string(),
                source_side,
                line: pos.0,
                col: pos.1,
            });
        }
        resolved
    }

    /// Build an error for an unknown keyword at the current (unconsumed) token,
    /// naming the context, the accepted keywords, and a close match if any.
    fn unknown_keyword_error(&self, context: &str, allowed: &[&str]) -> ParseError {
        let (line, col) = self.current_pos();
        let word = match self.peek() {
            Some(Token::Word(w)) => w.clone(),
            Some(other) => {
                return ParseError::syntax(
                    line,
                    col,
                    format!("unexpected {:?} in {}; expected one of: {}", other, context, allowed.join(", ")),
                )
            }
            None => return ParseError::UnexpectedEof,
        };
        let mut message = format!(
            "unknown keyword '{}' in {}; expected one of: {}",
            word,
            context,
            allowed.join(", ")
        );
        if let Some(suggestion) = crate::suggest::closest(&word, allowed.iter().copied()) {
            message.push_str(&format!("; did you mean '{}'?", suggestion));
        }
        ParseError::syntax(line, col, message)
    }

    /// Group membership for elements declared inside model-level `group` blocks:
    /// the current stack of group names joined by the group separator.
    fn current_model_group(&self) -> Option<String> {
        if self.model_group_stack.is_empty() {
            return None;
        }
        let sep = self.group_separator.as_deref().unwrap_or("/");
        Some(self.model_group_stack.join(sep))
    }

    /// Build an error for an unknown or misplaced keyword at model level.
    /// `container`/`component` get a pointed message since declaring them at
    /// the wrong nesting level is a common mistake.
    fn unknown_model_keyword_error(&self, word_lower: &str) -> ParseError {
        let (line, col) = self.current_pos();
        match word_lower {
            "container" => ParseError::syntax(
                line,
                col,
                "'container' cannot be declared at model level; declare it inside a softwareSystem block",
            ),
            "component" => ParseError::syntax(
                line,
                col,
                "'component' cannot be declared at model level; declare it inside a container block",
            ),
            _ => {
                let alias_names: Vec<&str> = self.kind_aliases.keys().map(|s| s.as_str()).collect();
                let allowed: Vec<&str> = MODEL_KEYWORDS.iter().copied().chain(alias_names).collect();
                let mut message = format!(
                    "unknown keyword '{}' in model; expected one of: {}, or a relationship 'a -> b'",
                    word_lower,
                    allowed.join(", ")
                );
                if let Some(suggestion) = crate::suggest::closest(word_lower, allowed.iter().copied()) {
                    message.push_str(&format!("; did you mean '{}'?", suggestion));
                }
                ParseError::syntax(line, col, message)
            }
        }
    }

    /// Retry unresolved relationship endpoints against the now-complete
    /// identifier register, rewrite the affected relationships, attach deferred
    /// model-level relationships to their source elements, and error (outside
    /// sketch mode) on anything still unresolved — all occurrences at once.
    fn finalize_model(&mut self, model: &mut Model) -> Result<(), ParseError> {
        let pendings = std::mem::take(&mut self.pending_endpoints);
        let element_ids = collect_element_ids(model);
        let mut failures: Vec<(usize, String)> = Vec::new();
        let mut first_pos: Option<(usize, usize)> = None;

        for p in &pendings {
            if self.endpoint_resolves(&p.ident) {
                let (id, port) = self.resolve_endpoint(&p.ident);
                rewrite_rel_endpoint(model, &mut self.deferred_rels, &p.rel_id, p.source_side, &id, port);
            } else if element_ids.contains(&p.ident) || self.sketch {
                // Literal element id, or sketch mode (lenient by design).
            } else {
                first_pos.get_or_insert((p.line, p.col));
                let mut msg = format!("unknown element identifier '{}' in relationship", p.ident);
                let candidates = self
                    .register
                    .identifiers
                    .keys()
                    .chain(self.port_register.keys())
                    .map(|s| s.as_str());
                if let Some(suggestion) = crate::suggest::closest(&p.ident, candidates) {
                    msg.push_str(&format!(" (did you mean '{}'?)", suggestion));
                }
                failures.push((p.line, msg));
            }
        }

        if let Some((line, col)) = first_pos {
            let message = if failures.len() == 1 {
                failures.remove(0).1
            } else {
                let lines: Vec<String> = failures
                    .into_iter()
                    .map(|(l, m)| format!("{}: {}", self.describe_line(l), m))
                    .collect();
                format!("unresolved identifiers:\n  {}", lines.join("\n  "))
            };
            return Err(ParseError::syntax(line, col, message));
        }

        for rel in std::mem::take(&mut self.deferred_rels) {
            let source_id = rel.source_id.clone();
            self.attach_relationship_to_element(model, &source_id, rel);
        }
        Ok(())
    }

    fn substitute_vars(&self, s: &str) -> String {
        let mut result = s.to_string();
        for (k, v) in &self.constants {
            result = result.replace(&format!("${{{}}}", k), v);
        }
        result
    }

    /// Check if next token is a word matching (case-insensitive).
    fn peek_word(&self, word: &str) -> bool {
        matches!(self.peek(), Some(Token::Word(w)) if w.eq_ignore_ascii_case(word))
    }

    #[allow(dead_code)]
    fn peek_directive(&self, name: &str) -> bool {
        matches!(self.peek(), Some(Token::Directive(d)) if d.eq_ignore_ascii_case(name))
    }

    #[allow(dead_code)]
    fn peek_arrow(&self) -> bool {
        matches!(self.peek(), Some(Token::Arrow))
    }

    fn peek_open_brace(&self) -> bool {
        matches!(self.peek(), Some(Token::OpenBrace))
    }

    fn peek_close_brace(&self) -> bool {
        matches!(self.peek(), Some(Token::CloseBrace))
    }

    #[allow(dead_code)]
    fn peek_equals(&self) -> bool {
        matches!(self.peek(), Some(Token::Equals))
    }

    /// Skip tokens until we find a close brace at the same depth.
    fn skip_block(&mut self) {
        let mut depth = 1;
        while self.pos < self.tokens.len() {
            match self.peek() {
                Some(Token::OpenBrace) => {
                    depth += 1;
                    self.advance();
                }
                Some(Token::CloseBrace) => {
                    depth -= 1;
                    self.advance();
                    if depth == 0 {
                        break;
                    }
                }
                _ => {
                    self.advance();
                }
            }
        }
    }

    // ─── ADR import ─────────────────────────────────────────────────────────────

    /// Read all AdrTools-format `.md` files from `rel_path` (relative to `base_path`)
    /// and return them as `Decision` objects.
    fn import_adrs(&self, rel_path: &str) -> Vec<Decision> {
        let base = match &self.base_path {
            Some(p) => p.clone(),
            None => {
                eprintln!("Warning: !adrs '{}' ignored (no base path — use parse_file)", rel_path);
                return vec![];
            }
        };
        let dir = base.join(rel_path);
        if !dir.is_dir() {
            eprintln!("Warning: ADR directory not found: {}", dir.display());
            return vec![];
        }
        let mut files: Vec<PathBuf> = match std::fs::read_dir(&dir) {
            Ok(entries) => entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "md"))
                .collect(),
            Err(e) => {
                eprintln!("Warning: Could not read ADR directory {}: {}", dir.display(), e);
                return vec![];
            }
        };
        files.sort();
        files.iter().filter_map(|p| Self::parse_adr_file(p)).collect()
    }

    /// Parse a single AdrTools-format Markdown file into a `Decision`.
    fn parse_adr_file(path: &Path) -> Option<Decision> {
        let filename = path.file_name()?.to_str()?;
        // ID is parsed from the leading digits of the filename (e.g. "0001" → "1").
        let leading_digits: String = filename.chars().take_while(|c| c.is_ascii_digit()).collect();
        if leading_digits.is_empty() {
            return None;
        }
        // Parse as integer to strip leading zeros ("0001" → 1 → "1", "0000" → 0 → "0").
        let id: u64 = leading_digits.parse().ok()?;
        let id = id.to_string();

        let raw = std::fs::read_to_string(path).ok()?;
        let content = raw.replace('\r', "");
        let lines: Vec<&str> = content.lines().collect();

        // Title: first line is expected to be "# N. Title" → extract "Title".
        let title = lines.first()
            .and_then(|l| {
                let stripped = l.trim_start_matches('#').trim();
                stripped.find(". ").map(|i| stripped[i + 2..].trim().to_string())
            })
            .unwrap_or_else(|| filename.to_string());

        // Date: first line matching "Date: YYYY-MM-DD".
        let date = lines.iter()
            .find(|l| l.starts_with("Date: "))
            .map(|l| l["Date: ".len()..].trim().to_string())
            .unwrap_or_default();

        // Status: first non-empty line after "## Status".
        let mut in_status = false;
        let mut status = "Proposed".to_string();
        for line in &lines {
            if !in_status {
                if line.trim() == "## Status" {
                    in_status = true;
                }
            } else {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    let word = trimmed.split_whitespace().next().unwrap_or("Proposed");
                    status = if word == "Superceded" { "Superseded".to_string() } else { word.to_string() };
                    break;
                }
            }
        }

        Some(Decision {
            id,
            title,
            date,
            status,
            format: "Markdown".to_string(),
            content,
            element_id: None,
        })
    }

    // ─── Top level ──────────────────────────────────────────────────────────────

    fn parse_workspace(&mut self) -> Result<Workspace, ParseError> {
        let mut workspace = Workspace::default();

        // Handle optional directives before `workspace`
        self.handle_pre_workspace_directives();

        // A file with no `workspace` block is a sketch (spec §4.1): bare model
        // statements, auto-vivified placeholders, one default landscape view.
        if !self.peek_word("workspace") && self.peek().is_some() {
            return self.parse_sketch();
        }

        // `workspace` keyword
        self.expect_word("workspace")?;

        // Optional name and description
        workspace.name = self
            .consume_string()
            .unwrap_or_else(|| "Workspace".to_string());
        if let Some(desc) = self.consume_string_if_not_brace() {
            workspace.description = Some(desc);
        }

        self.expect_open_brace()?;

        while !self.peek_close_brace() && self.peek().is_some() {
            self.parse_workspace_item(&mut workspace)?;
        }

        self.expect_close_brace()?;

        // Flush all accumulated ADR decisions into workspace.documentation.
        if !self.accumulated_decisions.is_empty() {
            let doc = workspace.documentation.get_or_insert_with(Documentation::default);
            let existing = doc.decisions.get_or_insert_with(Vec::new);
            existing.append(&mut self.accumulated_decisions);
        }

        Ok(workspace)
    }

    /// Parse a bare sketch file: statements are treated as model items and a
    /// default include-all landscape view is synthesized.
    fn parse_sketch(&mut self) -> Result<Workspace, ParseError> {
        self.sketch = true;
        let mut workspace = Workspace {
            name: "Sketch".to_string(),
            ..Default::default()
        };

        while self.peek().is_some() {
            if self.peek_close_brace() {
                self.advance(); // tolerate stray braces
                continue;
            }
            self.parse_model_item(&mut workspace.model, None)?;
        }
        self.finalize_model(&mut workspace.model)?;

        let mut view = SystemLandscapeView {
            key: Some("sketch".to_string()),
            ..Default::default()
        };
        self.populate_system_landscape_view(&workspace.model, &mut view);
        workspace.views.system_landscape_views = Some(vec![view]);

        Ok(workspace)
    }

    /// In sketch mode, create a placeholder software system for an identifier
    /// that does not resolve to any declared element (spec §4.1).
    fn vivify_placeholder(&mut self, model: &mut Model, ident: &str) {
        if ident.is_empty() || ident.contains('.') || ident == "*" {
            return;
        }
        if self.register.resolve_id(ident).is_some() {
            return;
        }
        let id = self.next_id();
        let ss = SoftwareSystem {
            id: id.clone(),
            name: ident.to_string(),
            tags: Some("Element,Software System,Placeholder".to_string()),
            ..Default::default()
        };
        model.software_systems.get_or_insert_with(Vec::new).push(ss);
        self.register.register(ident, id, ElementType::SoftwareSystem);
    }

    fn handle_pre_workspace_directives(&mut self) {
        while let Some(Token::Directive(_)) = self.peek() {
            let dir = match self.advance().map(|s| s.token.clone()) {
                Some(Token::Directive(d)) => d.to_lowercase(),
                _ => break,
            };
            match dir.as_str() {
                "const" | "constant" | "var" => {
                    let name = self.consume_string().unwrap_or_default();
                    let value = self.consume_string().unwrap_or_default();
                    self.constants.insert(name, value);
                }
                "sketch" => {
                    self.sketch = true;
                }
                _ => {}
            }
        }
    }

    fn consume_string_if_not_brace(&mut self) -> Option<String> {
        // Only consume explicitly quoted or text-block strings.
        // Bare words may be identifiers starting the next element and must not be consumed here.
        match self.peek() {
            Some(Token::Quoted(_)) | Some(Token::TextBlock(_)) => self.consume_string(),
            _ => None,
        }
    }

    fn parse_workspace_item(&mut self, workspace: &mut Workspace) -> Result<(), ParseError> {
        match self.peek() {
            Some(Token::Directive(d)) => {
                let d = d.clone();
                match d.to_lowercase().as_str() {
                    "const" | "constant" | "var" => {
                        self.advance();
                        let name = self.consume_string().unwrap_or_default();
                        let value = self.consume_string().unwrap_or_default();
                        self.constants.insert(name, value);
                    }
                    "identifiers" => {
                        self.advance();
                        let mode = self.consume_string().unwrap_or_default();
                        if mode.eq_ignore_ascii_case("hierarchical") {
                            self.register.mode = IdentifierMode::Hierarchical;
                        }
                    }
                    "sketch" => {
                        self.advance();
                        self.sketch = true;
                    }
                    "implied_relationships" | "impliedrelationships" => {
                        self.advance();
                        // Skip optional boolean/block
                        self.consume_string();
                    }
                    "adrs" | "decisions" => {
                        self.advance();
                        let rel_path = self.consume_string().unwrap_or_default();
                        // Skip optional exclude block.
                        if self.peek_open_brace() {
                            self.advance();
                            self.skip_block();
                        }
                        let decisions = self.import_adrs(&rel_path);
                        self.accumulated_decisions.extend(decisions);
                    }
                    _ => {
                        self.advance();
                        self.skip_directive_args();
                    }
                }
            }
            Some(Token::Word(w)) => {
                let w = w.to_lowercase();
                match w.as_str() {
                    "name" => {
                        self.advance();
                        if let Some(name) = self.consume_string() {
                            workspace.name = name;
                        }
                    }
                    "description" => {
                        self.advance();
                        workspace.description = self.consume_string();
                    }
                    "model" => {
                        self.advance();
                        self.expect_open_brace()?;
                        self.parse_model(&mut workspace.model)?;
                        self.expect_close_brace()?;
                    }
                    "views" => {
                        self.advance();
                        self.expect_open_brace()?;
                        self.parse_views(&mut workspace.views, &workspace.model)?;
                        self.expect_close_brace()?;
                    }
                    "configuration" => {
                        self.advance();
                        self.expect_open_brace()?;
                        let cfg = self.parse_configuration()?;
                        workspace.configuration = Some(cfg);
                    }
                    "documentation" | "docs" => {
                        self.advance();
                        self.expect_open_brace()?;
                        self.skip_block();
                    }
                    "specification" => {
                        self.advance();
                        self.expect_open_brace()?;
                        while !self.peek_close_brace() && self.peek().is_some() {
                            if self.peek_word("kind") {
                                self.advance();
                                let (line, col) = self.current_pos();
                                let alias_name = self.consume_bare_word_or_string().unwrap_or_default().to_lowercase();
                                let base = self.consume_bare_word_or_string().unwrap_or_default().to_lowercase();
                                if !matches!(base.as_str(), "person" | "softwaresystem" | "container" | "component") {
                                    return Err(ParseError::syntax(line, col, format!(
                                        "kind alias base must be person|softwareSystem|container|component, got: {}",
                                        base
                                    )));
                                }
                                let mut alias_tags = None;
                                let mut alias_tech = None;
                                if self.peek_open_brace() {
                                    self.advance();
                                    while !self.peek_close_brace() && self.peek().is_some() {
                                        if self.peek_word("tags") {
                                            self.advance();
                                            alias_tags = self.consume_string();
                                        } else if self.peek_word("technology") {
                                            self.advance();
                                            alias_tech = self.consume_string();
                                        } else {
                                            self.advance();
                                            self.skip_optional_block_or_value();
                                        }
                                    }
                                    self.expect_close_brace()?;
                                }
                                if !alias_name.is_empty() {
                                    self.kind_aliases.insert(alias_name, KindAlias {
                                        base,
                                        tags: alias_tags,
                                        technology: alias_tech,
                                    });
                                }
                            } else {
                                self.advance();
                                self.skip_optional_block_or_value();
                            }
                        }
                        self.expect_close_brace()?;
                    }
                    "milestones" => {
                        self.advance();
                        self.expect_open_brace()?;
                        let mut milestones: Vec<Milestone> = Vec::new();
                        while !self.peek_close_brace() && self.peek().is_some() {
                            // Each entry: name [date [description]]
                            // Name is a bare word or quoted string; date and description are
                            // quoted strings (or bare words for robustness).
                            let name = match self.peek() {
                                Some(Token::Word(_)) | Some(Token::Quoted(_)) | Some(Token::TextBlock(_)) => {
                                    self.consume_bare_word_or_string().unwrap_or_default()
                                }
                                _ => { self.advance(); continue; }
                            };
                            if name.is_empty() { continue; }
                            let date        = self.consume_string_if_not_brace();
                            let description = self.consume_string_if_not_brace();
                            milestones.push(Milestone { name, date, description });
                        }
                        self.expect_close_brace()?;
                        workspace.milestones = Some(milestones);
                    }
                    "perspectives" => {
                        // Workspace-level perspectives registry:
                        // perspectives { name ["description"] ... }
                        self.advance();
                        self.expect_open_brace()?;
                        let mut perspectives: Vec<Perspective> = Vec::new();
                        while !self.peek_close_brace() && self.peek().is_some() {
                            let p = self.parse_one_perspective();
                            if !p.name.is_empty() {
                                perspectives.push(p);
                            }
                        }
                        self.expect_close_brace()?;
                        workspace.perspectives = Some(perspectives);
                    }
                    "properties" => {
                        self.advance();
                        let props = self.parse_properties_block_body()?;
                        workspace.properties.get_or_insert_with(HashMap::new).extend(props);
                    }
                    _ => {
                        if !self.sketch {
                            return Err(self.unknown_keyword_error("workspace", &WORKSPACE_KEYWORDS));
                        }
                        self.advance();
                        self.skip_optional_block_or_value();
                    }
                }
            }
            Some(Token::CloseBrace) => {}
            _ => {
                self.advance();
            }
        }
        Ok(())
    }

    fn skip_optional_block_or_value(&mut self) {
        if self.peek_open_brace() {
            self.advance();
            self.skip_block();
        } else {
            // consume optional value tokens
            while matches!(self.peek(), Some(Token::Word(_)) | Some(Token::Quoted(_)) | Some(Token::TextBlock(_))) {
                self.advance();
            }
        }
    }

    /// Skip a directive's arguments: consume at most ONE argument string and then
    /// optionally a `{ ... }` block.  This avoids the greedy multi-word consumption
    /// of `skip_optional_block_or_value` which would accidentally eat the next
    /// keyword on the following line (e.g. `properties`, `model`, etc.).
    fn skip_directive_args(&mut self) {
        let _ = self.consume_string(); // consume at most one argument
        if self.peek_open_brace() {
            self.advance();
            self.skip_block();
        }
    }

    // ─── Model ──────────────────────────────────────────────────────────────────

    fn parse_model(&mut self, model: &mut Model) -> Result<(), ParseError> {
        while !self.peek_close_brace() && self.peek().is_some() {
            self.parse_model_item(model, None)?;
        }
        self.finalize_model(model)
    }

    fn parse_model_item(&mut self, model: &mut Model, _parent_env: Option<&str>) -> Result<(), ParseError> {
        // Check for assignment: `id = element ...`
        let (identifier, _) = self.peek_assignment();
        let has_assign = identifier.is_some();
        let identifier = identifier.unwrap_or_default();

        if has_assign {
            // consume identifier and `=`
            self.advance(); // id word
            self.advance(); // `=`
        }

        match self.peek() {
            Some(Token::Word(w)) => {
                let w = w.to_lowercase();
                match w.as_str() {
                    "person" => {
                        self.advance();
                        let p = self.parse_person(if has_assign { &identifier } else { "" })?;
                        model.people.get_or_insert_with(Vec::new).push(p);
                    }
                    "softwaresystem" => {
                        self.advance();
                        let ss = self.parse_software_system(if has_assign { &identifier } else { "" })?;
                        model.software_systems.get_or_insert_with(Vec::new).push(ss);
                    }
                    "deploymentenvironment" => {
                        self.advance();
                        let nodes = self.parse_deployment_environment()?;
                        model.deployment_nodes.get_or_insert_with(Vec::new).extend(nodes);
                    }
                    "enterprise" => {
                        self.advance();
                        let name = self.consume_string().unwrap_or_default();
                        model.enterprise = Some(Enterprise { name });
                        // Elements declared inside the enterprise block are
                        // ordinary model items (upstream: "internal" elements).
                        if self.peek_open_brace() {
                            self.advance();
                            while !self.peek_close_brace() && self.peek().is_some() {
                                self.parse_model_item(model, None)?;
                            }
                            self.expect_close_brace()?;
                        }
                    }
                    "group" => {
                        self.advance();
                        let name = self.consume_string().unwrap_or_default();
                        if self.peek_open_brace() {
                            self.advance();
                            self.model_group_stack.push(name);
                            let result = (|| {
                                while !self.peek_close_brace() && self.peek().is_some() {
                                    self.parse_model_item(model, None)?;
                                }
                                self.expect_close_brace()
                            })();
                            self.model_group_stack.pop();
                            result?;
                        }
                    }
                    "element" => {
                        self.advance();
                        let ce = self.parse_custom_element(if has_assign { &identifier } else { "" })?;
                        model.custom_elements.get_or_insert_with(Vec::new).push(ce);
                    }
                    "properties" => {
                        self.advance();
                        let props = self.parse_properties_block_body()?;
                        if let Some(sep) = props.get("structurizr.groupSeparator") {
                            self.group_separator = Some(sep.clone());
                        }
                        model.properties.get_or_insert_with(HashMap::new).extend(props);
                    }
                    _ => {
                        if let Some((alias_name, alias)) = self.peek_kind_alias("person") {
                            self.advance();
                            let mut p = self.parse_person(if has_assign { &identifier } else { "" })?;
                            apply_alias_to_tags_props(&alias_name, &alias, &mut p.tags, &mut p.properties);
                            model.people.get_or_insert_with(Vec::new).push(p);
                            return Ok(());
                        }
                        if let Some((alias_name, alias)) = self.peek_kind_alias("softwaresystem") {
                            self.advance();
                            let mut ss = self.parse_software_system(if has_assign { &identifier } else { "" })?;
                            apply_alias_to_tags_props(&alias_name, &alias, &mut ss.tags, &mut ss.properties);
                            model.software_systems.get_or_insert_with(Vec::new).push(ss);
                            return Ok(());
                        }
                        // Could be a relationship: `sourceId -> destinationId ...`
                        // or an unknown keyword
                        if !has_assign {
                            // peek ahead to see if it's a relationship
                            if self.peek_at_arrow_after_word() {
                                self.parse_relationship_in_model(model)?;
                            } else if !self.sketch {
                                return Err(self.unknown_model_keyword_error(&w));
                            } else {
                                // skip unknown
                                self.advance();
                                self.skip_optional_block_or_value();
                            }
                        } else if self.peek_at_arrow_after_word() {
                            // Named relationship: `name = a -> b ...`
                            let rel_id = self.parse_relationship_in_model(model)?;
                            self.register.register(&identifier, rel_id, ElementType::Relationship);
                        } else if !self.sketch {
                            return Err(self.unknown_model_keyword_error(&w));
                        } else {
                            // has identifier but unknown keyword
                            self.advance();
                            self.skip_optional_block_or_value();
                        }
                    }
                }
            }
            Some(Token::Directive(d)) => {
                let d = d.clone();
                self.advance();
                match d.to_lowercase().as_str() {
                    "const" | "constant" | "var" => {
                        let name = self.consume_string().unwrap_or_default();
                        let value = self.consume_string().unwrap_or_default();
                        self.constants.insert(name, value);
                    }
                    _ => {
                        self.skip_directive_args();
                    }
                }
            }
            Some(Token::CloseBrace) => {}
            _ => {
                self.advance();
            }
        }
        Ok(())
    }

    /// Look ahead to determine if current position has `word = keyword` pattern.
    fn peek_assignment(&self) -> (Option<String>, Option<String>) {
        if let (Some(Token::Word(id)), Some(Token::Equals)) =
            (self.tokens.get(self.pos).map(|s| &s.token),
             self.tokens.get(self.pos + 1).map(|s| &s.token))
        {
            (Some(id.clone()), None)
        } else {
            (None, None)
        }
    }

    fn peek_at_arrow_after_word(&self) -> bool {
        matches!(self.tokens.get(self.pos + 1).map(|s| &s.token), Some(Token::Arrow))
    }

    fn parse_person(&mut self, identifier: &str) -> Result<Person, ParseError> {
        let id = self.next_id();
        let name = self.consume_string().unwrap_or_else(|| "Person".to_string());
        let description = self.consume_string_if_not_brace();
        let tags = self.consume_string_if_not_brace_or_kw();

        if !identifier.is_empty() {
            self.register.register(identifier, id.clone(), ElementType::Person);
        }

        let mut person = Person {
            id: id.clone(),
            name,
            description,
            tags: merge_tags("Element,Person", tags),
            ..Default::default()
        };

        if self.consume_uncertainty_marker() {
            person.tags = match person.tags.take() {
                Some(t) => Some(format!("{},Uncertain", t)),
                None => Some("Uncertain".to_string()),
            };
        }

        if self.peek_open_brace() {
            self.advance();
            let paths = vec![identifier.to_string()];
            let (rels, extras) = self.parse_element_block(&id, &paths)?;
            if !rels.is_empty() {
                person.relationships = Some(rels);
            }
            if extras.status.is_some()    { person.status    = extras.status; }
            if extras.introduced.is_some(){ person.introduced = extras.introduced; }
            if extras.retired.is_some()   { person.retired   = extras.retired; }
            if !extras.perspectives.is_empty() {
                person.perspectives = Some(extras.perspectives);
            }
            if !extras.ports.is_empty() {
                person.ports = Some(extras.ports.into_iter().map(|(_, p)| p).collect());
            }
            if extras.description.is_some() { person.description = extras.description; }
            if extras.url.is_some()         { person.url         = extras.url; }
            if !extras.tags_extra.is_empty() {
                let extra = extras.tags_extra.join(",");
                person.tags = Some(match person.tags.take() {
                    Some(t) => format!("{},{}", t, extra),
                    None => extra,
                });
            }
            if !extras.properties.is_empty() {
                person.properties.get_or_insert_with(HashMap::new).extend(extras.properties);
            }
            if extras.group.is_some() { person.group = extras.group; }
            self.expect_close_brace()?;
        }
        if person.group.is_none() {
            person.group = self.current_model_group();
        }

        Ok(person)
    }

    fn parse_software_system(&mut self, identifier: &str) -> Result<SoftwareSystem, ParseError> {
        let id = self.next_id();
        let name = self.consume_string().unwrap_or_else(|| "SoftwareSystem".to_string());
        let description = self.consume_string_if_not_brace();
        let tags = self.consume_string_if_not_brace_or_kw();

        if !identifier.is_empty() {
            self.register
                .register(identifier, id.clone(), ElementType::SoftwareSystem);
        }

        let mut ss = SoftwareSystem {
            id: id.clone(),
            name,
            description,
            tags: merge_tags("Element,Software System", tags),
            ..Default::default()
        };

        if self.consume_uncertainty_marker() {
            ss.tags = match ss.tags.take() {
                Some(t) => Some(format!("{},Uncertain", t)),
                None => Some("Uncertain".to_string()),
            };
        }

        if self.peek_open_brace() {
            self.advance();
            let mut containers: Vec<Container> = Vec::new();
            let mut rels: Vec<Relationship> = Vec::new();
            let mut ss_extras = ElementExtras::default();

            while !self.peek_close_brace() && self.peek().is_some() {
                let (ident, _) = self.peek_assignment();
                let has_ident = ident.is_some();
                let ident = ident.unwrap_or_default();

                if has_ident {
                    self.advance(); // id
                    self.advance(); // =
                }

                if self.peek_word("container") {
                    self.advance();
                    let c = self.parse_container(
                        if has_ident { &ident } else { "" },
                        identifier,
                    )?;
                    containers.push(c);
                } else if let Some((alias_name, alias)) = self.peek_kind_alias("container") {
                    self.advance();
                    let mut c = self.parse_container(
                        if has_ident { &ident } else { "" },
                        identifier,
                    )?;
                    apply_alias_to_tags_props(&alias_name, &alias, &mut c.tags, &mut c.properties);
                    if c.technology.is_none() {
                        c.technology = alias.technology.clone();
                    }
                    containers.push(c);
                } else if self.peek_word("group") {
                    self.advance();
                    if let Some(leaf) = self.parse_ss_group(
                        if has_ident { &ident } else { "" },
                        identifier,
                        "",
                        &mut containers,
                        &mut rels,
                    )? {
                        ss_extras.group = Some(leaf);
                    }
                } else if self.peek_at_arrow_after_word() {
                    let src_pos = self.current_pos();
                    let src  = self.consume_string().unwrap_or_default();
                    self.advance(); // ->
                    let dst_pos = self.current_pos();
                    let dst  = self.consume_string().unwrap_or_default();
                    let desc = self.consume_string_if_not_brace();
                    let tech = self.consume_string_if_not_brace();
                    let rel_id = self.next_id();
                    let uncertain = self.consume_uncertainty_marker();
                    // `this` refers to the enclosing element (upstream DSL).
                    let (src_id, src_port) = if src.eq_ignore_ascii_case("this") {
                        (id.clone(), None)
                    } else {
                        self.resolve_endpoint_tracked(&src, &rel_id, true, src_pos)
                    };
                    let (dst_id, dst_port) = if dst.eq_ignore_ascii_case("this") {
                        (id.clone(), None)
                    } else {
                        self.resolve_endpoint_tracked(&dst, &rel_id, false, dst_pos)
                    };
                    let mut rel = Relationship {
                        id: rel_id,
                        source_id: src_id,
                        destination_id: dst_id,
                        source_port_id: src_port,
                        destination_port_id: dst_port,
                        description: desc,
                        technology: tech,
                        tags: Some("Relationship".to_string()),
                        ..Default::default()
                    };
                    if self.peek_open_brace() {
                        self.parse_relationship_body(&mut rel)?;
                    }
                    if uncertain {
                        rel.tags = Some(match rel.tags.take() {
                            Some(t) => format!("{},Uncertain", t),
                            None => "Uncertain".to_string(),
                        });
                    }
                    rels.push(rel);
                } else if !has_ident && matches!(self.peek(), Some(Token::Arrow)) {
                    let rel = self.parse_implicit_relationship(&id)?;
                    rels.push(rel);
                } else if !has_ident && {
                    let paths = vec![identifier.to_string()];
                    self.try_parse_common_element_keyword(&mut ss_extras, &id, &paths)?
                } {
                    // element attribute for the software system itself consumed
                } else if matches!(self.peek(), Some(Token::Directive(d)) if d.eq_ignore_ascii_case("adrs") || d.eq_ignore_ascii_case("decisions")) {
                    self.advance();
                    let rel_path = self.consume_string().unwrap_or_default();
                    if self.peek_open_brace() { self.advance(); self.skip_block(); }
                    let mut decisions = self.import_adrs(&rel_path);
                    for d in &mut decisions { d.element_id = Some(id.clone()); }
                    self.accumulated_decisions.extend(decisions);
                } else if matches!(self.peek(), Some(Token::Directive(_))) {
                    self.advance();
                    self.skip_directive_args();
                } else if !self.sketch {
                    return Err(self.unknown_keyword_error(
                        "softwareSystem body",
                        &SOFTWARE_SYSTEM_BODY_KEYWORDS,
                    ));
                } else {
                    self.advance();
                    if self.peek_open_brace() {
                        self.advance();
                        self.skip_block();
                    } else {
                        let _ = self.consume_string();
                    }
                }
            }
            self.expect_close_brace()?;
            if !containers.is_empty() {
                ss.containers = Some(containers);
            }
            if !rels.is_empty() {
                ss.relationships = Some(rels);
            }
            if ss_extras.status.is_some()     { ss.status     = ss_extras.status; }
            if ss_extras.introduced.is_some() { ss.introduced = ss_extras.introduced; }
            if ss_extras.retired.is_some()    { ss.retired    = ss_extras.retired; }
            if !ss_extras.perspectives.is_empty() {
                ss.perspectives = Some(ss_extras.perspectives);
            }
            if !ss_extras.ports.is_empty() {
                ss.ports = Some(ss_extras.ports.into_iter().map(|(_, p)| p).collect());
            }
            if ss_extras.description.is_some() { ss.description = ss_extras.description; }
            if ss_extras.url.is_some()         { ss.url         = ss_extras.url; }
            if !ss_extras.tags_extra.is_empty() {
                let extra = ss_extras.tags_extra.join(",");
                ss.tags = Some(match ss.tags.take() {
                    Some(t) => format!("{},{}", t, extra),
                    None => extra,
                });
            }
            if !ss_extras.properties.is_empty() {
                ss.properties.get_or_insert_with(HashMap::new).extend(ss_extras.properties);
            }
            if ss_extras.group.is_some() { ss.group = ss_extras.group; }
        }
        if ss.group.is_none() {
            ss.group = self.current_model_group();
        }

        Ok(ss)
    }

    /// Parse a `group` declaration inside a softwareSystem body (the `group`
    /// keyword is already consumed). Returns `Some(name)` for the leaf form
    /// (`group "Name"` with no block), which assigns membership to the
    /// enclosing element. Block members — containers, nested groups,
    /// relationships — are appended to `containers`/`rels`, with each
    /// container's `group` set to the joined group path.
    fn parse_ss_group(
        &mut self,
        group_ident: &str,
        parent_path: &str,
        display_prefix: &str,
        containers: &mut Vec<Container>,
        rels: &mut Vec<Relationship>,
    ) -> Result<Option<String>, ParseError> {
        let gname = self.consume_string().unwrap_or_default();

        // Register the group identifier so neighborhood includes can expand it.
        // Groups are registered with a synthetic ID (not a real element ID) that
        // is never used directly — only as a prefix key for `children_of`.
        let group_hier = if self.register.mode == IdentifierMode::Hierarchical
            && !parent_path.is_empty()
            && !group_ident.is_empty()
        {
            format!("{}.{}", parent_path, group_ident)
        } else {
            group_ident.to_string()
        };
        if !group_ident.is_empty() {
            let synthetic_id = format!("group:{}", group_hier);
            self.register.register(group_ident, synthetic_id.clone(), ElementType::Group);
            if !group_hier.is_empty() && group_hier != group_ident {
                self.register.register(&group_hier, synthetic_id, ElementType::Group);
            }
        }

        if !self.peek_open_brace() {
            return Ok(if gname.is_empty() { None } else { Some(gname) });
        }
        self.advance();

        let sep = self.group_separator.clone().unwrap_or_else(|| "/".to_string());
        let display_path = if display_prefix.is_empty() {
            gname.clone()
        } else {
            format!("{}{}{}", display_prefix, sep, gname)
        };
        // Containers inside a named group use the group's path as identifier
        // parent, e.g. `softwareSystem.service1.service1Api`.
        let effective_parent = if !group_hier.is_empty() {
            group_hier.clone()
        } else {
            parent_path.to_string()
        };

        while !self.peek_close_brace() && self.peek().is_some() {
            let (gident, _) = self.peek_assignment();
            let has_gi = gident.is_some();
            let gident = gident.unwrap_or_default();
            if has_gi {
                self.advance();
                self.advance();
            }

            if self.peek_word("container") {
                self.advance();
                let mut c = self.parse_container(if has_gi { &gident } else { "" }, &effective_parent)?;
                c.group = Some(display_path.clone());
                containers.push(c);
            } else if let Some((alias_name, alias)) = self.peek_kind_alias("container") {
                self.advance();
                let mut c = self.parse_container(if has_gi { &gident } else { "" }, &effective_parent)?;
                apply_alias_to_tags_props(&alias_name, &alias, &mut c.tags, &mut c.properties);
                if c.technology.is_none() {
                    c.technology = alias.technology.clone();
                }
                c.group = Some(display_path.clone());
                containers.push(c);
            } else if self.peek_word("group") {
                self.advance();
                self.parse_ss_group(
                    if has_gi { &gident } else { "" },
                    &effective_parent,
                    &display_path,
                    containers,
                    rels,
                )?;
            } else if self.peek_at_arrow_after_word() {
                let src_pos = self.current_pos();
                let src  = self.consume_string().unwrap_or_default();
                self.advance(); // ->
                let dst_pos = self.current_pos();
                let dst  = self.consume_string().unwrap_or_default();
                let desc = self.consume_string_if_not_brace();
                let tech = self.consume_string_if_not_brace();
                let rel_id = self.next_id();
                let uncertain = self.consume_uncertainty_marker();
                let (src_id, src_port) = self.resolve_endpoint_tracked(&src, &rel_id, true, src_pos);
                let (dst_id, dst_port) = self.resolve_endpoint_tracked(&dst, &rel_id, false, dst_pos);
                let mut rel = Relationship {
                    id: rel_id,
                    source_id: src_id,
                    destination_id: dst_id,
                    source_port_id: src_port,
                    destination_port_id: dst_port,
                    description: desc,
                    technology: tech,
                    tags: Some("Relationship".to_string()),
                    ..Default::default()
                };
                if self.peek_open_brace() {
                    self.parse_relationship_body(&mut rel)?;
                }
                if uncertain {
                    rel.tags = Some(match rel.tags.take() {
                        Some(t) => format!("{},Uncertain", t),
                        None => "Uncertain".to_string(),
                    });
                }
                rels.push(rel);
            } else if self.peek_word("url") {
                // Accepted upstream on groups; not stored in the model.
                self.advance();
                let _ = self.consume_string();
            } else if self.peek_word("properties") {
                self.advance();
                let _ = self.parse_properties_block_body()?;
            } else if matches!(self.peek(), Some(Token::Directive(_))) {
                self.advance();
                self.skip_directive_args();
            } else if !self.sketch {
                return Err(self.unknown_keyword_error(
                    "softwareSystem group body",
                    &["container", "group", "url", "properties"],
                ));
            } else {
                self.advance();
                self.skip_optional_block_or_value();
            }
        }
        self.expect_close_brace()?;
        Ok(None)
    }

    /// Parse a `group` declaration inside a container body (the `group` keyword
    /// is already consumed). Same contract as [`parse_ss_group`], for components.
    fn parse_container_group(
        &mut self,
        group_ident: &str,
        display_prefix: &str,
        components: &mut Vec<Component>,
        rels: &mut Vec<Relationship>,
    ) -> Result<Option<String>, ParseError> {
        let gname = self.consume_string().unwrap_or_default();

        if !group_ident.is_empty() {
            let synthetic_id = format!("group:{}", group_ident);
            self.register.register(group_ident, synthetic_id, ElementType::Group);
        }

        if !self.peek_open_brace() {
            return Ok(if gname.is_empty() { None } else { Some(gname) });
        }
        self.advance();

        let sep = self.group_separator.clone().unwrap_or_else(|| "/".to_string());
        let display_path = if display_prefix.is_empty() {
            gname.clone()
        } else {
            format!("{}{}{}", display_prefix, sep, gname)
        };

        while !self.peek_close_brace() && self.peek().is_some() {
            let (gident, _) = self.peek_assignment();
            let has_gi = gident.is_some();
            let gident = gident.unwrap_or_default();
            if has_gi {
                self.advance();
                self.advance();
            }

            if self.peek_word("component") {
                self.advance();
                let mut c = self.parse_component(if has_gi { &gident } else { "" })?;
                c.group = Some(display_path.clone());
                components.push(c);
            } else if let Some((alias_name, alias)) = self.peek_kind_alias("component") {
                self.advance();
                let mut c = self.parse_component(if has_gi { &gident } else { "" })?;
                apply_alias_to_tags_props(&alias_name, &alias, &mut c.tags, &mut c.properties);
                if c.technology.is_none() {
                    c.technology = alias.technology.clone();
                }
                c.group = Some(display_path.clone());
                components.push(c);
            } else if self.peek_word("group") {
                self.advance();
                self.parse_container_group(
                    if has_gi { &gident } else { "" },
                    &display_path,
                    components,
                    rels,
                )?;
            } else if self.peek_at_arrow_after_word() {
                let src_pos = self.current_pos();
                let src  = self.consume_string().unwrap_or_default();
                self.advance(); // ->
                let dst_pos = self.current_pos();
                let dst  = self.consume_string().unwrap_or_default();
                let desc = self.consume_string_if_not_brace();
                let tech = self.consume_string_if_not_brace();
                let rel_id = self.next_id();
                let uncertain = self.consume_uncertainty_marker();
                let (src_id, src_port) = self.resolve_endpoint_tracked(&src, &rel_id, true, src_pos);
                let (dst_id, dst_port) = self.resolve_endpoint_tracked(&dst, &rel_id, false, dst_pos);
                let mut rel = Relationship {
                    id: rel_id,
                    source_id: src_id,
                    destination_id: dst_id,
                    source_port_id: src_port,
                    destination_port_id: dst_port,
                    description: desc,
                    technology: tech,
                    tags: Some("Relationship".to_string()),
                    ..Default::default()
                };
                if self.peek_open_brace() {
                    self.parse_relationship_body(&mut rel)?;
                }
                if uncertain {
                    rel.tags = Some(match rel.tags.take() {
                        Some(t) => format!("{},Uncertain", t),
                        None => "Uncertain".to_string(),
                    });
                }
                rels.push(rel);
            } else if self.peek_word("url") {
                self.advance();
                let _ = self.consume_string();
            } else if self.peek_word("properties") {
                self.advance();
                let _ = self.parse_properties_block_body()?;
            } else if matches!(self.peek(), Some(Token::Directive(_))) {
                self.advance();
                self.skip_directive_args();
            } else if !self.sketch {
                return Err(self.unknown_keyword_error(
                    "container group body",
                    &["component", "group", "url", "properties"],
                ));
            } else {
                self.advance();
                self.skip_optional_block_or_value();
            }
        }
        self.expect_close_brace()?;
        Ok(None)
    }

    fn parse_container(&mut self, identifier: &str, parent_identifier: &str) -> Result<Container, ParseError> {
        let id = self.next_id();
        let name = self.consume_string().unwrap_or_else(|| "Container".to_string());
        let description = self.consume_string_if_not_brace();
        let technology = self.consume_string_if_not_brace();
        let tags = self.consume_string_if_not_brace_or_kw();

        if !identifier.is_empty() {
            self.register
                .register(identifier, id.clone(), ElementType::Container);

            if self.register.mode == IdentifierMode::Hierarchical && !parent_identifier.is_empty() {
                self.register.register(
                    &format!("{}.{}", parent_identifier, identifier),
                    id.clone(),
                    ElementType::Container,
                );
            }
        }

        let mut container = Container {
            id: id.clone(),
            name,
            description,
            technology,
            tags: merge_tags("Element,Container", tags),
            ..Default::default()
        };

        if self.consume_uncertainty_marker() {
            container.tags = match container.tags.take() {
                Some(t) => Some(format!("{},Uncertain", t)),
                None => Some("Uncertain".to_string()),
            };
        }

        if self.peek_open_brace() {
            self.advance();
            let mut components: Vec<Component> = Vec::new();
            let mut rels: Vec<Relationship> = Vec::new();
            let mut cont_extras = ElementExtras::default();

            while !self.peek_close_brace() && self.peek().is_some() {
                let (ident, _) = self.peek_assignment();
                let has_ident = ident.is_some();
                let ident = ident.unwrap_or_default();

                if has_ident {
                    self.advance();
                    self.advance();
                }

                if self.peek_word("component") {
                    self.advance();
                    let c = self.parse_component(if has_ident { &ident } else { "" })?;
                    components.push(c);
                } else if let Some((alias_name, alias)) = self.peek_kind_alias("component") {
                    self.advance();
                    let mut c = self.parse_component(if has_ident { &ident } else { "" })?;
                    apply_alias_to_tags_props(&alias_name, &alias, &mut c.tags, &mut c.properties);
                    if c.technology.is_none() {
                        c.technology = alias.technology.clone();
                    }
                    components.push(c);
                } else if self.peek_word("group") {
                    self.advance();
                    if let Some(leaf) = self.parse_container_group(
                        if has_ident { &ident } else { "" },
                        "",
                        &mut components,
                        &mut rels,
                    )? {
                        cont_extras.group = Some(leaf);
                    }
                } else if self.peek_at_arrow_after_word() {
                    let src_pos = self.current_pos();
                    let src  = self.consume_string().unwrap_or_default();
                    self.advance(); // ->
                    let dst_pos = self.current_pos();
                    let dst  = self.consume_string().unwrap_or_default();
                    let desc = self.consume_string_if_not_brace();
                    let tech = self.consume_string_if_not_brace();
                    let rel_id = self.next_id();
                    let uncertain = self.consume_uncertainty_marker();
                    // `this` refers to the enclosing element (upstream DSL).
                    let (src_id, src_port) = if src.eq_ignore_ascii_case("this") {
                        (id.clone(), None)
                    } else {
                        self.resolve_endpoint_tracked(&src, &rel_id, true, src_pos)
                    };
                    let (dst_id, dst_port) = if dst.eq_ignore_ascii_case("this") {
                        (id.clone(), None)
                    } else {
                        self.resolve_endpoint_tracked(&dst, &rel_id, false, dst_pos)
                    };
                    let mut rel = Relationship {
                        id: rel_id,
                        source_id: src_id,
                        destination_id: dst_id,
                        source_port_id: src_port,
                        destination_port_id: dst_port,
                        description: desc,
                        technology: tech,
                        tags: Some("Relationship".to_string()),
                        ..Default::default()
                    };
                    if self.peek_open_brace() {
                        self.parse_relationship_body(&mut rel)?;
                    }
                    if uncertain {
                        rel.tags = Some(match rel.tags.take() {
                            Some(t) => format!("{},Uncertain", t),
                            None => "Uncertain".to_string(),
                        });
                    }
                    rels.push(rel);
                } else if !has_ident && matches!(self.peek(), Some(Token::Arrow)) {
                    let rel = self.parse_implicit_relationship(&id)?;
                    rels.push(rel);
                } else if !has_ident && {
                    let mut paths = vec![identifier.to_string()];
                    if self.register.mode == IdentifierMode::Hierarchical
                        && !parent_identifier.is_empty()
                        && !identifier.is_empty()
                    {
                        paths.push(format!("{}.{}", parent_identifier, identifier));
                    }
                    self.try_parse_common_element_keyword(&mut cont_extras, &id, &paths)?
                } {
                    // element attribute for the container itself consumed
                } else if matches!(self.peek(), Some(Token::Directive(d)) if d.eq_ignore_ascii_case("adrs") || d.eq_ignore_ascii_case("decisions")) {
                    self.advance();
                    let rel_path = self.consume_string().unwrap_or_default();
                    if self.peek_open_brace() { self.advance(); self.skip_block(); }
                    let mut decisions = self.import_adrs(&rel_path);
                    for d in &mut decisions { d.element_id = Some(id.clone()); }
                    self.accumulated_decisions.extend(decisions);
                } else if matches!(self.peek(), Some(Token::Directive(_))) {
                    self.advance();
                    self.skip_directive_args();
                } else if !self.sketch {
                    return Err(self.unknown_keyword_error("container body", &CONTAINER_BODY_KEYWORDS));
                } else {
                    // Consume only one value to avoid eating the next element's identifier.
                    self.advance();
                    if self.peek_open_brace() {
                        self.advance();
                        self.skip_block();
                    } else {
                        let _ = self.consume_string();
                    }
                }
            }
            self.expect_close_brace()?;
            if !components.is_empty() {
                container.components = Some(components);
            }
            if !rels.is_empty() {
                container.relationships = Some(rels);
            }
            if cont_extras.status.is_some()     { container.status     = cont_extras.status; }
            if cont_extras.introduced.is_some() { container.introduced = cont_extras.introduced; }
            if cont_extras.retired.is_some()    { container.retired    = cont_extras.retired; }
            if !cont_extras.perspectives.is_empty() {
                container.perspectives = Some(cont_extras.perspectives);
            }
            if !cont_extras.ports.is_empty() {
                container.ports = Some(cont_extras.ports.into_iter().map(|(_, p)| p).collect());
            }
            if cont_extras.description.is_some() { container.description = cont_extras.description; }
            if cont_extras.technology.is_some()  { container.technology  = cont_extras.technology; }
            if cont_extras.url.is_some()         { container.url         = cont_extras.url; }
            if !cont_extras.tags_extra.is_empty() {
                let extra = cont_extras.tags_extra.join(",");
                container.tags = Some(match container.tags.take() {
                    Some(t) => format!("{},{}", t, extra),
                    None => extra,
                });
            }
            if !cont_extras.properties.is_empty() {
                container.properties.get_or_insert_with(HashMap::new).extend(cont_extras.properties);
            }
            if cont_extras.group.is_some() { container.group = cont_extras.group; }
        }

        Ok(container)
    }

    fn parse_component(&mut self, identifier: &str) -> Result<Component, ParseError> {
        let id = self.next_id();
        let name = self.consume_string().unwrap_or_else(|| "Component".to_string());
        let description = self.consume_string_if_not_brace();
        let technology = self.consume_string_if_not_brace();
        let tags = self.consume_string_if_not_brace_or_kw();

        if !identifier.is_empty() {
            self.register
                .register(identifier, id.clone(), ElementType::Component);
        }

        let mut component = Component {
            id: id.clone(),
            name,
            description,
            technology,
            tags: merge_tags("Element,Component", tags),
            ..Default::default()
        };

        if self.consume_uncertainty_marker() {
            component.tags = match component.tags.take() {
                Some(t) => Some(format!("{},Uncertain", t)),
                None => Some("Uncertain".to_string()),
            };
        }

        if self.peek_open_brace() {
            self.advance();
            let paths = vec![identifier.to_string()];
            let (rels, extras) = self.parse_element_block(&id, &paths)?;
            if !rels.is_empty() {
                component.relationships = Some(rels);
            }
            if extras.status.is_some()     { component.status     = extras.status; }
            if extras.introduced.is_some() { component.introduced = extras.introduced; }
            if extras.retired.is_some()    { component.retired    = extras.retired; }
            if !extras.perspectives.is_empty() {
                component.perspectives = Some(extras.perspectives);
            }
            if !extras.ports.is_empty() {
                component.ports = Some(extras.ports.into_iter().map(|(_, p)| p).collect());
            }
            if extras.description.is_some() { component.description = extras.description; }
            if extras.technology.is_some()  { component.technology  = extras.technology; }
            if extras.url.is_some()         { component.url         = extras.url; }
            if !extras.tags_extra.is_empty() {
                let extra = extras.tags_extra.join(",");
                component.tags = Some(match component.tags.take() {
                    Some(t) => format!("{},{}", t, extra),
                    None => extra,
                });
            }
            if !extras.properties.is_empty() {
                component.properties.get_or_insert_with(HashMap::new).extend(extras.properties);
            }
            if extras.group.is_some() { component.group = extras.group; }
            self.expect_close_brace()?;
        }

        Ok(component)
    }

    /// Parse an upstream custom element: `x = element "Name" ["metadata" ["description" ["tags"]]]`.
    fn parse_custom_element(&mut self, identifier: &str) -> Result<CustomElement, ParseError> {
        let id = self.next_id();
        let name = self.consume_string().unwrap_or_else(|| "Element".to_string());
        let metadata = self.consume_string_if_not_brace();
        let description = self.consume_string_if_not_brace();
        let tags = self.consume_string_if_not_brace_or_kw();

        if !identifier.is_empty() {
            self.register.register(identifier, id.clone(), ElementType::CustomElement);
        }

        let mut element = CustomElement {
            id: id.clone(),
            name,
            metadata,
            description,
            tags: merge_tags("Element", tags),
            ..Default::default()
        };

        if self.consume_uncertainty_marker() {
            element.tags = match element.tags.take() {
                Some(t) => Some(format!("{},Uncertain", t)),
                None => Some("Uncertain".to_string()),
            };
        }

        if self.peek_open_brace() {
            self.advance();
            let paths = vec![identifier.to_string()];
            let (rels, extras) = self.parse_element_block(&id, &paths)?;
            if !rels.is_empty() {
                element.relationships = Some(rels);
            }
            if extras.status.is_some()     { element.status     = extras.status; }
            if extras.introduced.is_some() { element.introduced = extras.introduced; }
            if extras.retired.is_some()    { element.retired    = extras.retired; }
            if !extras.perspectives.is_empty() {
                element.perspectives = Some(extras.perspectives);
            }
            if !extras.ports.is_empty() {
                element.ports = Some(extras.ports.into_iter().map(|(_, p)| p).collect());
            }
            if extras.description.is_some() { element.description = extras.description; }
            if extras.url.is_some()         { element.url         = extras.url; }
            if !extras.tags_extra.is_empty() {
                let extra = extras.tags_extra.join(",");
                element.tags = Some(match element.tags.take() {
                    Some(t) => format!("{},{}", t, extra),
                    None => extra,
                });
            }
            if !extras.properties.is_empty() {
                element.properties.get_or_insert_with(HashMap::new).extend(extras.properties);
            }
            if extras.group.is_some() { element.group = extras.group; }
            self.expect_close_brace()?;
        }
        if element.group.is_none() {
            element.group = self.current_model_group();
        }

        Ok(element)
    }

    fn parse_deployment_environment(&mut self) -> Result<Vec<DeploymentNode>, ParseError> {
        let env_name = self.consume_string().unwrap_or_else(|| "Default".to_string());
        let mut nodes = Vec::new();
        // Collect relationships defined at environment level (e.g. node -> node)
        let mut env_rels: Vec<Relationship> = Vec::new();

        if self.peek_open_brace() {
            self.advance();
            // Depth of `group { ... }` wrappers around nodes; contents are
            // parsed flattened (node grouping is not modelled).
            let mut open_groups = 0usize;
            while self.peek().is_some() {
                if self.peek_close_brace() {
                    if open_groups > 0 {
                        self.advance();
                        open_groups -= 1;
                        continue;
                    }
                    break;
                }
                let (ident, _) = self.peek_assignment();
                let has_ident = ident.is_some();
                let ident = ident.unwrap_or_default();

                if has_ident {
                    self.advance();
                    self.advance();
                }

                if self.peek_word("group") {
                    self.advance();
                    let _ = self.consume_string();
                    if self.peek_open_brace() {
                        self.advance();
                        open_groups += 1;
                    }
                } else if self.peek_word("deploymentnode") {
                    self.advance();
                    let node = self.parse_deployment_node(
                        if has_ident { &ident } else { "" },
                        &env_name,
                    )?;
                    nodes.push(node);
                } else if self.peek_word("deploymentgroup") {
                    self.advance();
                    let name = self.consume_string().unwrap_or_default();
                    if has_ident && !name.is_empty() {
                        self.deployment_group_names.insert(ident.to_lowercase(), name);
                    }
                } else if !has_ident && self.peek_at_arrow_after_word() {
                    // Relationship between deployment nodes at environment level.
                    let src = self.consume_string().unwrap_or_default();
                    self.advance(); // ->
                    let dst = self.consume_string().unwrap_or_default();
                    let desc = self.consume_string_if_not_brace();
                    let tech = self.consume_string_if_not_brace();
                    let rel_id = self.next_id();
                    let src_id = self.resolve_identifier(&src);
                    let dst_id = self.resolve_identifier(&dst);
                    env_rels.push(Relationship {
                        id: rel_id,
                        source_id: src_id.clone(),
                        destination_id: dst_id,
                        description: desc,
                        technology: tech,
                        tags: Some("Relationship".to_string()),
                        ..Default::default()
                    });
                } else if matches!(self.peek(), Some(Token::Directive(_))) {
                    self.advance();
                    self.skip_directive_args();
                } else if !self.sketch {
                    return Err(self.unknown_keyword_error(
                        "deploymentEnvironment body",
                        &["deploymentNode", "deploymentGroup"],
                    ));
                } else {
                    self.advance();
                    self.skip_optional_block_or_value();
                }
            }
            self.expect_close_brace()?;
        }

        // Attach environment-level relationships to their source nodes.
        if !env_rels.is_empty() {
            Self::attach_deployment_rels(&mut nodes, &env_rels);
        }

        Ok(nodes)
    }

    /// Recursively search deployment node trees for a source node and attach
    /// unresolved environment-level relationships to it.
    fn attach_deployment_rels(nodes: &mut Vec<DeploymentNode>, rels: &[Relationship]) {
        for node in nodes.iter_mut() {
            // Attach any relationship whose source matches this deployment node.
            let to_attach: Vec<Relationship> = rels
                .iter()
                .filter(|r| r.source_id == node.id)
                .cloned()
                .collect();
            if !to_attach.is_empty() {
                let existing = node.relationships.get_or_insert_with(Vec::new);
                existing.extend(to_attach);
            }
            // Also check infrastructure nodes within this deployment node.
            if let Some(infra_nodes) = node.infrastructure_nodes.as_mut() {
                for inf in infra_nodes.iter_mut() {
                    let to_attach: Vec<Relationship> = rels
                        .iter()
                        .filter(|r| r.source_id == inf.id)
                        .cloned()
                        .collect();
                    if !to_attach.is_empty() {
                        let existing = inf.relationships.get_or_insert_with(Vec::new);
                        existing.extend(to_attach);
                    }
                }
            }
            // Also check container instances within this deployment node.
            if let Some(cis) = node.container_instances.as_mut() {
                for ci in cis.iter_mut() {
                    let to_attach: Vec<Relationship> = rels
                        .iter()
                        .filter(|r| r.source_id == ci.id)
                        .cloned()
                        .collect();
                    if !to_attach.is_empty() {
                        let existing = ci.relationships.get_or_insert_with(Vec::new);
                        existing.extend(to_attach);
                    }
                }
            }
            // Recurse into children.
            if let Some(children) = node.children.as_mut() {
                Self::attach_deployment_rels(children, rels);
            }
        }
    }

    fn parse_deployment_node(&mut self, identifier: &str, env: &str) -> Result<DeploymentNode, ParseError> {
        let id = self.next_id();
        let name = self.consume_string().unwrap_or_else(|| "Node".to_string());
        let description = self.consume_string_if_not_brace();
        let technology = self.consume_string_if_not_brace();
        let tags = self.consume_string_if_not_brace_or_kw();
        // Optional instances count (bare number or quoted)
        let instances_str = match self.peek() {
            Some(Token::Word(w)) if w.parse::<i64>().is_ok() => self.consume_string(),
            _ => self.consume_string_if_not_brace_or_kw(),
        };
        let instances: Option<serde_json::Value> = instances_str
            .and_then(|s| s.parse::<i64>().ok())
            .map(|n| serde_json::Value::Number(serde_json::Number::from(n)));

        if !identifier.is_empty() {
            self.register
                .register(identifier, id.clone(), ElementType::DeploymentNode);
        }

        let mut node = DeploymentNode {
            id: id.clone(),
            name,
            description,
            technology,
            tags: merge_tags("Element,Deployment Node", tags),
            environment: Some(env.to_string()),
            instances,
            ..Default::default()
        };

        if self.peek_open_brace() {
            self.advance();
            let mut children = Vec::new();
            let mut container_instances = Vec::new();
            let mut software_system_instances = Vec::new();
            let mut infrastructure_nodes = Vec::new();
            let mut rels = Vec::new();
            let mut node_extras = ElementExtras::default();
            // `deploymentGroup <ref>` statements: groups inherited by the
            // instances declared after them in this node.
            let mut inherited_groups: Vec<String> = Vec::new();
            // Depth of `group { ... }` wrappers; contents parsed flattened.
            let mut open_groups = 0usize;

            while self.peek().is_some() {
                if self.peek_close_brace() {
                    if open_groups > 0 {
                        self.advance();
                        open_groups -= 1;
                        continue;
                    }
                    break;
                }
                let (ident, _) = self.peek_assignment();
                let has_ident = ident.is_some();
                let ident = ident.unwrap_or_default();

                if has_ident {
                    self.advance();
                    self.advance();
                }

                if self.peek_word("group") {
                    self.advance();
                    let _ = self.consume_string();
                    if self.peek_open_brace() {
                        self.advance();
                        open_groups += 1;
                    }
                } else if self.peek_word("deploymentgroup") {
                    self.advance();
                    if let Some(gref) = self.consume_string() {
                        let name = self
                            .deployment_group_names
                            .get(&gref.to_lowercase())
                            .cloned()
                            .unwrap_or(gref);
                        inherited_groups.push(name);
                    }
                } else if self.peek_word("deploymentnode") {
                    self.advance();
                    let child = self.parse_deployment_node(
                        if has_ident { &ident } else { "" },
                        env,
                    )?;
                    children.push(child);
                } else if self.peek_word("containerinstance") {
                    self.advance();
                    let mut ci = self.parse_container_instance(if has_ident { &ident } else { "" }, env)?;
                    if ci.deployment_groups.is_none() && !inherited_groups.is_empty() {
                        ci.deployment_groups = Some(inherited_groups.clone());
                    }
                    container_instances.push(ci);
                } else if self.peek_word("softwaresysteminstance") {
                    self.advance();
                    let mut ssi = self.parse_software_system_instance(if has_ident { &ident } else { "" }, env)?;
                    if ssi.deployment_groups.is_none() && !inherited_groups.is_empty() {
                        ssi.deployment_groups = Some(inherited_groups.clone());
                    }
                    software_system_instances.push(ssi);
                } else if self.peek_word("infrastructurenode") {
                    self.advance();
                    let inf = self.parse_infrastructure_node(if has_ident { &ident } else { "" }, env)?;
                    infrastructure_nodes.push(inf);
                } else if self.peek_word("instanceof") {
                    // `instanceOf <ref>`: shorthand for containerInstance /
                    // softwareSystemInstance depending on what the ref names.
                    self.advance();
                    let (line, col) = self.current_pos();
                    let target = self.consume_string().unwrap_or_default();
                    let resolved = self.register.resolve(&target).cloned().or_else(|| {
                        target
                            .rsplit('.')
                            .next()
                            .and_then(|last| self.register.resolve(last).cloned())
                    });
                    let deployment_groups = self.consume_deployment_groups();
                    if self.peek_open_brace() {
                        self.advance();
                        self.skip_block();
                    }
                    match resolved {
                        Some((eid, ElementType::Container)) => {
                            let iid = self.next_id();
                            if has_ident {
                                self.register.register(&ident, iid.clone(), ElementType::ContainerInstance);
                            }
                            container_instances.push(ContainerInstance {
                                id: iid,
                                container_id: eid,
                                environment: Some(env.to_string()),
                                tags: merge_tags("Container Instance", None),
                                deployment_groups,
                                ..Default::default()
                            });
                        }
                        Some((eid, ElementType::SoftwareSystem)) => {
                            let iid = self.next_id();
                            if has_ident {
                                self.register.register(&ident, iid.clone(), ElementType::SoftwareSystemInstance);
                            }
                            software_system_instances.push(SoftwareSystemInstance {
                                id: iid,
                                software_system_id: eid,
                                environment: Some(env.to_string()),
                                tags: merge_tags("Software System Instance", None),
                                deployment_groups,
                                ..Default::default()
                            });
                        }
                        _ => {
                            return Err(ParseError::syntax(
                                line,
                                col,
                                format!(
                                    "instanceOf target '{}' does not resolve to a container or software system",
                                    target
                                ),
                            ));
                        }
                    }
                } else if self.peek_at_arrow_after_word() {
                    let src_pos = self.current_pos();
                    let src = self.consume_string().unwrap_or_default();
                    self.advance(); // ->
                    let dst_pos = self.current_pos();
                    let dst = self.consume_string().unwrap_or_default();
                    let desc = self.consume_string_if_not_brace();
                    let tech = self.consume_string_if_not_brace();
                    let rel_id = self.next_id();
                    // `this` refers to the enclosing deployment node.
                    let (src_id, src_port) = if src.eq_ignore_ascii_case("this") {
                        (id.clone(), None)
                    } else {
                        self.resolve_endpoint_tracked(&src, &rel_id, true, src_pos)
                    };
                    let (dst_id, dst_port) = if dst.eq_ignore_ascii_case("this") {
                        (id.clone(), None)
                    } else {
                        self.resolve_endpoint_tracked(&dst, &rel_id, false, dst_pos)
                    };
                    rels.push(Relationship {
                        id: rel_id,
                        source_id: src_id,
                        destination_id: dst_id,
                        source_port_id: src_port,
                        destination_port_id: dst_port,
                        description: desc,
                        technology: tech,
                        tags: Some("Relationship".to_string()),
                        ..Default::default()
                    });
                } else if self.peek_word("instances") {
                    self.advance();
                    node.instances = self
                        .consume_string()
                        .and_then(|s| s.parse::<i64>().ok())
                        .map(|n| serde_json::Value::Number(serde_json::Number::from(n)));
                } else if !has_ident && {
                    let paths = vec![identifier.to_string()];
                    self.try_parse_common_element_keyword(&mut node_extras, &id, &paths)?
                } {
                    // node attribute consumed
                } else if matches!(self.peek(), Some(Token::Directive(_))) {
                    self.advance();
                    self.skip_directive_args();
                } else if !self.sketch {
                    return Err(self.unknown_keyword_error(
                        "deploymentNode body",
                        &DEPLOYMENT_NODE_BODY_KEYWORDS,
                    ));
                } else {
                    self.advance(); // the keyword
                    if self.peek_open_brace() {
                        self.advance();
                        self.skip_block();
                    } else {
                        // consume one optional value token
                        let _ = self.consume_string();
                    }
                }
            }
            self.expect_close_brace()?;
            if node_extras.description.is_some() { node.description = node_extras.description; }
            if node_extras.technology.is_some()  { node.technology  = node_extras.technology; }
            if node_extras.url.is_some()         { node.url         = node_extras.url; }
            if !node_extras.tags_extra.is_empty() {
                let extra = node_extras.tags_extra.join(",");
                node.tags = Some(match node.tags.take() {
                    Some(t) => format!("{},{}", t, extra),
                    None => extra,
                });
            }
            if !node_extras.properties.is_empty() {
                node.properties.get_or_insert_with(HashMap::new).extend(node_extras.properties);
            }

            if !children.is_empty() {
                node.children = Some(children);
            }
            if !container_instances.is_empty() {
                node.container_instances = Some(container_instances);
            }
            if !software_system_instances.is_empty() {
                node.software_system_instances = Some(software_system_instances);
            }
            if !infrastructure_nodes.is_empty() {
                node.infrastructure_nodes = Some(infrastructure_nodes);
            }
            if !rels.is_empty() {
                node.relationships = Some(rels);
            }
        }

        Ok(node)
    }

    /// Consume trailing `deploymentGroup` identifiers on an instance declaration.
    fn consume_deployment_groups(&mut self) -> Option<Vec<String>> {
        let mut groups = Vec::new();
        while let Some(Token::Word(w)) = self.peek() {
            match self.deployment_group_names.get(&w.to_lowercase()) {
                Some(name) => {
                    groups.push(name.clone());
                    self.advance();
                }
                None => break,
            }
        }
        if groups.is_empty() { None } else { Some(groups) }
    }

    fn parse_container_instance(&mut self, identifier: &str, env: &str) -> Result<ContainerInstance, ParseError> {
        let id = self.next_id();
        let container_ref = self.consume_string().unwrap_or_default();
        let deployment_groups = self.consume_deployment_groups();
        let tags = self.consume_string_if_not_brace_or_kw();

        let container_id = self.register.resolve_id(&container_ref).unwrap_or(container_ref);

        if !identifier.is_empty() {
            self.register
                .register(identifier, id.clone(), ElementType::ContainerInstance);
        }

        let ci = ContainerInstance {
            id,
            container_id,
            environment: Some(env.to_string()),
            tags: merge_tags("Container Instance", tags),
            deployment_groups,
            ..Default::default()
        };

        if self.peek_open_brace() {
            self.advance();
            self.skip_block();
        }

        Ok(ci)
    }

    fn parse_software_system_instance(&mut self, identifier: &str, env: &str) -> Result<SoftwareSystemInstance, ParseError> {
        let id = self.next_id();
        let ss_ref = self.consume_string().unwrap_or_default();
        let deployment_groups = self.consume_deployment_groups();
        let tags = self.consume_string_if_not_brace_or_kw();

        let ss_id = self.register.resolve_id(&ss_ref).unwrap_or(ss_ref);

        if !identifier.is_empty() {
            self.register
                .register(identifier, id.clone(), ElementType::SoftwareSystemInstance);
        }

        let ssi = SoftwareSystemInstance {
            id,
            software_system_id: ss_id,
            environment: Some(env.to_string()),
            tags: merge_tags("Software System Instance", tags),
            deployment_groups,
            ..Default::default()
        };

        if self.peek_open_brace() {
            self.advance();
            self.skip_block();
        }

        Ok(ssi)
    }

    fn parse_infrastructure_node(&mut self, identifier: &str, env: &str) -> Result<InfrastructureNode, ParseError> {
        let id = self.next_id();
        let name = self.consume_string().unwrap_or_default();
        let description = self.consume_string_if_not_brace();
        let technology = self.consume_string_if_not_brace();
        let tags = self.consume_string_if_not_brace_or_kw();

        if !identifier.is_empty() {
            self.register
                .register(identifier, id.clone(), ElementType::InfrastructureNode);
        }

        let inf = InfrastructureNode {
            id,
            name,
            description,
            technology,
            tags: merge_tags("Infrastructure Node", tags),
            environment: Some(env.to_string()),
            ..Default::default()
        };

        if self.peek_open_brace() {
            self.advance();
            self.skip_block();
        }

        Ok(inf)
    }

    /// Parse an element block (relationships and attributes inside `{ }`).
    /// Returns the relationships found plus any element-level extras (status, etc.).
    fn parse_element_block(
        &mut self,
        source_id: &str,
        ident_paths: &[String],
    ) -> Result<(Vec<Relationship>, ElementExtras), ParseError> {
        let mut rels   = Vec::new();
        let mut extras = ElementExtras::default();
        while !self.peek_close_brace() && self.peek().is_some() {
            if self.peek_at_arrow_after_word() {
                let src_pos = self.current_pos();
                let src  = self.consume_string().unwrap_or_default();
                self.advance(); // ->
                let dst_pos = self.current_pos();
                let dst  = self.consume_string().unwrap_or_default();
                let desc = self.consume_string_if_not_brace();
                let tech = self.consume_string_if_not_brace();
                let rel_id = self.next_id();
                let uncertain = self.consume_uncertainty_marker();
                // `this` refers to the enclosing element (upstream DSL).
                let (src_id, src_port) = if src.eq_ignore_ascii_case("this") {
                    (source_id.to_string(), None)
                } else {
                    self.resolve_endpoint_tracked(&src, &rel_id, true, src_pos)
                };
                let (dst_id, dst_port) = if dst.eq_ignore_ascii_case("this") {
                    (source_id.to_string(), None)
                } else {
                    self.resolve_endpoint_tracked(&dst, &rel_id, false, dst_pos)
                };
                let mut rel = Relationship {
                    id: rel_id,
                    source_id: src_id,
                    destination_id: dst_id,
                    source_port_id: src_port,
                    destination_port_id: dst_port,
                    description: desc,
                    technology: tech,
                    tags: Some("Relationship".to_string()),
                    ..Default::default()
                };
                if self.peek_open_brace() {
                    self.parse_relationship_body(&mut rel)?;
                }
                if uncertain {
                    rel.tags = Some(match rel.tags.take() {
                        Some(t) => format!("{},Uncertain", t),
                        None => "Uncertain".to_string(),
                    });
                }
                rels.push(rel);
            } else if matches!(self.peek(), Some(Token::Arrow)) {
                let rel = self.parse_implicit_relationship(source_id)?;
                rels.push(rel);
            } else if self.try_parse_common_element_keyword(&mut extras, source_id, ident_paths)? {
                // element attribute consumed
            } else if matches!(self.peek(), Some(Token::Directive(d)) if d.eq_ignore_ascii_case("adrs") || d.eq_ignore_ascii_case("decisions")) {
                self.advance();
                let rel_path = self.consume_string().unwrap_or_default();
                if self.peek_open_brace() { self.advance(); self.skip_block(); }
                let mut decisions = self.import_adrs(&rel_path);
                for d in &mut decisions { d.element_id = Some(source_id.to_string()); }
                self.accumulated_decisions.extend(decisions);
            } else if matches!(self.peek(), Some(Token::Directive(_))) {
                self.advance();
                self.skip_directive_args();
            } else if !self.sketch {
                return Err(self.unknown_keyword_error("element body", &ELEMENT_BODY_KEYWORDS));
            } else {
                self.advance();
                self.skip_optional_block_or_value();
            }
        }
        Ok((rels, extras))
    }

    fn parse_relationship_in_model(&mut self, model: &mut Model) -> Result<String, ParseError> {
        let src_pos = self.current_pos();
        let src  = self.consume_string().unwrap_or_default();
        self.advance(); // ->
        let dst_pos = self.current_pos();
        let dst  = self.consume_string().unwrap_or_default();
        let desc = self.consume_string_if_not_brace();
        let tech = self.consume_string_if_not_brace_or_kw();
        let tags = self.consume_string_if_not_brace_or_kw();

        let uncertain = self.consume_uncertainty_marker();
        if self.sketch {
            self.vivify_placeholder(model, &src);
            self.vivify_placeholder(model, &dst);
        }
        let rel_id = self.next_id();
        let returned_rel_id = rel_id.clone();
        let src_unresolved = !src.is_empty() && !self.endpoint_resolves(&src);
        let (src_id, src_port) = self.resolve_endpoint_tracked(&src, &rel_id, true, src_pos);
        let (dst_id, dst_port) = self.resolve_endpoint_tracked(&dst, &rel_id, false, dst_pos);

        let mut rel = Relationship {
            id: rel_id,
            source_id: src_id.clone(),
            destination_id: dst_id,
            source_port_id: src_port,
            destination_port_id: dst_port,
            description: desc,
            technology: tech,
            tags: tags.or_else(|| Some("Relationship".to_string())),
            ..Default::default()
        };

        if self.peek_open_brace() {
            self.parse_relationship_body(&mut rel)?;
        }

        if uncertain {
            rel.tags = Some(match rel.tags.take() {
                Some(t) => format!("{},Uncertain", t),
                None => "Uncertain".to_string(),
            });
        }

        // Attach to the source element now, or defer until the end of the
        // model block when the source is a forward reference.
        if src_unresolved {
            self.deferred_rels.push(rel);
        } else {
            self.attach_relationship_to_element(model, &src_id, rel);
        }

        Ok(returned_rel_id)
    }

    /// Parse an implicit-source relationship inside an element body:
    /// `-> destination ["description" ["technology"]] [?] [{ ... }]`.
    /// The source is the enclosing element. Current token must be the arrow.
    fn parse_implicit_relationship(&mut self, source_id: &str) -> Result<Relationship, ParseError> {
        self.advance(); // ->
        let dst_pos = self.current_pos();
        let dst = self.consume_string().unwrap_or_default();
        let desc = self.consume_string_if_not_brace();
        let tech = self.consume_string_if_not_brace();
        let rel_id = self.next_id();
        let uncertain = self.consume_uncertainty_marker();
        let (dst_id, dst_port) = self.resolve_endpoint_tracked(&dst, &rel_id, false, dst_pos);
        let mut rel = Relationship {
            id: rel_id,
            source_id: source_id.to_string(),
            destination_id: dst_id,
            destination_port_id: dst_port,
            description: desc,
            technology: tech,
            tags: Some("Relationship".to_string()),
            ..Default::default()
        };
        if self.peek_open_brace() {
            self.parse_relationship_body(&mut rel)?;
        }
        if uncertain {
            rel.tags = Some(match rel.tags.take() {
                Some(t) => format!("{},Uncertain", t),
                None => "Uncertain".to_string(),
            });
        }
        Ok(rel)
    }

    fn attach_relationship_to_element(&self, model: &mut Model, source_id: &str, rel: Relationship) {
        // Try people
        if let Some(people) = &mut model.people {
            for p in people.iter_mut() {
                if p.id == source_id {
                    p.relationships.get_or_insert_with(Vec::new).push(rel);
                    return;
                }
            }
        }
        // Try software systems
        if let Some(systems) = &mut model.software_systems {
            for ss in systems.iter_mut() {
                if ss.id == source_id {
                    ss.relationships.get_or_insert_with(Vec::new).push(rel);
                    return;
                }
                // Try containers
                if let Some(containers) = &mut ss.containers {
                    for c in containers.iter_mut() {
                        if c.id == source_id {
                            c.relationships.get_or_insert_with(Vec::new).push(rel);
                            return;
                        }
                        if let Some(components) = &mut c.components {
                            for comp in components.iter_mut() {
                                if comp.id == source_id {
                                    comp.relationships.get_or_insert_with(Vec::new).push(rel);
                                    return;
                                }
                            }
                        }
                    }
                }
            }
        }
        // Try custom elements
        if let Some(elements) = &mut model.custom_elements {
            for e in elements.iter_mut() {
                if e.id == source_id {
                    e.relationships.get_or_insert_with(Vec::new).push(rel);
                    return;
                }
            }
        }
    }

    // ─── Views ──────────────────────────────────────────────────────────────────

    fn parse_views(&mut self, views: &mut ViewSet, model: &Model) -> Result<(), ParseError> {
        while !self.peek_close_brace() && self.peek().is_some() {
            match self.peek() {
                Some(Token::Word(w)) => {
                    let w = w.to_lowercase();
                    match w.as_str() {
                        "auto" => {
                            self.advance();
                            let spec = self.parse_auto_view_spec()?;
                            views.auto_views.get_or_insert_with(Vec::new).push(spec);
                        }
                        "systemlandscape" => {
                            self.advance();
                            let v = self.parse_system_landscape_view(model)?;
                            views.system_landscape_views.get_or_insert_with(Vec::new).push(v);
                        }
                        "systemcontext" => {
                            self.advance();
                            let v = self.parse_system_context_view(model)?;
                            views.system_context_views.get_or_insert_with(Vec::new).push(v);
                        }
                        "container" => {
                            self.advance();
                            let v = self.parse_container_view(model)?;
                            views.container_views.get_or_insert_with(Vec::new).push(v);
                        }
                        "component" => {
                            self.advance();
                            let v = self.parse_component_view(model)?;
                            views.component_views.get_or_insert_with(Vec::new).push(v);
                        }
                        "dynamic" => {
                            self.advance();
                            let v = self.parse_dynamic_view(model)?;
                            views.dynamic_views.get_or_insert_with(Vec::new).push(v);
                        }
                        "deployment" => {
                            self.advance();
                            let v = self.parse_deployment_view(model)?;
                            views.deployment_views.get_or_insert_with(Vec::new).push(v);
                        }
                        "filtered" => {
                            self.advance();
                            let v = self.parse_filtered_view()?;
                            views.filtered_views.get_or_insert_with(Vec::new).push(v);
                        }
                        "styles" => {
                            self.advance();
                            self.expect_open_brace()?;
                            let styles = self.parse_styles()?;
                            views.configuration
                                .get_or_insert_with(ViewConfiguration::default)
                                .styles = Some(styles);
                        }
                        "theme" | "themes" => {
                            self.advance();
                            let mut themes = Vec::new();
                            while matches!(self.peek(), Some(Token::Word(_)) | Some(Token::Quoted(_))) {
                                if let Some(t) = self.consume_string() {
                                    if t.eq_ignore_ascii_case("default") {
                                        themes.push("https://static.structurizr.com/themes/default/theme.json".to_string());
                                    } else {
                                        themes.push(t);
                                    }
                                }
                            }
                            views.configuration
                                .get_or_insert_with(ViewConfiguration::default)
                                .themes = Some(themes);
                        }
                        "branding" => {
                            self.advance();
                            self.expect_open_brace()?;
                            self.skip_block();
                        }
                        "properties" => {
                            self.advance();
                            self.expect_open_brace()?;
                            self.skip_block();
                        }
                        // Upstream view types we accept but do not render yet:
                        // consume positional args and an optional block.
                        "image" | "custom" => {
                            self.advance();
                            while matches!(
                                self.peek(),
                                Some(Token::Word(_)) | Some(Token::Quoted(_)) | Some(Token::TextBlock(_))
                            ) {
                                self.advance();
                            }
                            if self.peek_open_brace() {
                                self.advance();
                                self.skip_block();
                            }
                        }
                        _ => {
                            if !self.sketch {
                                return Err(self.unknown_keyword_error("views", &VIEWS_KEYWORDS));
                            }
                            self.advance();
                            while matches!(
                                self.peek(),
                                Some(Token::Word(_)) | Some(Token::Quoted(_)) | Some(Token::TextBlock(_))
                            ) {
                                self.advance();
                            }
                            if self.peek_open_brace() {
                                self.advance();
                                self.skip_block();
                            }
                        }
                    }
                }
                Some(Token::Directive(d)) => {
                    let d = d.clone();
                    self.advance();
                    match d.to_lowercase().as_str() {
                        "const" | "constant" | "var" => {
                            let name = self.consume_string().unwrap_or_default();
                            let value = self.consume_string().unwrap_or_default();
                            self.constants.insert(name, value);
                        }
                        _ => {
                            self.skip_directive_args();
                        }
                    }
                }
                Some(Token::CloseBrace) => break,
                _ => {
                    self.advance();
                }
            }
        }
        Ok(())
    }

    /// Parse an `auto ...` generated-view declaration (spec §6.3).
    ///
    /// Forms: `auto` | `auto focus <ref> [{ depth N  direction in|out|both
    /// splitBy kind|tag|layer  asof <milestone> }]` | `auto perspective <name|*>`
    /// | `auto layer <name>` | `auto slice <selector-expr>` | `auto paths <a> <b>`
    /// | `auto rollup [<partition>]` | `auto asof <m>` | `auto delta <m1> <m2>`
    /// | `auto lint`
    fn parse_auto_view_spec(&mut self) -> Result<AutoViewSpec, ParseError> {
        let mut spec = AutoViewSpec::default();

        let generator = match self.peek() {
            Some(Token::Word(w)) if matches!(
                w.to_lowercase().as_str(),
                "focus" | "perspective" | "layer" | "slice" | "paths" | "rollup"
                    | "asof" | "delta" | "lint"
            ) => {
                let g = w.to_lowercase();
                self.advance();
                g
            }
            _ => "default".to_string(),
        };
        spec.generator = generator.clone();

        match generator.as_str() {
            "focus" => {
                // Resolve DSL identifiers to element ids while the register is
                // alive; unknown refs pass through for name-based matching.
                spec.target = self.consume_bare_word_or_string().map(|t| self.resolve_identifier(&t));
                if self.peek_open_brace() {
                    self.advance();
                    while !self.peek_close_brace() && self.peek().is_some() {
                        if self.peek_word("depth") {
                            self.advance();
                            let (line, col) = self.current_pos();
                            let v = self.consume_bare_word_or_string().unwrap_or_default();
                            if v == "*" {
                                spec.depth = Some(u32::MAX);
                            } else {
                                spec.depth = Some(v.parse().map_err(|_| {
                                    ParseError::syntax(line, col, format!("depth must be a number or *, got: {}", v))
                                })?);
                            }
                        } else if self.peek_word("direction") {
                            self.advance();
                            let (line, col) = self.current_pos();
                            let v = self.consume_bare_word_or_string().unwrap_or_default().to_lowercase();
                            if !matches!(v.as_str(), "in" | "out" | "both") {
                                return Err(ParseError::syntax(line, col, format!("direction must be in|out|both, got: {}", v)));
                            }
                            spec.direction = Some(v);
                        } else if self.peek_word("splitby") {
                            self.advance();
                            let (line, col) = self.current_pos();
                            let v = self.consume_bare_word_or_string().unwrap_or_default().to_lowercase();
                            if !matches!(v.as_str(), "kind" | "tag" | "layer") {
                                return Err(ParseError::syntax(line, col, format!("splitBy must be kind|tag|layer, got: {}", v)));
                            }
                            spec.split_by = Some(v);
                        } else if self.peek_word("asof") {
                            self.advance();
                            spec.asof = self.consume_bare_word_or_string();
                        } else {
                            self.advance();
                            self.skip_optional_block_or_value();
                        }
                    }
                    self.expect_close_brace()?;
                }
            }
            "perspective" | "layer" | "asof" | "rollup" => {
                spec.target = self.consume_bare_word_or_string_same_line();
            }
            "paths" => {
                spec.target = self.consume_bare_word_or_string().map(|t| self.resolve_identifier(&t));
                spec.target2 = self.consume_bare_word_or_string().map(|t| self.resolve_identifier(&t));
            }
            "delta" => {
                // delta arguments are milestone names, never element refs
                spec.target = self.consume_bare_word_or_string();
                spec.target2 = self.consume_bare_word_or_string();
            }
            "slice" => {
                spec.expression = Some(self.reassemble_selector_expression_same_line());
            }
            _ => {} // default | lint take no arguments
        }

        Ok(spec)
    }

    /// Consume a bare word or string only if it is on the same line as the
    /// previously consumed token (so `auto rollup` on its own line does not
    /// swallow the next view keyword).
    fn consume_bare_word_or_string_same_line(&mut self) -> Option<String> {
        let last_line = self.last_consumed_line();
        let next_line = self.tokens.get(self.pos).map(|s| s.pos.line).unwrap_or(usize::MAX);
        if next_line != last_line {
            return None;
        }
        self.consume_bare_word_or_string()
    }

    /// Rebuild a selector expression string (structurizr-query syntax) from the
    /// DSL token stream: consume tokens while they remain on the starting line.
    /// `element.kind==container` lexes as Word/Equals/Equals/Word and is joined
    /// back together here.
    fn reassemble_selector_expression_same_line(&mut self) -> String {
        let line = self.last_consumed_line();
        let mut out = String::new();
        loop {
            let same_line = self
                .tokens
                .get(self.pos)
                .map(|s| s.pos.line == line)
                .unwrap_or(false);
            if !same_line {
                break;
            }
            match self.peek() {
                Some(Token::Word(w)) => {
                    out.push_str(w);
                    self.advance();
                }
                Some(Token::Equals) => {
                    out.push('=');
                    self.advance();
                }
                Some(Token::Arrow) => {
                    out.push_str("->");
                    self.advance();
                }
                Some(Token::Quoted(q)) => {
                    out.push('"');
                    out.push_str(q);
                    out.push('"');
                    self.advance();
                }
                _ => break,
            }
        }
        out
    }

    fn parse_system_landscape_view(&mut self, model: &Model) -> Result<SystemLandscapeView, ParseError> {
        let key = self.consume_string_if_not_brace_or_kw();
        let title = self.consume_string_if_not_brace_or_kw();
        let mut include_all = false;
        let mut explicit_includes: Vec<String> = Vec::new();
        let auto_layout = self.parse_view_block(&mut include_all, &mut explicit_includes)?;
        let mut view = SystemLandscapeView {
            key,
            title,
            automatic_layout: auto_layout,
            ..Default::default()
        };

        if include_all {
            self.populate_system_landscape_view(model, &mut view);
        }

        Ok(view)
    }

    fn parse_system_context_view(&mut self, model: &Model) -> Result<SystemContextView, ParseError> {
        let ss_ref = self.consume_string().unwrap_or_default();
        let ss_id = self.resolve_identifier(&ss_ref);
        let key = self.consume_string_if_not_brace_or_kw();
        let title = self.consume_string_if_not_brace_or_kw();
        let mut include_all = false;
        let mut explicit_includes: Vec<String> = Vec::new();
        let auto_layout = self.parse_view_block(&mut include_all, &mut explicit_includes)?;
        let mut view = SystemContextView {
            software_system_id: ss_id,
            key,
            title,
            automatic_layout: auto_layout,
            ..Default::default()
        };

        if include_all {
            self.populate_system_context_view(model, &mut view);
        } else if !explicit_includes.is_empty() {
            self.populate_view_with_explicit_includes(model, &explicit_includes,
                &mut view.element_views, &mut view.relationship_views);
        }

        Ok(view)
    }

    fn parse_container_view(&mut self, model: &Model) -> Result<ContainerView, ParseError> {
        let ss_ref = self.consume_string().unwrap_or_default();
        let ss_id = self.resolve_identifier(&ss_ref);
        let key = self.consume_string_if_not_brace_or_kw();
        let title = self.consume_string_if_not_brace_or_kw();
        let mut include_all = false;
        let mut explicit_includes: Vec<String> = Vec::new();
        let auto_layout = self.parse_view_block(&mut include_all, &mut explicit_includes)?;
        let mut view = ContainerView {
            software_system_id: ss_id,
            key,
            title,
            automatic_layout: auto_layout,
            ..Default::default()
        };

        if include_all {
            self.populate_container_view(model, &mut view);
        } else if !explicit_includes.is_empty() {
            self.populate_view_with_explicit_includes(model, &explicit_includes,
                &mut view.element_views, &mut view.relationship_views);
        }

        Ok(view)
    }

    fn parse_component_view(&mut self, model: &Model) -> Result<ComponentView, ParseError> {
        let cont_ref = self.consume_string().unwrap_or_default();
        let cont_id = self.resolve_identifier(&cont_ref);
        let key = self.consume_string_if_not_brace_or_kw();
        let title = self.consume_string_if_not_brace_or_kw();
        let mut include_all = false;
        let mut explicit_includes: Vec<String> = Vec::new();
        let auto_layout = self.parse_view_block(&mut include_all, &mut explicit_includes)?;
        let mut view = ComponentView {
            container_id: cont_id,
            key,
            title,
            automatic_layout: auto_layout,
            ..Default::default()
        };
        if include_all {
            self.populate_component_view(model, &mut view);
        } else if !explicit_includes.is_empty() {
            self.populate_view_with_explicit_includes(model, &explicit_includes,
                &mut view.element_views, &mut view.relationship_views);
        }
        Ok(view)
    }

    fn parse_dynamic_view(&mut self, model: &Model) -> Result<DynamicView, ParseError> {
        let elem_ref = self.consume_string().unwrap_or_default();
        let elem_id = if elem_ref == "*" {
            None
        } else {
            Some(self.resolve_identifier(&elem_ref))
        };
        let key = self.consume_string_if_not_brace_or_kw();
        let title = self.consume_string_if_not_brace_or_kw();
        let description = self.consume_string_if_not_brace_or_kw();

        let (element_views, relationship_views, auto_layout) =
            self.parse_dynamic_view_block(model);

        Ok(DynamicView {
            element_id: elem_id,
            key,
            title,
            description,
            element_views: if element_views.is_empty() { None } else { Some(element_views) },
            relationship_views: if relationship_views.is_empty() { None } else { Some(relationship_views) },
            automatic_layout: auto_layout,
            ..Default::default()
        })
    }

    /// Parse a dynamic view block, collecting `source -> dest "desc"` steps.
    fn parse_dynamic_view_block(
        &mut self,
        model: &Model,
    ) -> (Vec<ElementView>, Vec<RelationshipView>, Option<AutomaticLayout>) {
        let mut element_set: HashSet<String> = HashSet::new();
        let mut rel_views: Vec<RelationshipView> = Vec::new();
        let mut auto_layout = None;
        let mut order = 1u32;
        // Depth of anonymous `{ ... }` parallel-sequence groups; their steps
        // are parsed like any other (ordering nuances are not modelled).
        let mut parallel_depth = 0usize;

        if !self.peek_open_brace() {
            return (Vec::new(), Vec::new(), None);
        }
        self.advance();

        while self.peek().is_some() {
            if self.peek_close_brace() {
                if parallel_depth > 0 {
                    self.advance();
                    parallel_depth -= 1;
                    continue;
                }
                break;
            }
            match self.peek() {
                Some(Token::OpenBrace) => {
                    self.advance();
                    parallel_depth += 1;
                }
                Some(Token::Word(w)) if w.eq_ignore_ascii_case("autolayout") || w.eq_ignore_ascii_case("autoLayout") => {
                    self.advance();
                    let direction = match self.peek() {
                        Some(Token::Word(w)) if is_autolayout_direction(w) => self.consume_string(),
                        _ => None,
                    };
                    let rank_sep = match self.peek() {
                        Some(Token::Word(w)) if w.parse::<i32>().is_ok() => {
                            self.consume_string().and_then(|s| s.parse::<i32>().ok())
                        }
                        _ => None,
                    };
                    let node_sep = match self.peek() {
                        Some(Token::Word(w)) if w.parse::<i32>().is_ok() => {
                            self.consume_string().and_then(|s| s.parse::<i32>().ok())
                        }
                        _ => None,
                    };
                    auto_layout = Some(AutomaticLayout {
                        implementation: Some("Graphviz".to_string()),
                        rank_direction: direction,
                        rank_separation: rank_sep,
                        node_separation: node_sep,
                        ..Default::default()
                    });
                }
                Some(Token::Word(w)) if w.eq_ignore_ascii_case("description") || w.eq_ignore_ascii_case("title") || w.eq_ignore_ascii_case("properties") => {
                    self.advance();
                    if self.peek_open_brace() {
                        self.advance();
                        self.skip_block();
                    } else {
                        let _ = self.consume_string();
                    }
                }
                _ if self.peek_at_arrow_after_word() => {
                    // `sourceId -> destId "desc" "tech"` step
                    let src_ref = self.consume_string().unwrap_or_default();
                    self.advance(); // ->
                    let dst_ref = self.consume_string().unwrap_or_default();
                    let step_desc = self.consume_string_if_not_brace();
                    let _step_tech = self.consume_string_if_not_brace();

                    let src_id = self.resolve_identifier(&src_ref);
                    let dst_id = self.resolve_identifier(&dst_ref);

                    element_set.insert(src_id.clone());
                    element_set.insert(dst_id.clone());

                    // Find relationship ID in the model between src and dst
                    let rel_id = Self::find_relationship_id(model, &src_id, &dst_id);
                    if let Some(id) = rel_id {
                        rel_views.push(RelationshipView {
                            id,
                            order: Some(order.to_string()),
                            description: step_desc,
                            ..Default::default()
                        });
                    } else {
                        // No existing relationship found; create a placeholder with a
                        // synthetic ID so the view can still record the step.
                        rel_views.push(RelationshipView {
                            id: format!("dyn-{}-{}", src_id, dst_id),
                            order: Some(order.to_string()),
                            description: step_desc,
                            ..Default::default()
                        });
                    }
                    order += 1;
                }
                _ => {
                    self.advance();
                }
            }
        }
        if self.peek_close_brace() {
            self.advance();
        }

        let element_views = element_set
            .into_iter()
            .map(|id| ElementView { id, ..Default::default() })
            .collect();

        (element_views, rel_views, auto_layout)
    }

    /// Search the model for a relationship between `src_id` and `dst_id` and
    /// return its ID, if found.
    fn find_relationship_id(model: &Model, src_id: &str, dst_id: &str) -> Option<String> {
        fn check_rels(rels: &Option<Vec<Relationship>>, src: &str, dst: &str) -> Option<String> {
            rels.as_ref()?.iter().find(|r| r.source_id == src && r.destination_id == dst).map(|r| r.id.clone())
        }

        if let Some(people) = &model.people {
            for p in people {
                if let Some(id) = check_rels(&p.relationships, src_id, dst_id) { return Some(id); }
            }
        }
        if let Some(systems) = &model.software_systems {
            for ss in systems {
                if let Some(id) = check_rels(&ss.relationships, src_id, dst_id) { return Some(id); }
                if let Some(containers) = &ss.containers {
                    for c in containers {
                        if let Some(id) = check_rels(&c.relationships, src_id, dst_id) { return Some(id); }
                        if let Some(components) = &c.components {
                            for comp in components {
                                if let Some(id) = check_rels(&comp.relationships, src_id, dst_id) { return Some(id); }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn parse_deployment_view(&mut self, model: &Model) -> Result<DeploymentView, ParseError> {
        let scope_ref = self.consume_string().unwrap_or_default();
        let scope_id = if scope_ref == "*" {
            None
        } else {
            Some(self.resolve_identifier(&scope_ref))
        };
        let env = self.consume_string().unwrap_or_default();
        let key = self.consume_string_if_not_brace_or_kw();
        let title = self.consume_string_if_not_brace_or_kw();
        let mut include_all = false;
        let auto_layout = self.parse_optional_view_block(&mut include_all)?;
        let mut view = DeploymentView {
            software_system_id: scope_id,
            environment: env,
            key,
            title,
            automatic_layout: auto_layout,
            ..Default::default()
        };
        if include_all {
            self.populate_deployment_view(model, &mut view);
        }
        Ok(view)
    }

    fn parse_filtered_view(&mut self) -> Result<FilteredView, ParseError> {
        let base_key = self.consume_string().unwrap_or_default();
        let mode = self.consume_string().unwrap_or_else(|| "Include".to_string());
        let key = self.consume_string_if_not_brace_or_kw();
        let title = self.consume_string_if_not_brace_or_kw();

        if self.peek_open_brace() {
            self.advance();
            self.skip_block();
        }

        Ok(FilteredView {
            base_view_key: base_key,
            mode,
            key,
            title,
            ..Default::default()
        })
    }

    /// Parse a view block (inside `{ }`), return automatic layout if present.
    fn parse_optional_view_block(&mut self, include_all: &mut bool) -> Result<Option<AutomaticLayout>, ParseError> {
        let mut ignored = Vec::new();
        self.parse_view_block_inner(include_all, &mut ignored)
    }

    /// Full view-block parser that also collects explicit include specs:
    /// - `include *` sets `include_all = true`
    /// - `include ident` adds `ident` to `explicit_includes`
    /// - `include ->ident->` adds the neighborhood marker to `explicit_includes`
    fn parse_view_block(
        &mut self,
        include_all: &mut bool,
        explicit_includes: &mut Vec<String>,
    ) -> Result<Option<AutomaticLayout>, ParseError> {
        self.parse_view_block_inner(include_all, explicit_includes)
    }

    fn parse_view_block_inner(
        &mut self,
        include_all: &mut bool,
        explicit_includes: &mut Vec<String>,
    ) -> Result<Option<AutomaticLayout>, ParseError> {
        if !self.peek_open_brace() {
            return Ok(None);
        }
        self.advance();
        let mut auto_layout = None;
        let mut depth = 1i32;

        while self.peek().is_some() {
            match self.peek() {
                Some(Token::OpenBrace) => {
                    depth += 1;
                    self.advance();
                }
                Some(Token::CloseBrace) => {
                    depth -= 1;
                    self.advance();
                    if depth == 0 {
                        break;
                    }
                }
                Some(Token::Word(w)) if w.eq_ignore_ascii_case("autolayout") || w.eq_ignore_ascii_case("autoLayout") => {
                    self.advance();
                    let direction = match self.peek() {
                        Some(Token::Word(w)) if is_autolayout_direction(w) => {
                            self.consume_string()
                        }
                        _ => None,
                    };
                    let rank_sep = match self.peek() {
                        Some(Token::Word(w)) if w.parse::<i32>().is_ok() => {
                            self.consume_string().and_then(|s| s.parse::<i32>().ok())
                        }
                        _ => None,
                    };
                    let node_sep = match self.peek() {
                        Some(Token::Word(w)) if w.parse::<i32>().is_ok() => {
                            self.consume_string().and_then(|s| s.parse::<i32>().ok())
                        }
                        _ => None,
                    };
                    auto_layout = Some(AutomaticLayout {
                        implementation: Some("Graphviz".to_string()),
                        rank_direction: direction,
                        rank_separation: rank_sep,
                        node_separation: node_sep,
                        ..Default::default()
                    });
                }
                Some(Token::Word(w)) if w.eq_ignore_ascii_case("include") => {
                    self.advance();
                    // Parse the include argument(s).
                    // Handles: `include *`, `include ident`, `include ->ident->`
                    loop {
                        // Leading `->` means neighborhood syntax: `->ident->`
                        if matches!(self.peek(), Some(Token::Arrow)) {
                            self.advance(); // consume leading `->`
                            // Collect the identifier (may be dotted like `ss.container`)
                            if let Some(ident) = self.consume_string() {
                                // Trailing `->` is optional but expected
                                if matches!(self.peek(), Some(Token::Arrow)) {
                                    self.advance();
                                }
                                // Mark as neighborhood with a `->` prefix so callers
                                // can distinguish it from a plain element include.
                                explicit_includes.push(format!("->{}", ident));
                            }
                        } else if matches!(self.peek(), Some(Token::Word(_)) | Some(Token::Quoted(_))) {
                            let is_kw = matches!(
                                self.peek(),
                                Some(Token::Word(w)) if is_view_block_keyword(w)
                            );
                            if is_kw { break; }
                            if let Some(token) = self.consume_string() {
                                if token == "*" {
                                    *include_all = true;
                                } else {
                                    explicit_includes.push(token);
                                }
                            }
                        } else {
                            break;
                        }
                    }
                }
                _ => {
                    self.advance();
                }
            }
        }
        Ok(auto_layout)
    }

    fn collect_system_landscape_ids(&self, model: &Model) -> HashSet<String> {
        let mut ids = HashSet::new();

        if let Some(people) = &model.people {
            for p in people {
                ids.insert(p.id.clone());
            }
        }

        if let Some(systems) = &model.software_systems {
            for ss in systems {
                ids.insert(ss.id.clone());
            }
        }

        ids
    }

    /// Build a set of element IDs that are components (used to filter them out
    /// from container/context views where they should not appear as standalone nodes).
    fn collect_component_id_set(&self, model: &Model) -> HashSet<String> {
        let mut ids = HashSet::new();
        if let Some(systems) = &model.software_systems {
            for ss in systems {
                if let Some(containers) = &ss.containers {
                    for c in containers {
                        if let Some(components) = &c.components {
                            for comp in components {
                                ids.insert(comp.id.clone());
                            }
                        }
                    }
                }
            }
        }
        ids
    }

    /// Collect element IDs for a system context view.
    /// Includes the target software system itself plus any person/system that
    /// has a direct relationship to or from it.
    fn collect_system_context_view_ids(&self, model: &Model, software_system_id: &str) -> HashSet<String> {
        let mut ids = HashSet::new();
        ids.insert(software_system_id.to_string());

        fn add_if_related(
            relationships: &Option<Vec<Relationship>>,
            target_id: &str,
            ids: &mut HashSet<String>,
        ) {
            if let Some(rels) = relationships {
                for rel in rels {
                    if rel.source_id == target_id {
                        ids.insert(rel.destination_id.clone());
                    } else if rel.destination_id == target_id {
                        ids.insert(rel.source_id.clone());
                    }
                }
            }
        }

        if let Some(people) = &model.people {
            for p in people {
                add_if_related(&p.relationships, software_system_id, &mut ids);
            }
        }
        if let Some(systems) = &model.software_systems {
            for ss in systems {
                add_if_related(&ss.relationships, software_system_id, &mut ids);
            }
        }

        ids
    }

    /// Collect element IDs for a container view.
    ///
    /// - All containers of the target software system are always included.
    /// - External people, software systems, and containers of other systems that
    ///   have a relationship with any element inside the target system (including
    ///   component-level relationships) are included.
    /// - Components are never included (they live in the component view).
    /// - The target software system itself is NOT included (it renders as the
    ///   boundary via `softwareSystemId`).
    fn collect_container_view_ids(&self, model: &Model, software_system_id: &str) -> HashSet<String> {
        // Build the set of all IDs that are INTERNAL to the target SS.
        let mut internal_ids: HashSet<String> = HashSet::new();
        internal_ids.insert(software_system_id.to_string());

        let mut container_ids: HashSet<String> = HashSet::new();
        if let Some(systems) = &model.software_systems {
            for ss in systems {
                if ss.id == software_system_id {
                    if let Some(containers) = &ss.containers {
                        for c in containers {
                            container_ids.insert(c.id.clone());
                            internal_ids.insert(c.id.clone());
                            if let Some(components) = &c.components {
                                for comp in components {
                                    internal_ids.insert(comp.id.clone());
                                }
                            }
                        }
                    }
                }
            }
        }

        // Components must never appear as top-level elements in a container view.
        let component_ids = self.collect_component_id_set(model);

        let mut external_ids: HashSet<String> = HashSet::new();

        // Helper: if rel crosses the boundary (one side internal, other external),
        // add the external side (unless it's a component).
        fn maybe_add(
            rel_source: &str,
            rel_dest: &str,
            internal_ids: &HashSet<String>,
            component_ids: &HashSet<String>,
            external_ids: &mut HashSet<String>,
        ) {
            if internal_ids.contains(rel_source) && !internal_ids.contains(rel_dest) {
                if !component_ids.contains(rel_dest) {
                    external_ids.insert(rel_dest.to_string());
                }
            } else if internal_ids.contains(rel_dest) && !internal_ids.contains(rel_source)
                && !component_ids.contains(rel_source) {
                    external_ids.insert(rel_source.to_string());
                }
        }

        fn scan_rels(
            relationships: &Option<Vec<Relationship>>,
            internal_ids: &HashSet<String>,
            component_ids: &HashSet<String>,
            external_ids: &mut HashSet<String>,
        ) {
            if let Some(rels) = relationships {
                for rel in rels {
                    maybe_add(&rel.source_id, &rel.destination_id, internal_ids, component_ids, external_ids);
                }
            }
        }

        if let Some(people) = &model.people {
            for p in people {
                scan_rels(&p.relationships, &internal_ids, &component_ids, &mut external_ids);
            }
        }
        if let Some(systems) = &model.software_systems {
            for ss in systems {
                scan_rels(&ss.relationships, &internal_ids, &component_ids, &mut external_ids);
                if let Some(containers) = &ss.containers {
                    for c in containers {
                        scan_rels(&c.relationships, &internal_ids, &component_ids, &mut external_ids);
                        if let Some(components) = &c.components {
                            for comp in components {
                                scan_rels(&comp.relationships, &internal_ids, &component_ids, &mut external_ids);
                            }
                        }
                    }
                }
            }
        }

        // Result = containers of target SS + external elements
        let mut ids = container_ids;
        ids.extend(external_ids);
        ids
    }

    /// Collect all element IDs for a component view scoped to `container_id`.
    /// Includes: all components of that container, plus external people/systems/containers
    /// that have a direct relationship with any of those components.
    /// Does NOT include the container itself (rendered via `containerId` field).
    fn collect_component_view_ids(&self, model: &Model, container_id: &str) -> HashSet<String> {
        let mut component_ids: HashSet<String> = HashSet::new();
        if let Some(systems) = &model.software_systems {
            for ss in systems {
                if let Some(containers) = &ss.containers {
                    for c in containers {
                        if c.id == container_id {
                            if let Some(components) = &c.components {
                                for comp in components {
                                    component_ids.insert(comp.id.clone());
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut ids = component_ids.clone();
        // component_ids is the "internal" set for filtering
        let internal_ids = component_ids.clone();

        fn related_elements(
            relationships: &Option<Vec<Relationship>>,
            internal_ids: &HashSet<String>,
            container_id: &str,
            ids: &mut HashSet<String>,
        ) {
            if let Some(rels) = relationships {
                for rel in rels {
                    if internal_ids.contains(&rel.source_id)
                        && !internal_ids.contains(&rel.destination_id)
                        && rel.destination_id != container_id
                    {
                        ids.insert(rel.destination_id.clone());
                    } else if internal_ids.contains(&rel.destination_id)
                        && !internal_ids.contains(&rel.source_id)
                        && rel.source_id != container_id
                    {
                        ids.insert(rel.source_id.clone());
                    }
                }
            }
        }

        if let Some(people) = &model.people {
            for p in people {
                related_elements(&p.relationships, &internal_ids, container_id, &mut ids);
            }
        }
        if let Some(systems) = &model.software_systems {
            for ss in systems {
                related_elements(&ss.relationships, &internal_ids, container_id, &mut ids);
                if let Some(containers) = &ss.containers {
                    for c in containers {
                        related_elements(&c.relationships, &internal_ids, container_id, &mut ids);
                        if let Some(components) = &c.components {
                            for comp in components {
                                related_elements(&comp.relationships, &internal_ids, container_id, &mut ids);
                            }
                        }
                    }
                }
            }
        }

        ids
    }

    /// Collect all deployment node IDs for a deployment view scoped to `environment`.
    fn collect_deployment_view_ids(&self, model: &Model, environment: &str) -> HashSet<String> {
        let mut ids = HashSet::new();
        if let Some(nodes) = &model.deployment_nodes {
            for node in nodes {
                if node.environment.as_deref().unwrap_or("") == environment
                    || environment.is_empty()
                {
                    self.collect_deployment_node_ids_recursive(node, &mut ids);
                }
            }
        }
        ids
    }

    fn collect_deployment_node_ids_recursive(&self, node: &DeploymentNode, ids: &mut HashSet<String>) {
        ids.insert(node.id.clone());
        if let Some(children) = &node.children {
            for child in children {
                self.collect_deployment_node_ids_recursive(child, ids);
            }
        }
        if let Some(instances) = &node.container_instances {
            for inst in instances {
                ids.insert(inst.id.clone());
            }
        }
        if let Some(instances) = &node.software_system_instances {
            for inst in instances {
                ids.insert(inst.id.clone());
            }
        }
        if let Some(nodes) = &node.infrastructure_nodes {
            for inf in nodes {
                ids.insert(inf.id.clone());
            }
        }
    }

    /// Collect the relationships that appear in a view over the element set `ids`.
    ///
    /// Two passes:
    /// 1. Relationships whose endpoints are *both* directly in `ids` — the
    ///    same-level relationships, exactly as before.
    /// 2. Implied relationships: every remaining relationship is lifted by
    ///    replacing each endpoint with its nearest ancestor present in `ids`
    ///    (component → container → software system). A lifted relationship is
    ///    added only if the resulting pair is not already connected, so a
    ///    container view surfaces relationships defined at component level
    ///    (e.g. a component's `-> otherContainer`) collapsed to the container
    ///    level, mirroring upstream Structurizr's implied relationships.
    fn collect_relationship_view_ids(&self, model: &Model, ids: &HashSet<String>) -> Vec<RelationshipView> {
        let parents = self.build_parent_map(model);
        let lift = |id: &str| -> Option<String> {
            let mut cur = id.to_string();
            loop {
                if ids.contains(&cur) {
                    return Some(cur);
                }
                cur = parents.get(&cur)?.clone();
            }
        };

        // Gather every relationship in the model, at any nesting level.
        let mut all: Vec<&Relationship> = Vec::new();
        for p in model.people.iter().flatten() {
            all.extend(p.relationships.iter().flatten());
        }
        for ss in model.software_systems.iter().flatten() {
            all.extend(ss.relationships.iter().flatten());
            for c in ss.containers.iter().flatten() {
                all.extend(c.relationships.iter().flatten());
                for comp in c.components.iter().flatten() {
                    all.extend(comp.relationships.iter().flatten());
                }
            }
        }
        fn walk_node<'a>(node: &'a DeploymentNode, all: &mut Vec<&'a Relationship>) {
            all.extend(node.relationships.iter().flatten());
            for ci in node.container_instances.iter().flatten() {
                all.extend(ci.relationships.iter().flatten());
            }
            for ssi in node.software_system_instances.iter().flatten() {
                all.extend(ssi.relationships.iter().flatten());
            }
            for inf in node.infrastructure_nodes.iter().flatten() {
                all.extend(inf.relationships.iter().flatten());
            }
            for child in node.children.iter().flatten() {
                walk_node(child, all);
            }
        }
        for node in model.deployment_nodes.iter().flatten() {
            walk_node(node, &mut all);
        }

        let mut seen_ids: HashSet<String> = HashSet::new();
        let mut seen_pairs: HashSet<(String, String)> = HashSet::new();
        let mut out = Vec::new();

        // Pass 1: same-level relationships (both endpoints directly visible).
        for rel in &all {
            if ids.contains(&rel.source_id)
                && ids.contains(&rel.destination_id)
                && seen_ids.insert(rel.id.clone())
            {
                seen_pairs.insert((rel.source_id.clone(), rel.destination_id.clone()));
                out.push(RelationshipView {
                    id: rel.id.clone(),
                    ..Default::default()
                });
            }
        }

        // Pass 2: implied relationships lifted to the nearest visible ancestor.
        for rel in &all {
            if seen_ids.contains(&rel.id) {
                continue;
            }
            let (Some(src), Some(dst)) = (lift(&rel.source_id), lift(&rel.destination_id)) else {
                continue;
            };
            if src == dst || !seen_pairs.insert((src, dst)) {
                continue;
            }
            seen_ids.insert(rel.id.clone());
            out.push(RelationshipView {
                id: rel.id.clone(),
                ..Default::default()
            });
        }

        out
    }

    /// Map each container/component id to its parent element id
    /// (component → container, container → software system).
    fn build_parent_map(&self, model: &Model) -> HashMap<String, String> {
        let mut map = HashMap::new();
        for ss in model.software_systems.iter().flatten() {
            for c in ss.containers.iter().flatten() {
                map.insert(c.id.clone(), ss.id.clone());
                for comp in c.components.iter().flatten() {
                    map.insert(comp.id.clone(), c.id.clone());
                }
            }
        }
        map
    }

    fn populate_system_landscape_view(&self, model: &Model, view: &mut SystemLandscapeView) {
        let ids = self.collect_system_landscape_ids(model);
        view.element_views = Some(
            ids.iter()
                .map(|id| ElementView {
                    id: id.clone(),
                    ..Default::default()
                })
                .collect(),
        );
        view.relationship_views = Some(self.collect_relationship_view_ids(model, &ids));
    }

    fn populate_system_context_view(&self, model: &Model, view: &mut SystemContextView) {
        let ids = self.collect_system_context_view_ids(model, &view.software_system_id);
        view.element_views = Some(
            ids.iter()
                .map(|id| ElementView {
                    id: id.clone(),
                    ..Default::default()
                })
                .collect(),
        );
        view.relationship_views = Some(self.collect_relationship_view_ids(model, &ids));
    }

    fn populate_container_view(&self, model: &Model, view: &mut ContainerView) {
        let ids = self.collect_container_view_ids(model, &view.software_system_id);
        view.element_views = Some(
            ids.iter()
                .map(|id| ElementView {
                    id: id.clone(),
                    ..Default::default()
                })
                .collect(),
        );
        view.relationship_views = Some(self.collect_relationship_view_ids(model, &ids));
    }

    fn populate_component_view(&self, model: &Model, view: &mut ComponentView) {
        let ids = self.collect_component_view_ids(model, &view.container_id);
        view.element_views = Some(
            ids.iter()
                .map(|id| ElementView {
                    id: id.clone(),
                    ..Default::default()
                })
                .collect(),
        );
        view.relationship_views = Some(self.collect_relationship_view_ids(model, &ids));
    }

    fn populate_deployment_view(&self, model: &Model, view: &mut DeploymentView) {
        let ids = self.collect_deployment_view_ids(model, &view.environment);
        view.element_views = Some(
            ids.iter()
                .map(|id| ElementView {
                    id: id.clone(),
                    ..Default::default()
                })
                .collect(),
        );
        view.relationship_views = Some(self.collect_relationship_view_ids(model, &ids));
    }

    /// Populate element_views and relationship_views from a list of explicit include specs.
    ///
    /// Each spec is either:
    /// - A plain identifier string (e.g. `"person_apple"`) meaning include that element directly.
    /// - A string with a `->` prefix (e.g. `"->softwareSystem.service1"`) meaning include the
    ///   element AND all elements directly related to it (neighborhood syntax).
    fn populate_view_with_explicit_includes(
        &self,
        model: &Model,
        specs: &[String],
        element_views: &mut Option<Vec<ElementView>>,
        relationship_views: &mut Option<Vec<RelationshipView>>,
    ) {
        let mut ids: HashSet<String> = HashSet::new();

        for spec in specs {
            if let Some(ident) = spec.strip_prefix("->") {
                // Neighborhood: include the element itself + all directly related elements.
                // If the identifier resolves to a group (synthetic ID), expand to all children
                // of that group (all identifiers with the group as prefix) and include their
                // neighborhoods instead.
                let center_id = self.resolve_identifier(ident);
                if center_id.starts_with("group:") {
                    // Expand: include all children of this group + their neighborhoods
                    let children = self.register.children_of(ident);
                    for child_id in &children {
                        if child_id.starts_with("group:") { continue; }
                        ids.insert(child_id.clone());
                        self.collect_neighborhood_ids(model, child_id, &mut ids);
                    }
                } else {
                    ids.insert(center_id.clone());
                    self.collect_neighborhood_ids(model, &center_id, &mut ids);
                }
            } else {
                let resolved = self.resolve_identifier(spec);
                ids.insert(resolved);
            }
        }

        // Remove any group synthetic IDs (they're not real elements)
        ids.retain(|id| !id.starts_with("group:"));

        *element_views = Some(
            ids.iter()
                .map(|id| ElementView { id: id.clone(), ..Default::default() })
                .collect(),
        );
        *relationship_views = Some(self.collect_relationship_view_ids(model, &ids));
    }

    /// Add all elements that have a direct relationship to/from `center_id` into `ids`.
    fn collect_neighborhood_ids(&self, model: &Model, center_id: &str, ids: &mut HashSet<String>) {
        fn check_rels(rels: &Option<Vec<Relationship>>, center: &str, ids: &mut HashSet<String>) {
            if let Some(rels) = rels {
                for rel in rels {
                    if rel.source_id == center {
                        ids.insert(rel.destination_id.clone());
                    } else if rel.destination_id == center {
                        ids.insert(rel.source_id.clone());
                    }
                }
            }
        }

        if let Some(people) = &model.people {
            for p in people {
                check_rels(&p.relationships, center_id, ids);
            }
        }
        if let Some(systems) = &model.software_systems {
            for ss in systems {
                check_rels(&ss.relationships, center_id, ids);
                if let Some(containers) = &ss.containers {
                    for c in containers {
                        check_rels(&c.relationships, center_id, ids);
                        if let Some(components) = &c.components {
                            for comp in components {
                                check_rels(&comp.relationships, center_id, ids);
                            }
                        }
                    }
                }
            }
        }
    }


    fn parse_styles(&mut self) -> Result<Styles, ParseError> {
        let mut elements = Vec::new();
        let mut relationships = Vec::new();

        while !self.peek_close_brace() && self.peek().is_some() {
            match self.peek() {
                Some(Token::Word(w)) => {
                    let w = w.to_lowercase();
                    match w.as_str() {
                        "element" => {
                            self.advance();
                            let tag = self.consume_string().unwrap_or_default();
                            let style = self.parse_element_style(tag)?;
                            elements.push(style);
                        }
                        "relationship" => {
                            self.advance();
                            let tag = self.consume_string().unwrap_or_default();
                            let style = self.parse_relationship_style(tag)?;
                            relationships.push(style);
                        }
                        _ => {
                            self.advance();
                            self.skip_optional_block_or_value();
                        }
                    }
                }
                _ => {
                    self.advance();
                }
            }
        }
        self.expect_close_brace()?;

        Ok(Styles {
            elements: if elements.is_empty() { None } else { Some(elements) },
            relationships: if relationships.is_empty() { None } else { Some(relationships) },
        })
    }

    fn parse_element_style(&mut self, tag: String) -> Result<ElementStyle, ParseError> {
        let mut style = ElementStyle {
            tag,
            ..Default::default()
        };

        if self.peek_open_brace() {
            self.advance();
            while !self.peek_close_brace() && self.peek().is_some() {
                let key = self.consume_string().unwrap_or_default().to_lowercase();
                let value = self.consume_string().unwrap_or_default();
                match key.as_str() {
                    "shape" => style.shape = Some(canonicalize_shape(&value)),
                    "background" => style.background = Some(value),
                    "color" | "colour" => style.color = Some(value),
                    "stroke" => style.stroke = Some(value),
                    "fontsize" => style.font_size = value.parse().ok(),
                    "border" => style.border = Some(value),
                    "opacity" => style.opacity = value.parse().ok(),
                    "width" => style.width = value.parse().ok(),
                    "height" => style.height = value.parse().ok(),
                    _ => {}
                }
            }
            self.expect_close_brace()?;
        }

        Ok(style)
    }

    fn parse_relationship_style(&mut self, tag: String) -> Result<RelationshipStyle, ParseError> {
        let mut style = RelationshipStyle {
            tag,
            ..Default::default()
        };

        if self.peek_open_brace() {
            self.advance();
            while !self.peek_close_brace() && self.peek().is_some() {
                let key = self.consume_string().unwrap_or_default().to_lowercase();
                let value = self.consume_string().unwrap_or_default();
                match key.as_str() {
                    "thickness" => style.thickness = value.parse().ok(),
                    "color" | "colour" => style.color = Some(value),
                    "fontsize" => style.font_size = value.parse().ok(),
                    "linestyle" => style.line_style = Some(value),
                    "routing" => style.routing = Some(value),
                    "opacity" => style.opacity = value.parse().ok(),
                    "dashed" => style.dashed = Some(value.eq_ignore_ascii_case("true")),
                    "position" => style.position = value.parse().ok(),
                    _ => {}
                }
            }
            self.expect_close_brace()?;
        }

        Ok(style)
    }

    // ─── Configuration ──────────────────────────────────────────────────────────

    fn parse_configuration(&mut self) -> Result<WorkspaceConfiguration, ParseError> {
        let mut cfg = WorkspaceConfiguration::default();
        while !self.peek_close_brace() && self.peek().is_some() {
            let key = self.consume_string().unwrap_or_default().to_lowercase();
            match key.as_str() {
                "scope" => cfg.scope = self.consume_string(),
                "visibility" => cfg.visibility = self.consume_string(),
                "}" => break,
                _ => {
                    self.skip_optional_block_or_value();
                }
            }
        }
        self.expect_close_brace()?;
        Ok(cfg)
    }

    // ─── Helpers ────────────────────────────────────────────────────────────────

    fn consume_string_if_not_brace_or_kw(&mut self) -> Option<String> {
        match self.peek() {
            Some(Token::Quoted(_)) | Some(Token::TextBlock(_)) => self.consume_string(),
            // A bare word is a positional value (e.g. an unquoted tag) only when
            // it cannot be the start of the next statement: not a keyword or
            // declared alias, not numeric (instance counts), not `?`, and not
            // followed by `=` or `->`.
            Some(Token::Word(w)) => {
                let lower = w.to_lowercase();
                let identifier_like = w.chars().next().is_some_and(|c| c.is_alphanumeric() || c == '_');
                let starts_statement = is_reserved_body_word(&lower)
                    || self.kind_aliases.contains_key(&lower)
                    || w.parse::<f64>().is_ok()
                    || matches!(
                        self.tokens.get(self.pos + 1).map(|s| &s.token),
                        Some(Token::Equals) | Some(Token::Arrow)
                    );
                if identifier_like && !starts_statement {
                    self.consume_string()
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// If the next token is a declared kind alias with the given base kind,
    /// return its name and definition without consuming anything.
    fn peek_kind_alias(&self, base: &str) -> Option<(String, KindAlias)> {
        if let Some(Token::Word(w)) = self.peek() {
            let w = w.to_lowercase();
            if let Some(alias) = self.kind_aliases.get(&w) {
                if alias.base == base {
                    return Some((w, alias.clone()));
                }
            }
        }
        None
    }

    /// Consume a trailing `?` uncertainty marker if present (spec §4.1).
    /// Returns true when a marker was consumed; callers add the `Uncertain` tag.
    fn consume_uncertainty_marker(&mut self) -> bool {
        if matches!(self.peek(), Some(Token::Word(w)) if w == "?") {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Consume the next token as a bare word or quoted string, but not an OpenBrace/CloseBrace.
    /// Used for autolayout direction and similar unambiguous positional args.
    fn consume_bare_word_or_string(&mut self) -> Option<String> {
        match self.peek() {
            Some(Token::Quoted(_)) | Some(Token::TextBlock(_)) => self.consume_string(),
            Some(Token::Word(_)) => self.consume_string(),
            _ => None,
        }
    }

    // ─── Phase-2a helpers ────────────────────────────────────────────────────────

    /// Parse a `Status` value word from the current position.
    fn parse_status_value(&mut self) -> Result<Status, ParseError> {
        let (line, col) = self.current_pos();
        let word = self.consume_bare_word_or_string().unwrap_or_default();
        match word.to_lowercase().as_str() {
            "idea"        => Ok(Status::Idea),
            "draft"       => Ok(Status::Draft),
            "specified"   => Ok(Status::Specified),
            "implemented" => Ok(Status::Implemented),
            "deprecated"  => Ok(Status::Deprecated),
            other => Err(ParseError::syntax(line, col, format!("unknown status value: {}", other))),
        }
    }

    /// Parse a `PortDirection` value word from the current position.
    fn parse_port_direction_value(&mut self) -> Result<PortDirection, ParseError> {
        let (line, col) = self.current_pos();
        let word = self.consume_bare_word_or_string().unwrap_or_default();
        match word.to_lowercase().as_str() {
            "in"    => Ok(PortDirection::In),
            "out"   => Ok(PortDirection::Out),
            "inout" => Ok(PortDirection::InOut),
            other => Err(ParseError::syntax(line, col, format!("unknown port direction: {}", other))),
        }
    }

    /// Parse a `port <ident> ["Name"] [{ ... }]` declaration. The `port` keyword
    /// has already been consumed. Returns the DSL-local port identifier and the Port.
    fn parse_port(&mut self) -> Result<(String, Port), ParseError> {
        let ident = self.consume_bare_word_or_string().unwrap_or_default();
        let id = self.next_id();
        let name = self.consume_string_if_not_brace().unwrap_or_else(|| ident.clone());
        let mut port = Port { id, name, ..Default::default() };

        if self.peek_open_brace() {
            self.advance();
            while !self.peek_close_brace() && self.peek().is_some() {
                match self.peek() {
                    Some(Token::Word(w)) => {
                        let w = w.to_lowercase();
                        match w.as_str() {
                            "protocol" => {
                                self.advance();
                                port.protocol = self.consume_string();
                            }
                            "direction" => {
                                self.advance();
                                port.direction = Some(self.parse_port_direction_value()?);
                            }
                            "description" => {
                                self.advance();
                                port.description = self.consume_string();
                            }
                            "url" => {
                                self.advance();
                                port.url = self.consume_string();
                            }
                            "tags" => {
                                self.advance();
                                let mut new_tags: Vec<String> = Vec::new();
                                while matches!(self.peek(), Some(Token::Quoted(_)) | Some(Token::TextBlock(_))) {
                                    if let Some(t) = self.consume_string() {
                                        for part in t.split(',') {
                                            let p = part.trim().to_string();
                                            if !p.is_empty() { new_tags.push(p); }
                                        }
                                    }
                                }
                                if !new_tags.is_empty() {
                                    let extra = new_tags.join(",");
                                    port.tags = Some(match port.tags.take() {
                                        Some(base) => format!("{},{}", base, extra),
                                        None => extra,
                                    });
                                }
                            }
                            "properties" => {
                                self.advance();
                                let props = self.parse_properties_block_body()?;
                                port.properties.get_or_insert_with(HashMap::new).extend(props);
                            }
                            "perspective" => {
                                self.advance();
                                let p = self.parse_one_perspective();
                                port.perspectives.get_or_insert_with(Vec::new).push(p);
                            }
                            _ => {
                                self.advance();
                                self.skip_optional_block_or_value();
                            }
                        }
                    }
                    Some(Token::Directive(_)) => {
                        self.advance();
                        self.skip_directive_args();
                    }
                    _ => {
                        self.advance();
                    }
                }
            }
            self.expect_close_brace()?;
        }

        Ok((ident, port))
    }

    /// Parse a `RelationshipKind` value word from the current position.
    fn parse_relationship_kind_value(&mut self) -> Result<RelationshipKind, ParseError> {
        let (line, col) = self.current_pos();
        let word = self.consume_bare_word_or_string().unwrap_or_default();
        match word.to_lowercase().as_str() {
            "sync"        => Ok(RelationshipKind::Sync),
            "async"       => Ok(RelationshipKind::Async),
            "publish"     => Ok(RelationshipKind::Publish),
            "subscribe"   => Ok(RelationshipKind::Subscribe),
            "dataflow"    => Ok(RelationshipKind::Dataflow),
            "dependency"  => Ok(RelationshipKind::Dependency),
            "deploy"      => Ok(RelationshipKind::Deploy),
            other => Err(ParseError::syntax(line, col, format!("unknown relationship kind: {}", other))),
        }
    }

    /// Return the source line of the last consumed token.
    fn last_consumed_line(&self) -> usize {
        self.tokens
            .get(self.pos.saturating_sub(1))
            .map(|s| s.pos.line)
            .unwrap_or(0)
    }

    /// Consume the next quoted/textblock string token only if it lies on the same
    /// source line as the previously consumed token.  This lets `parse_one_perspective`
    /// distinguish between `"name" "description"` (same line = description) and a
    /// `"name"` that starts the *next* perspective entry (different line = stop).
    fn consume_string_if_same_line(&mut self) -> Option<String> {
        let last_line = self.last_consumed_line();
        let next_line = self.tokens.get(self.pos).map(|s| s.pos.line).unwrap_or(usize::MAX);
        if next_line != last_line {
            return None;
        }
        match self.peek() {
            Some(Token::Quoted(_)) | Some(Token::TextBlock(_)) => self.consume_string(),
            _ => None,
        }
    }

    /// Parse a single perspective entry: `name ["description" ["value"]]`.
    /// Name can be a bare word or quoted string.
    /// Description and value are only consumed when they appear on the same source
    /// line as the name — this prevents consuming the next entry's name as the
    /// current entry's description when names are quoted strings inside a block.
    fn parse_one_perspective(&mut self) -> Perspective {
        let name        = self.consume_bare_word_or_string().unwrap_or_default();
        let description = self.consume_string_if_same_line();
        let value       = self.consume_string_if_same_line();
        Perspective { name, description, value }
    }

    /// Parse a `properties { key value ... }` block and return the map.
    fn parse_properties_block_body(&mut self) -> Result<HashMap<String, String>, ParseError> {
        let mut props = HashMap::new();
        self.expect_open_brace()?;
        while !self.peek_close_brace() && self.peek().is_some() {
            let key   = self.consume_bare_word_or_string().unwrap_or_default();
            let value = self.consume_string().unwrap_or_default();
            if !key.is_empty() {
                props.insert(key, value);
            }
        }
        self.expect_close_brace()?;
        Ok(props)
    }

    /// Parse the body of a relationship block `{ ... }` and fill `rel`.
    fn parse_relationship_body(&mut self, rel: &mut Relationship) -> Result<(), ParseError> {
        self.expect_open_brace()?;
        while !self.peek_close_brace() && self.peek().is_some() {
            match self.peek() {
                Some(Token::Word(w)) => {
                    let w = w.to_lowercase();
                    match w.as_str() {
                        "kind" => {
                            self.advance();
                            rel.kind = Some(self.parse_relationship_kind_value()?);
                        }
                        "status" => {
                            self.advance();
                            rel.status = Some(self.parse_status_value()?);
                        }
                        "introduced" => {
                            self.advance();
                            rel.introduced = self.consume_bare_word_or_string();
                        }
                        "retired" => {
                            self.advance();
                            rel.retired = self.consume_bare_word_or_string();
                        }
                        "perspective" => {
                            self.advance();
                            let p = self.parse_one_perspective();
                            rel.perspectives.get_or_insert_with(Vec::new).push(p);
                        }
                        "description" => {
                            self.advance();
                            rel.description = self.consume_string();
                        }
                        "technology" => {
                            self.advance();
                            rel.technology = self.consume_string();
                        }
                        "url" => {
                            self.advance();
                            rel.url = self.consume_string();
                        }
                        "tags" => {
                            self.advance();
                            let mut new_tags: Vec<String> = Vec::new();
                            // Accept multiple quoted/textblock strings, each optionally comma-separated.
                            while matches!(self.peek(), Some(Token::Quoted(_)) | Some(Token::TextBlock(_))) {
                                if let Some(t) = self.consume_string() {
                                    for part in t.split(',') {
                                        let p = part.trim().to_string();
                                        if !p.is_empty() { new_tags.push(p); }
                                    }
                                }
                            }
                            if !new_tags.is_empty() {
                                let extra = new_tags.join(",");
                                let base  = rel.tags.as_deref().unwrap_or("Relationship");
                                rel.tags  = Some(format!("{},{}", base, extra));
                            }
                        }
                        "properties" => {
                            self.advance();
                            let props = self.parse_properties_block_body()?;
                            let existing = rel.properties.get_or_insert_with(HashMap::new);
                            existing.extend(props);
                        }
                        _ => {
                            // lenient: skip unknown keyword + optional value/block
                            self.advance();
                            self.skip_optional_block_or_value();
                        }
                    }
                }
                Some(Token::Directive(_)) => {
                    self.advance();
                    self.skip_directive_args();
                }
                _ => {
                    self.advance();
                }
            }
        }
        self.expect_close_brace()?;
        Ok(())
    }

    /// Try to consume a common element-level attribute keyword into `extras`.
    /// Returns `true` if a keyword was consumed, `false` if the current token
    /// did not match (caller should fall through to its existing logic).
    ///
    /// `element_id` and `ident_paths` identify the element whose body is being
    /// parsed; they are used to register ports as soon as they are declared so
    /// that later relationships can reference `<element>.<port>`.
    fn try_parse_common_element_keyword(
        &mut self,
        extras: &mut ElementExtras,
        element_id: &str,
        ident_paths: &[String],
    ) -> Result<bool, ParseError> {
        match self.peek() {
            Some(Token::Word(w)) => {
                let w = w.to_lowercase();
                match w.as_str() {
                    "port" => {
                        self.advance();
                        let (port_ident, port) = self.parse_port()?;
                        if !port_ident.is_empty() {
                            for path in ident_paths {
                                if path.is_empty() {
                                    continue;
                                }
                                self.port_register.insert(
                                    format!("{}.{}", path, port_ident).to_lowercase(),
                                    (element_id.to_string(), port.id.clone()),
                                );
                            }
                        }
                        extras.ports.push((port_ident, port));
                        Ok(true)
                    }
                    "status" => {
                        self.advance();
                        extras.status = Some(self.parse_status_value()?);
                        Ok(true)
                    }
                    "description" => {
                        self.advance();
                        extras.description = self.consume_string();
                        Ok(true)
                    }
                    "technology" => {
                        self.advance();
                        extras.technology = self.consume_string();
                        Ok(true)
                    }
                    "url" => {
                        self.advance();
                        extras.url = self.consume_string();
                        Ok(true)
                    }
                    "properties" => {
                        self.advance();
                        let props = self.parse_properties_block_body()?;
                        extras.properties.extend(props);
                        Ok(true)
                    }
                    "tags" | "tag" => {
                        self.advance();
                        while matches!(self.peek(), Some(Token::Quoted(_)) | Some(Token::TextBlock(_))) {
                            if let Some(t) = self.consume_string() {
                                for part in t.split(',') {
                                    let p = part.trim().to_string();
                                    if !p.is_empty() { extras.tags_extra.push(p); }
                                }
                            }
                        }
                        Ok(true)
                    }
                    "group" => {
                        // Leaf form only: `group "Name"` assigns group membership.
                        // (Group blocks with child elements are handled by the
                        // softwareSystem/container body parsers before this.)
                        self.advance();
                        let name = self.consume_string().unwrap_or_default();
                        if self.peek_open_brace() {
                            let (line, col) = self.current_pos();
                            return Err(ParseError::syntax(
                                line,
                                col,
                                "a group block with child elements is not allowed in this element body; use a bare `group \"Name\"` for membership",
                            ));
                        }
                        if !name.is_empty() {
                            extras.group = Some(name);
                        }
                        Ok(true)
                    }
                    "introduced" => {
                        self.advance();
                        extras.introduced = self.consume_bare_word_or_string();
                        Ok(true)
                    }
                    "retired" => {
                        self.advance();
                        extras.retired = self.consume_bare_word_or_string();
                        Ok(true)
                    }
                    "perspective" => {
                        self.advance();
                        let p = self.parse_one_perspective();
                        extras.perspectives.push(p);
                        Ok(true)
                    }
                    "perspectives" => {
                        // Block form: `perspectives { "name" ["desc" ["value"]] ... }`
                        self.advance();
                        self.expect_open_brace()?;
                        while !self.peek_close_brace() && self.peek().is_some() {
                            let p = self.parse_one_perspective();
                            extras.perspectives.push(p);
                        }
                        self.expect_close_brace()?;
                        Ok(true)
                    }
                    _ => Ok(false),
                }
            }
            _ => Ok(false),
        }
    }
}

/// Position of the deepest `{` that is never matched by a `}`, if any.
fn find_unclosed_brace(tokens: &[Spanned]) -> Option<crate::lexer::Pos> {
    let mut stack: Vec<crate::lexer::Pos> = Vec::new();
    for t in tokens {
        match t.token {
            Token::OpenBrace => stack.push(t.pos),
            Token::CloseBrace => {
                stack.pop();
            }
            _ => {}
        }
    }
    stack.last().copied()
}

/// All element ids present in the model, including deployment nodes and instances.
fn collect_element_ids(model: &Model) -> HashSet<String> {
    let mut ids = HashSet::new();
    for p in model.people.iter().flatten() {
        ids.insert(p.id.clone());
    }
    for s in model.software_systems.iter().flatten() {
        ids.insert(s.id.clone());
        for c in s.containers.iter().flatten() {
            ids.insert(c.id.clone());
            for comp in c.components.iter().flatten() {
                ids.insert(comp.id.clone());
            }
        }
    }
    for ce in model.custom_elements.iter().flatten() {
        ids.insert(ce.id.clone());
    }
    fn walk_node(node: &DeploymentNode, ids: &mut HashSet<String>) {
        ids.insert(node.id.clone());
        for ci in node.container_instances.iter().flatten() {
            ids.insert(ci.id.clone());
        }
        for ssi in node.software_system_instances.iter().flatten() {
            ids.insert(ssi.id.clone());
        }
        for inf in node.infrastructure_nodes.iter().flatten() {
            ids.insert(inf.id.clone());
        }
        for child in node.children.iter().flatten() {
            walk_node(child, ids);
        }
    }
    for node in model.deployment_nodes.iter().flatten() {
        walk_node(node, &mut ids);
    }
    ids
}

/// Apply `f` to every relationship stored anywhere in the model.
fn for_each_rel_mut(model: &mut Model, f: &mut impl FnMut(&mut Relationship)) {
    for p in model.people.iter_mut().flatten() {
        p.relationships.iter_mut().flatten().for_each(&mut *f);
    }
    for s in model.software_systems.iter_mut().flatten() {
        s.relationships.iter_mut().flatten().for_each(&mut *f);
        for c in s.containers.iter_mut().flatten() {
            c.relationships.iter_mut().flatten().for_each(&mut *f);
            for comp in c.components.iter_mut().flatten() {
                comp.relationships.iter_mut().flatten().for_each(&mut *f);
            }
        }
    }
    for ce in model.custom_elements.iter_mut().flatten() {
        ce.relationships.iter_mut().flatten().for_each(&mut *f);
    }
    fn walk_node(node: &mut DeploymentNode, f: &mut impl FnMut(&mut Relationship)) {
        node.relationships.iter_mut().flatten().for_each(&mut *f);
        for ci in node.container_instances.iter_mut().flatten() {
            ci.relationships.iter_mut().flatten().for_each(&mut *f);
        }
        for child in node.children.iter_mut().flatten() {
            walk_node(child, f);
        }
    }
    for node in model.deployment_nodes.iter_mut().flatten() {
        walk_node(node, f);
    }
}

/// Rewrite one endpoint of the relationship with id `rel_id`, wherever it lives
/// (attached in the model or still in the deferred list).
fn rewrite_rel_endpoint(
    model: &mut Model,
    deferred: &mut [Relationship],
    rel_id: &str,
    source_side: bool,
    new_id: &str,
    new_port: Option<String>,
) {
    let mut apply = |rel: &mut Relationship| {
        if rel.id != rel_id {
            return;
        }
        if source_side {
            rel.source_id = new_id.to_string();
            if new_port.is_some() {
                rel.source_port_id = new_port.clone();
            }
        } else {
            rel.destination_id = new_id.to_string();
            if new_port.is_some() {
                rel.destination_port_id = new_port.clone();
            }
        }
    };
    for_each_rel_mut(model, &mut apply);
    deferred.iter_mut().for_each(&mut apply);
}

/// Merge a kind alias's tags into `tags` and record the alias name in the
/// element's `kind` property.
fn apply_alias_to_tags_props(
    alias_name: &str,
    alias: &KindAlias,
    tags: &mut Option<String>,
    properties: &mut Option<HashMap<String, String>>,
) {
    if let Some(alias_tags) = &alias.tags {
        *tags = Some(match tags.take() {
            Some(t) => format!("{},{}", t, alias_tags),
            None => alias_tags.clone(),
        });
    }
    properties
        .get_or_insert_with(HashMap::new)
        .insert("kind".to_string(), alias_name.to_string());
}

/// Words that can start a statement inside a model or element body, and must
/// therefore never be consumed as a bare positional value (tags etc.).
fn is_reserved_body_word(lower: &str) -> bool {
    matches!(
        lower,
        "person" | "softwaresystem" | "container" | "component" | "group" | "enterprise"
            | "deploymentenvironment" | "deploymentnode" | "containerinstance"
            | "softwaresysteminstance" | "infrastructurenode" | "deploymentgroup" | "element"
            | "description" | "technology" | "url" | "tags" | "tag" | "properties"
            | "perspective" | "perspectives" | "port" | "status" | "introduced" | "retired"
            | "instances" | "instanceof" | "this" | "model" | "views" | "workspace"
            | "configuration" | "milestones" | "specification" | "auto" | "?"
    )
}

#[allow(dead_code)]
fn is_top_level_keyword(w: &str) -> bool {
    matches!(
        w.to_lowercase().as_str(),
        "workspace" | "model" | "views" | "configuration" | "documentation" | "docs"
    )
}

/// Returns true if `w` is a keyword that can appear at the top-level of a
/// view body block (i.e. it is NOT a valid element-identifier argument to `include`).
fn is_view_block_keyword(w: &str) -> bool {
    matches!(
        w.to_lowercase().as_str(),
        "autolayout"
            | "exclude"
            | "animation"
            | "title"
            | "description"
            | "properties"
            | "include"
    )
}

/// Returns true if `w` is a valid `autoLayout` direction argument.
fn is_autolayout_direction(w: &str) -> bool {
    matches!(
        w.to_lowercase().as_str(),
        "topbottom" | "bottomtop" | "leftright" | "rightleft"
    )
}

/// Canonicalize a shape name to the PascalCase expected by structurizr-diagram.js.
fn canonicalize_shape(shape: &str) -> String {
    match shape.to_lowercase().as_str() {
        "box"                   => "Box",
        "bucket"                => "Bucket",
        "circle"                => "Circle",
        "component"             => "Component",
        "cylinder"              => "Cylinder",
        "diamond"               => "Diamond",
        "ellipse"               => "Ellipse",
        "folder"                => "Folder",
        "hexagon"               => "Hexagon",
        "mobiledevicelandscape" => "MobileDeviceLandscape",
        "mobiledeviceportrait"  => "MobileDevicePortrait",
        "person"                => "Person",
        "pipe"                  => "Pipe",
        "robot"                 => "Robot",
        "roundedbox"            => "RoundedBox",
        "shell"                 => "Shell",
        "terminal"              => "Terminal",
        "webbrowser"            => "WebBrowser",
        "window"                => "Window",
        _                       => return shape.to_string(),
    }
    .to_string()
}

/// Merge base tags with optional extra tags from the DSL.
/// `base` is always included (e.g. "Element,Person").
/// If `extra` is Some("Customer"), result is "Element,Person,Customer".
fn merge_tags(base: &str, extra: Option<String>) -> Option<String> {
    match extra {
        None => Some(base.to_string()),
        Some(e) if e.is_empty() => Some(base.to_string()),
        Some(e) => Some(format!("{},{}", base, e)),
    }
}

#[allow(dead_code)]
fn is_element_keyword(w: &str) -> bool {
    matches!(
        w.to_lowercase().as_str(),
        "person"
            | "softwaresystem"
            | "container"
            | "component"
            | "deploymentenvironment"
            | "deploymentnode"
            | "containerinstance"
            | "softwaresysteminstance"
            | "infrastructurenode"
            | "customelement"
            | "enterprise"
            | "group"
    )
}

#[allow(dead_code)]
fn is_view_keyword(w: &str) -> bool {
    matches!(
        w.to_lowercase().as_str(),
        "systemlandscape"
            | "systemcontext"
            | "container"
            | "component"
            | "dynamic"
            | "deployment"
            | "filtered"
            | "styles"
            | "theme"
            | "branding"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_workspace() {
        let dsl = r#"
workspace "Test" "A test workspace" {
    model {
        user = person "User"
        system = softwareSystem "System"
        user -> system "Uses"
    }
    views {
        systemContext system {
            include *
            autolayout
        }
        theme default
    }
}
"#;
        let ws = parse_str(dsl).expect("should parse");
        assert_eq!(ws.name, "Test");
        let people = ws.model.people.as_ref().expect("should have people");
        assert_eq!(people[0].name, "User");
        let systems = ws.model.software_systems.as_ref().expect("should have systems");
        assert_eq!(systems[0].name, "System");
    }

    #[test]
    fn include_star_populates_system_context_and_container_views() {
        let dsl = r#"
workspace {
    !identifiers hierarchical

    model {
        u = person "User"
        ss = softwareSystem "Software System" {
            wa = container "Web Application"
            db = container "Database"
        }

        u -> ss "Uses"
        u -> ss.wa "Uses"
        ss.wa -> ss.db "Reads from and writes to"
    }

    views {
        systemContext ss "Diagram1" {
            include *
        }

        container ss "Diagram2" {
            include *
        }
    }
}
"#;

        let ws = parse_str(dsl).expect("should parse");

        let context_view = ws
            .views
            .system_context_views
            .as_ref()
            .and_then(|v| v.first())
            .expect("should have system context view");
        let context_elements = context_view
            .element_views
            .as_ref()
            .expect("context view should contain elements");
        assert!(!context_elements.is_empty(), "system context view should have elements from include *");

        let container_view = ws
            .views
            .container_views
            .as_ref()
            .and_then(|v| v.first())
            .expect("should have container view");
        let container_elements = container_view
            .element_views
            .as_ref()
            .expect("container view should contain elements");
        assert!(
            container_elements.len() >= 3,
            "container view should include person and containers (not the boundary software system itself)"
        );

        let container_relationships = container_view
            .relationship_views
            .as_ref()
            .expect("container view should contain relationships");
        assert!(
            container_relationships.len() >= 2,
            "container view should include relationships from include *"
        );
    }

    #[test]
    fn hierarchical_identifiers_resolve_for_container_relationships() {
        let dsl = r#"
workspace {
    !identifiers hierarchical

    model {
        ss = softwareSystem "Software System" {
            a = container "A"
            b = container "B"
        }

        ss.a -> ss.b "Calls"
    }
}
"#;

        let ws = parse_str(dsl).expect("should parse");
        let systems = ws.model.software_systems.expect("should have software systems");
        let ss = systems.first().expect("should have first software system");
        let containers = ss.containers.as_ref().expect("should have containers");
        let a = containers.iter().find(|c| c.name == "A").expect("should have A");
        let b = containers.iter().find(|c| c.name == "B").expect("should have B");

        let rels = a.relationships.as_ref().expect("A should have outgoing relationship");
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].source_id, a.id);
        assert_eq!(rels[0].destination_id, b.id);
    }
}
