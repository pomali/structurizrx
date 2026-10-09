//! `structurizrx add | remove | rename`: surgical edits to DSL source.
//!
//! Agents (and people) edit the DSL as text; these commands do the three
//! edits that are easy to get wrong by hand — putting a statement inside the
//! right block, deleting a declaration together with what refers to it, and
//! renaming an identifier everywhere including `!include`d files — while
//! keeping comments and formatting. Targets are addressed by DSL identifier
//! or by name path (`Shop/API`, `Shop/API->Shop/DB`), resolved through the
//! parser's [`SourceLocations`]. Every edit is written and re-parsed; if the
//! result does not parse, the original files are restored and the parse
//! errors are reported instead, so a workspace never goes from valid to
//! broken.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use structurizr_dsl::lexer::{tokenize, Spanned, Token};
use structurizr_dsl::{ElementType, ParseError, Parsed, SourceLocation};
use structurizr_query::reference::{parse_reference, Catalog, Target};

const INDENT: &str = "    ";

/// What an edit did, as JSON (for `--json` and the MCP tools) and as text.
pub struct Outcome {
    pub json: serde_json::Value,
    pub text: String,
}

impl Outcome {
    pub fn print(&self, json: bool) {
        if json {
            println!("{}", self.json);
        } else {
            print!("{}", self.text);
        }
    }
}

/// Parse the workspace with locations, or explain why it does not parse.
fn parse(file: &Path) -> Result<Parsed> {
    if file
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("json"))
    {
        bail!(
            "{} is a JSON workspace; edits apply to DSL source only",
            file.display()
        );
    }
    structurizr_dsl::parse_file_detailed(file).map_err(|e| {
        anyhow!(
            "{} does not parse:\n{}",
            file.display(),
            render_errors(&e, file)
        )
    })
}

fn render_errors(e: &ParseError, entry: &Path) -> String {
    e.diagnostics()
        .iter()
        .map(|d| {
            let path = ParseError::resolve_file(d, entry);
            if d.line > 0 {
                format!(
                    "  {}:{}:{}: {}",
                    path.display(),
                    d.line,
                    d.column,
                    d.message
                )
            } else {
                format!("  {}: {}", path.display(), d.message)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// What a reference names: the model id and, for elements, its kind.
struct Resolved {
    id: String,
    label: String,
}

/// Resolve `reference` as a DSL identifier first, then as a name path.
fn resolve(parsed: &Parsed, reference: &str) -> Result<Resolved> {
    if let Some((id, kind)) = parsed.identifiers.resolve(reference) {
        let label = match kind {
            ElementType::Relationship => format!("relationship {}", reference),
            other => format!("{:?} {}", other, reference).to_lowercase(),
        };
        return Ok(Resolved {
            id: id.clone(),
            label,
        });
    }
    let catalog = Catalog::new(&parsed.workspace);
    let targets = catalog
        .resolve(&parse_reference(reference))
        .map_err(|miss| anyhow!("{}", structurizr_web::locate::miss_message(&miss)))?;
    let mut ids: Vec<Resolved> = targets
        .into_iter()
        .filter_map(|t| match t {
            Target::Element { id, path, kind } => Some(Resolved {
                id,
                label: format!("{} {}", kind, path),
            }),
            Target::Relationship(m) => Some(Resolved {
                id: m.id.clone(),
                label: structurizr_web::locate::describe(&m),
            }),
            Target::Port {
                path,
                element_id,
                port_id,
            } => {
                // Ports are declared inside their element; the parser locates
                // them by port id.
                let _ = element_id;
                Some(Resolved {
                    id: port_id,
                    label: format!("port {}", path),
                })
            }
            _ => None,
        })
        .collect();
    match ids.len() {
        0 => bail!(
            "'{}' names nothing that can be edited (use an element, relationship or port)",
            reference
        ),
        1 => Ok(ids.remove(0)),
        n => bail!(
            "'{}' is ambiguous ({} matches): {}",
            reference,
            n,
            ids.iter()
                .map(|r| r.label.clone())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn location_of<'a>(parsed: &'a Parsed, id: &str, label: &str) -> Result<&'a SourceLocation> {
    parsed.locations.get(id).ok_or_else(|| {
        anyhow!(
            "{} has no statement of its own in the source (it is derived), so it cannot be edited",
            label
        )
    })
}

/// Text of every file this edit touches, keyed by path, with a snapshot of
/// the originals so a failed edit can be rolled back.
struct Files {
    text: BTreeMap<PathBuf, String>,
    original: BTreeMap<PathBuf, String>,
}

impl Files {
    fn new() -> Self {
        Files {
            text: BTreeMap::new(),
            original: BTreeMap::new(),
        }
    }

    fn load(&mut self, path: &Path) -> Result<&mut String> {
        let key = path.to_path_buf();
        if !self.text.contains_key(&key) {
            let content = std::fs::read_to_string(path)
                .with_context(|| format!("cannot read {}", path.display()))?;
            self.original.insert(key.clone(), content.clone());
            self.text.insert(key.clone(), content);
        }
        Ok(self.text.get_mut(&key).unwrap())
    }

    /// Write every changed file, re-parse the workspace, and roll everything
    /// back if it no longer parses.
    fn commit(&self, entry: &Path) -> Result<()> {
        for (path, text) in &self.text {
            if self.original.get(path) != Some(text) {
                std::fs::write(path, text)
                    .with_context(|| format!("cannot write {}", path.display()))?;
            }
        }
        if let Err(e) = structurizr_dsl::parse_file(entry) {
            for (path, text) in &self.original {
                if self.text.get(path) != Some(text) {
                    let _ = std::fs::write(path, text);
                }
            }
            bail!(
                "the edit would leave the workspace unparseable, so nothing was changed:\n{}",
                render_errors(&e, entry)
            );
        }
        Ok(())
    }
}

fn leading_ws(line: &str) -> &str {
    &line[..line.len() - line.trim_start().len()]
}

/// Lines of `text` as an editable vector (without terminators) plus whether
/// the text ended with a newline.
fn split_lines(text: &str) -> (Vec<String>, bool) {
    let ends_nl = text.ends_with('\n');
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    if lines.is_empty() {
        lines.push(String::new());
    }
    (lines, ends_nl)
}

fn join_lines(lines: &[String], ends_nl: bool) -> String {
    let mut out = lines.join("\n");
    if ends_nl {
        out.push('\n');
    }
    out
}

/// Re-indent a (possibly multi-line) statement so its first line sits at
/// `indent` and nested lines keep their relative indentation.
fn indent_statement(statement: &str, indent: &str) -> Vec<String> {
    let raw: Vec<&str> = statement.trim_matches('\n').lines().collect();
    let common = raw
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| leading_ws(l).len())
        .min()
        .unwrap_or(0);
    raw.iter()
        .map(|l| {
            if l.trim().is_empty() {
                String::new()
            } else {
                format!("{}{}", indent, &l[common.min(leading_ws(l).len())..])
            }
        })
        .collect()
}

/// Where a statement was inserted.
pub struct Inserted {
    pub file: PathBuf,
    pub line: usize,
}

/// Insert `statement` at the end of the block that starts on `start_line`
/// and closes on `end_line` (1-based, `end_line == start_line` for a
/// statement without a block, which then gets one).
fn insert_in_block(
    text: &mut String,
    start_line: usize,
    end_line: usize,
    statement: &str,
) -> usize {
    let (mut lines, ends_nl) = split_lines(text);
    let start_idx = start_line.saturating_sub(1).min(lines.len() - 1);
    let outer = leading_ws(&lines[start_idx]).to_string();
    let inserted_at;
    if end_line > start_line {
        let end_idx = end_line.saturating_sub(1).min(lines.len() - 1);
        // Indent like the block's existing content, else one level in.
        let inner = lines[start_idx + 1..end_idx]
            .iter()
            .find(|l| !l.trim().is_empty())
            .map(|l| leading_ws(l).to_string())
            .unwrap_or_else(|| format!("{}{}", outer, INDENT));
        let new_lines = indent_statement(statement, &inner);
        inserted_at = end_idx + 1;
        let closing = &lines[end_idx];
        if closing.trim() == "}" {
            for (k, l) in new_lines.into_iter().enumerate() {
                lines.insert(end_idx + k, l);
            }
        } else {
            // Closing brace shares its line with content: split it off.
            let brace = closing.rfind('}').unwrap_or(closing.len());
            let head = closing[..brace].trim_end().to_string();
            let tail = format!("{}}}", outer);
            lines[end_idx] = head;
            let mut k = end_idx + 1;
            for l in new_lines {
                lines.insert(k, l);
                k += 1;
            }
            lines.insert(k, tail);
        }
    } else {
        let inner = format!("{}{}", outer, INDENT);
        let new_lines = indent_statement(statement, &inner);
        let line = lines[start_idx].clone();
        let trimmed = line.trim_end();
        let (head, had_block) = match trimmed.strip_suffix('}') {
            Some(h) => (h.trim_end().to_string(), true),
            None => (format!("{} {{", trimmed), false),
        };
        let _ = had_block;
        lines[start_idx] = head;
        inserted_at = start_idx + 2;
        let mut k = start_idx + 1;
        for l in new_lines {
            lines.insert(k, l);
            k += 1;
        }
        lines.insert(k, format!("{}}}", outer));
    }
    *text = join_lines(&lines, ends_nl);
    inserted_at
}

/// Find the `model { … }` or `views { … }` block of the entry file: the line
/// its keyword is on and the line of its closing brace.
fn top_block(text: &str, keyword: &str) -> Option<(usize, usize)> {
    let tokens = tokenize(text);
    let mut depth = 0usize;
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i].token {
            Token::OpenBrace => depth += 1,
            Token::CloseBrace => depth = depth.saturating_sub(1),
            Token::Word(w) if depth == 1 && w.eq_ignore_ascii_case(keyword) => {
                if matches!(tokens.get(i + 1).map(|t| &t.token), Some(Token::OpenBrace)) {
                    let start = tokens[i].pos.line;
                    let mut d = 0usize;
                    for t in &tokens[i + 1..] {
                        match t.token {
                            Token::OpenBrace => d += 1,
                            Token::CloseBrace => {
                                d -= 1;
                                if d == 0 {
                                    return Some((start, t.pos.line));
                                }
                            }
                            _ => {}
                        }
                    }
                    return None;
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// `structurizrx add`: put `statement` inside the block of `parent`
/// (`model`, `views`, or an element/relationship reference).
pub fn add(file: &Path, statement: &str, parent: &str) -> Result<Outcome> {
    let parsed = parse(file)?;
    let statement = statement.trim();
    if statement.is_empty() {
        bail!("nothing to add: the statement is empty");
    }
    let mut files = Files::new();
    let inserted = match parent.to_ascii_lowercase().as_str() {
        block @ ("model" | "views") => {
            let text = files.load(file)?;
            match top_block(text, block) {
                Some((start, end)) => {
                    let line = insert_in_block(text, start, end, statement);
                    Inserted {
                        file: file.to_path_buf(),
                        line,
                    }
                }
                None if block == "model"
                    && !text.trim_start().starts_with("workspace")
                    && !text.contains("workspace") =>
                {
                    // A sketch: statements live at the top level.
                    let (mut lines, _) = split_lines(text);
                    while lines.last().is_some_and(|l| l.trim().is_empty()) {
                        lines.pop();
                    }
                    lines.extend(indent_statement(statement, ""));
                    let line = lines.len();
                    *text = join_lines(&lines, true);
                    Inserted {
                        file: file.to_path_buf(),
                        line,
                    }
                }
                None => bail!(
                    "{} has no `{} {{ … }}` block to add to",
                    file.display(),
                    block
                ),
            }
        }
        _ => {
            let target = resolve(&parsed, parent)?;
            let loc = location_of(&parsed, &target.id, &target.label)?.clone();
            let path = loc.file.clone().unwrap_or_else(|| file.to_path_buf());
            let text = files.load(&path)?;
            let line = insert_in_block(text, loc.line, loc.end_line, statement);
            Inserted { file: path, line }
        }
    };
    files.commit(file)?;
    Ok(Outcome {
        json: serde_json::json!({ "added": { "file": inserted.file.display().to_string(), "line": inserted.line } }),
        text: format!("added at {}:{}\n", inserted.file.display(), inserted.line),
    })
}

/// Delete lines `start..=end` (1-based) from `text`.
fn delete_lines(text: &mut String, start: usize, end: usize) {
    let (mut lines, ends_nl) = split_lines(text);
    let s = start.saturating_sub(1).min(lines.len());
    let e = end.min(lines.len());
    if s < e {
        lines.drain(s..e);
    }
    *text = join_lines(&lines, ends_nl);
}

/// `structurizrx remove`: delete the statement declaring `reference`; with
/// `cascade`, also every relationship statement touching it or anything
/// inside it.
pub fn remove(file: &Path, reference: &str, cascade: bool) -> Result<Outcome> {
    let parsed = parse(file)?;
    let target = resolve(&parsed, reference)?;
    let mut ranges: Vec<(PathBuf, usize, usize, String)> = Vec::new();
    let loc = location_of(&parsed, &target.id, &target.label)?;
    let path_of = |loc: &SourceLocation| loc.file.clone().unwrap_or_else(|| file.to_path_buf());
    ranges.push((path_of(loc), loc.line, loc.end_line, target.label.clone()));

    if cascade {
        let idx = structurizr_query::build_index(&parsed.workspace);
        let mut inside: HashSet<String> = HashSet::new();
        inside.insert(target.id.clone());
        for e in &idx.elements {
            if e.ancestors.contains(&target.id) {
                inside.insert(e.id.clone());
            }
        }
        let summaries = structurizr_query::relationship_summaries(&parsed.workspace);
        for r in structurizr_query::all_relationships(&parsed.workspace) {
            if r.id == target.id {
                continue;
            }
            if inside.contains(&r.source_id) || inside.contains(&r.destination_id) {
                if let Some(rl) = parsed.locations.get(&r.id) {
                    let label = summaries
                        .get(&r.id)
                        .map(|s| s.line())
                        .unwrap_or_else(|| r.id.clone());
                    ranges.push((
                        path_of(rl),
                        rl.line,
                        rl.end_line,
                        format!("relationship {}", label),
                    ));
                }
            }
        }
    }

    // Drop ranges nested inside another range (a relationship declared in
    // the removed element's own block), then delete bottom-up per file.
    let keep: Vec<(PathBuf, usize, usize, String)> = ranges
        .iter()
        .filter(|(p, s, e, _)| {
            !ranges
                .iter()
                .any(|(p2, s2, e2, _)| p2 == p && (s2, e2) != (s, e) && *s2 <= *s && *e <= *e2)
        })
        .cloned()
        .collect();
    let mut by_file: HashMap<PathBuf, Vec<(usize, usize)>> = HashMap::new();
    for (p, s, e, _) in &keep {
        by_file.entry(p.clone()).or_default().push((*s, *e));
    }
    let mut files = Files::new();
    for (path, mut spans) in by_file {
        spans.sort();
        spans.dedup();
        let text = files.load(&path)?;
        for (s, e) in spans.into_iter().rev() {
            delete_lines(text, s, e);
        }
    }
    files.commit(file).map_err(|e| {
        if cascade {
            e
        } else {
            anyhow!(
                "{}\n(use --cascade to also remove the relationships that refer to it)",
                e
            )
        }
    })?;
    let mut text = String::new();
    for (p, s, e, label) in &keep {
        if e > s {
            text.push_str(&format!(
                "removed {} ({}:{}-{})\n",
                label,
                p.display(),
                s,
                e
            ));
        } else {
            text.push_str(&format!("removed {} ({}:{})\n", label, p.display(), s));
        }
    }
    Ok(Outcome {
        json: serde_json::json!({ "removed": keep.iter().map(|(p, s, e, label)| serde_json::json!({
            "what": label, "file": p.display().to_string(), "line": s, "endLine": e
        })).collect::<Vec<_>>() }),
        text,
    })
}

/// `structurizrx fmt`: the workspace re-emitted as canonical DSL (a `.json`
/// workspace becomes DSL; a sketch becomes a full `workspace { … }`).
/// Returns the text and why writing it over the source would lose
/// something, if it would.
pub fn format(file: &Path) -> Result<(String, Vec<String>)> {
    let is_json = file
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("json"));
    if is_json {
        let ws = crate::load_workspace(&file.to_path_buf())?;
        return Ok((structurizr_dsl::emit(&ws), Vec::new()));
    }
    let parsed = structurizr_dsl::parse_file_detailed(file).map_err(|e| {
        anyhow!(
            "{} does not parse:\n{}",
            file.display(),
            render_errors(&e, file)
        )
    })?;
    let text = structurizr_dsl::emit_with_identifiers(&parsed.workspace, &parsed.identifiers);
    let source =
        std::fs::read_to_string(file).with_context(|| format!("cannot read {}", file.display()))?;
    let mut losses = Vec::new();
    let mut comments = false;
    let mut directives: Vec<&str> = Vec::new();
    for line in source.lines() {
        let t = line.trim_start();
        if t.starts_with("//") || t.starts_with('#') || t.starts_with("/*") || line.contains(" // ")
        {
            comments = true;
        }
        for d in [
            "!include",
            "!docs",
            "!adrs",
            "!decisions",
            "!const",
            "!constant",
            "!var",
        ] {
            if t.len() >= d.len()
                && t[..d.len()].eq_ignore_ascii_case(d)
                && !directives.contains(&d)
            {
                directives.push(d);
            }
        }
    }
    if comments {
        losses.push("comments are not preserved".to_string());
    }
    for d in directives {
        losses.push(match d {
            "!include" => "`!include`d files would be inlined into one file".to_string(),
            "!docs" | "!adrs" | "!decisions" => {
                format!("`{}` directory imports are not re-emitted", d)
            }
            other => format!("`{}` values are substituted, not kept", other),
        });
    }
    if source.contains("specification") {
        losses.push("`specification` kind aliases are written as their base kinds".to_string());
    }
    Ok((text, losses))
}

/// The entry file and, recursively, every file it `!include`s.
fn include_closure(entry: &Path) -> Result<Vec<PathBuf>> {
    fn walk(path: &Path, out: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>) -> Result<()> {
        if !seen.insert(path.to_path_buf()) {
            return Ok(());
        }
        out.push(path.to_path_buf());
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        for line in text.lines() {
            let trimmed = line.trim_start();
            let rest = trimmed
                .strip_prefix("!include ")
                .or_else(|| trimmed.strip_prefix("!INCLUDE "));
            if let Some(rest) = rest {
                let rel = rest.trim().trim_matches('"');
                let inc = dir.join(rel);
                if inc.is_file() {
                    walk(&inc, out, seen)?;
                }
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(entry, &mut out, &mut HashSet::new())?;
    Ok(out)
}

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Replace every segment of a dotted word equal to `old` (case-insensitive).
fn rename_word(word: &str, old: &str, new: &str) -> Option<String> {
    let mut changed = false;
    let parts: Vec<String> = word
        .split('.')
        .map(|seg| {
            if seg.eq_ignore_ascii_case(old) {
                changed = true;
                new.to_string()
            } else {
                seg.to_string()
            }
        })
        .collect();
    changed.then(|| parts.join("."))
}

/// Rewrite identifier tokens in `text`; returns the number of replacements.
fn rename_in_text(text: &mut String, old: &str, new: &str) -> usize {
    let tokens: Vec<Spanned> = tokenize(text);
    let mut edits: Vec<(usize, usize, usize, String)> = Vec::new(); // line, col, old char len, new
    for t in &tokens {
        if let Token::Word(w) = &t.token {
            if let Some(replacement) = rename_word(w, old, new) {
                edits.push((t.pos.line, t.pos.col, w.chars().count(), replacement));
            }
        }
    }
    if edits.is_empty() {
        return 0;
    }
    let (mut lines, ends_nl) = split_lines(text);
    // Apply right-to-left within each line so earlier columns stay valid.
    edits.sort_by(|a, b| (b.0, b.1).cmp(&(a.0, a.1)));
    for (line, col, len, replacement) in &edits {
        let Some(l) = lines.get_mut(line - 1) else {
            continue;
        };
        let chars: Vec<char> = l.chars().collect();
        let start = col - 1;
        if start + len > chars.len() {
            continue;
        }
        let mut rebuilt: String = chars[..start].iter().collect();
        rebuilt.push_str(replacement);
        rebuilt.extend(chars[start + len..].iter());
        *l = rebuilt;
    }
    *text = join_lines(&lines, ends_nl);
    edits.len()
}

/// `structurizrx rename`: change a DSL identifier everywhere, across
/// `!include`d files. Element names (quoted strings) are untouched.
pub fn rename(file: &Path, old: &str, new: &str) -> Result<Outcome> {
    let parsed = parse(file)?;
    if parsed.identifiers.resolve(old).is_none() {
        let mut msg = format!("'{}' is not a declared identifier", old);
        let candidates = parsed.identifiers.identifiers.keys().map(String::as_str);
        if let Some(s) = structurizr_dsl::suggest::closest(old, candidates) {
            msg.push_str(&format!(" (did you mean '{}'?)", s));
        }
        bail!("{}", msg);
    }
    if !is_identifier(new) {
        bail!("'{}' is not a valid identifier (letters, digits, '_' and '-', not starting with a digit)", new);
    }
    if parsed.identifiers.resolve(new).is_some() {
        bail!("'{}' is already an identifier in this workspace", new);
    }
    let mut files = Files::new();
    let mut counts: Vec<(PathBuf, usize)> = Vec::new();
    for path in include_closure(file)? {
        let text = files.load(&path)?;
        let n = rename_in_text(text, old, new);
        if n > 0 {
            counts.push((path, n));
        }
    }
    files.commit(file)?;
    let total: usize = counts.iter().map(|(_, n)| n).sum();
    let plural = |n: usize| if n == 1 { "" } else { "s" };
    let mut text = format!(
        "renamed {} -> {} ({} occurrence{} in {} file{})\n",
        old,
        new,
        total,
        plural(total),
        counts.len(),
        plural(counts.len())
    );
    for (p, n) in &counts {
        text.push_str(&format!("  {}: {}\n", p.display(), n));
    }
    Ok(Outcome {
        json: serde_json::json!({ "renamed": { "from": old, "to": new, "files": counts.iter().map(|(p, n)| serde_json::json!({
            "file": p.display().to_string(), "replacements": n
        })).collect::<Vec<_>>() } }),
        text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_into_multiline_block_before_closing_brace() {
        let mut text = "workspace {\n    model {\n        a = person \"A\"\n    }\n}\n".to_string();
        let line = insert_in_block(&mut text, 2, 4, "b = person \"B\"");
        assert_eq!(text, "workspace {\n    model {\n        a = person \"A\"\n        b = person \"B\"\n    }\n}\n");
        assert_eq!(line, 4);
    }

    #[test]
    fn insert_into_single_line_statement_opens_a_block() {
        let mut text = "    api = container \"API\"\n".to_string();
        insert_in_block(&mut text, 1, 1, "technology \"Rust\"");
        assert_eq!(
            text,
            "    api = container \"API\" {\n        technology \"Rust\"\n    }\n"
        );
    }

    #[test]
    fn insert_into_inline_block_splits_it() {
        let mut text = "    api = container \"API\" { tags \"x\" }\n".to_string();
        insert_in_block(&mut text, 1, 1, "technology \"Rust\"");
        assert_eq!(
            text,
            "    api = container \"API\" { tags \"x\"\n        technology \"Rust\"\n    }\n"
        );
    }

    #[test]
    fn top_block_finds_model_and_views() {
        let text = "workspace \"x\" {\n  model {\n    a = person \"A\"\n  }\n  views {\n  }\n}\n";
        assert_eq!(top_block(text, "model"), Some((2, 4)));
        assert_eq!(top_block(text, "views"), Some((5, 6)));
        assert_eq!(top_block(text, "styles"), None);
    }

    #[test]
    fn rename_replaces_words_and_dotted_segments_but_not_strings() {
        let mut text = "api = container \"api\" {\n  port rest \"R\"\n}\nweb -> api.rest \"calls api\"\nAPI -> db\n".to_string();
        let n = rename_in_text(&mut text, "api", "gateway");
        assert_eq!(n, 3);
        assert_eq!(text, "gateway = container \"api\" {\n  port rest \"R\"\n}\nweb -> gateway.rest \"calls api\"\ngateway -> db\n");
    }

    #[test]
    fn delete_lines_removes_inclusive_range() {
        let mut text = "a\nb\nc\nd\n".to_string();
        delete_lines(&mut text, 2, 3);
        assert_eq!(text, "a\nd\n");
    }
}
