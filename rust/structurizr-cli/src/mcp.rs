//! `structurizrx mcp`: a Model Context Protocol server over stdio exposing
//! the CLI's read and check commands as tools, for agent hosts (Claude
//! Desktop, Cursor, Codex, Claude Code) that would otherwise have to shell
//! out to `structurizrx`.
//!
//! Hand-rolled JSON-RPC 2.0 (no MCP SDK crate), newline-delimited JSON on
//! stdin/stdout, mirroring the shape of [`structurizr_lsp::jsonrpc`]: a
//! synchronous dispatcher whose `handle` takes one decoded message and
//! returns the messages to send back, plus a thin stdio loop that owns
//! reading, writing and flushing. Nothing but protocol messages goes to
//! stdout; diagnostics go to stderr.

use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use serde_json::{json, Value};

use structurizr_dsl::{parse_file, parse_str, ParseError};
use structurizr_model::{validation, Workspace};
use structurizr_renderer::{exporter::DiagramExporter, svg::SvgExporter};

const PROTOCOL_VERSION: &str = "2025-06-18";
const PARSE_ERROR: i32 = -32700;
const METHOD_NOT_FOUND: i32 = -32601;
const INVALID_PARAMS: i32 = -32602;

/// Dispatches MCP (JSON-RPC 2.0) messages. Stateless beyond what a single
/// tool call needs — every tool loads its workspace fresh, so there is
/// nothing to carry between calls today, but `handle` takes `&mut self` to
/// match the shape a stateful extension (e.g. tracking `initialize`) would
/// need.
#[derive(Default)]
pub struct Dispatcher;

impl Dispatcher {
    pub fn new() -> Self {
        Self
    }

    /// Handle one incoming JSON-RPC message, returning the messages to send
    /// back. A request yields exactly one response; a notification yields
    /// none.
    pub fn handle(&mut self, message: &str) -> Vec<String> {
        let Ok(value) = serde_json::from_str::<Value>(message) else {
            return vec![error_response(Value::Null, PARSE_ERROR, "invalid JSON-RPC message")];
        };
        let method = value.get("method").and_then(Value::as_str).unwrap_or("");
        let params = value.get("params").cloned().unwrap_or(Value::Null);

        match value.get("id").cloned() {
            Some(id) => vec![self.request(id, method, params)],
            None => {
                self.notification(method, params);
                Vec::new()
            }
        }
    }

    fn request(&mut self, id: Value, method: &str, params: Value) -> String {
        let result: Value = match method {
            "initialize" => json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "structurizrx", "version": env!("CARGO_PKG_VERSION") },
            }),
            "ping" => json!({}),
            "tools/list" => json!({ "tools": tool_definitions() }),
            "tools/call" => return self.tools_call(id, params),
            _ => {
                return error_response(id, METHOD_NOT_FOUND, &format!("unsupported method: {method}"))
            }
        };
        json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
    }

    /// `notifications/initialized`, `notifications/cancelled`, … — all
    /// ignored; there is no session state to react to them with today.
    fn notification(&mut self, _method: &str, _params: Value) {}

    fn tools_call(&mut self, id: Value, params: Value) -> String {
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            return error_response(id, INVALID_PARAMS, "tools/call requires a string `name`");
        };
        let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));

        let result = match name {
            "validate" => call(&args, tools::validate),
            "digest" => call(&args, tools::digest),
            "query" => call(&args, tools::query),
            "locate" => call(&args, tools::locate),
            "lint" => call(&args, tools::lint),
            "diff" => call(&args, tools::diff),
            "render" => call(&args, tools::render),
            "docs" => call(&args, tools::docs),
            "add" => call(&args, tools::add),
            "remove" => call(&args, tools::remove),
            "rename" => call(&args, tools::rename),
            "format" => call(&args, tools::format),
            other => Err(ToolError::Execution(format!("unknown tool: {other}"))),
        };

        match result {
            Ok(value) => json!({ "jsonrpc": "2.0", "id": id, "result": value }).to_string(),
            Err(ToolError::Invalid(message)) => error_response(id, INVALID_PARAMS, &message),
            Err(ToolError::Execution(message)) => {
                json!({ "jsonrpc": "2.0", "id": id, "result": error_result(&message) }).to_string()
            }
        }
    }
}

/// A stdio JSON-RPC loop over [`Dispatcher`]: one JSON object per line in,
/// zero or more JSON objects (one per line) out, flushed after every
/// message.
pub fn run() -> anyhow::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut dispatcher = Dispatcher::new();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        for response in dispatcher.handle(&line) {
            stdout.write_all(response.as_bytes())?;
            stdout.write_all(b"\n")?;
            stdout.flush()?;
        }
    }
    Ok(())
}

fn error_response(id: Value, code: i32, message: &str) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }).to_string()
}

/// A successful `tools/call` result carrying one text block and `isError:
/// true` — how a tool reports that it could not do its job (bad file, git
/// failure, …), as distinct from a malformed request.
fn error_result(message: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": message }], "isError": true })
}

/// A successful `tools/call` result: one text block holding pretty JSON,
/// plus the same value as `structuredContent`.
fn json_result(value: Value) -> Value {
    let text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
    json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": value,
        "isError": false,
    })
}

/// A successful `tools/call` result: a single text block, no structured
/// content.
fn text_result(text: impl Into<String>) -> Value {
    json!({ "content": [{ "type": "text", "text": text.into() }], "isError": false })
}

/// Why a tool implementation could not produce a result: `Invalid` is a
/// malformed request (missing/wrong-typed argument) reported as a JSON-RPC
/// `-32602` error; `Execution` is everything a tool discovers while doing
/// its job (bad workspace, git failure, …), reported as a successful
/// `tools/call` result with `isError: true`.
enum ToolError {
    Invalid(String),
    Execution(String),
}

fn call(args: &Value, f: impl FnOnce(&Value) -> Result<Value, ToolError>) -> Result<Value, ToolError> {
    f(args)
}

fn arg_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ToolError::Invalid(format!("missing or invalid required argument `{key}`")))
}

fn arg_str_opt<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str)
}

fn arg_bool(args: &Value, key: &str, default: bool) -> bool {
    args.get(key).and_then(Value::as_bool).unwrap_or(default)
}

fn arg_str_array(args: &Value, key: &str) -> Result<Vec<String>, ToolError> {
    let Some(array) = args.get(key).and_then(Value::as_array) else {
        return Err(ToolError::Invalid(format!("missing or invalid required argument `{key}`")));
    };
    array
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_string)
                .ok_or_else(|| ToolError::Invalid(format!("`{key}` must be an array of strings")))
        })
        .collect()
}

/// Load a workspace the same way `load_workspace` in `main.rs` does: `.json`
/// via serde, else `structurizr_dsl::parse_file`. Duplicated here rather
/// than shared, per this crate's module boundaries.
fn load_workspace(path: &Path) -> Result<Workspace, ToolError> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext == "json" {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ToolError::Execution(format!("failed to read {}: {e}", path.display())))?;
        serde_json::from_str(&content)
            .map_err(|e| ToolError::Execution(format!("failed to parse JSON from {}: {e}", path.display())))
    } else {
        parse_file(path)
            .map_err(|e| ToolError::Execution(format!("failed to parse DSL from {}: {e}", path.display())))
    }
}

/// Materialize `auto` views, warning to stderr (never stdout — that's the
/// protocol channel) rather than failing the tool call.
fn generate_views(workspace: &mut Workspace) {
    if let Err(e) = structurizr_query::generate_views(workspace) {
        eprintln!("warning: view generation failed: {e}");
    }
}

fn tool_definitions() -> Vec<Value> {
    let file_prop = json!({ "type": "string", "description": "Absolute or cwd-relative path to a .dsl or .json workspace file." });
    vec![
        json!({
            "name": "validate",
            "description": "Parse and validate a Structurizr DSL or JSON workspace file. Use this to check a workspace is well-formed before relying on any other tool's output, or to surface parse/validation/lint problems to fix. Returns parse errors with file/line/column, model validation errors, and lint findings.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": file_prop,
                    "strict": { "type": "boolean", "description": "Also treat lint findings (placeholders, uncertain items, orphans, unbound ports) as failing validity. Default false." },
                },
                "required": ["file"],
            },
        }),
        json!({
            "name": "digest",
            "description": "Produce a compact plain-text summary of a workspace's model (elements, relationships, views), sized to fit in an LLM's context. Use this to get oriented in an unfamiliar workspace before querying or editing it.",
            "inputSchema": {
                "type": "object",
                "properties": { "file": file_prop },
                "required": ["file"],
            },
        }),
        json!({
            "name": "query",
            "description": "Run a selector expression against a workspace and return the matching elements and relationships as structured data. Use this to find specific elements (e.g. by tag, kind, or name pattern) or to explore a neighbourhood, without loading the whole model.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": file_prop,
                    "expression": { "type": "string", "description": "Selector expression, e.g. `element.tag==Database`, `element.status==idea && element.layer==domain`, or `->api->2` for a neighbourhood." },
                },
                "required": ["file", "expression"],
            },
        }),
        json!({
            "name": "locate",
            "description": "Find where elements, ports, relationships, views or decisions are declared in the DSL source, by canonical name path (e.g. `Shop/API`) or by a viewer link carrying a selection. Use this to jump from something you found with `query` or `digest` to the exact source line to read or edit.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": file_prop,
                    "references": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Name paths (`Shop/API`, `Shop/API.http`, `Shop/API->Shop/DB \"reads\"`, `view:<key>`, `decision:<id>`) or viewer links (`http://…#view&sel=…`).",
                    },
                },
                "required": ["file", "references"],
            },
        }),
        json!({
            "name": "lint",
            "description": "Check a workspace's model hygiene: blocking findings (placeholders, uncertain items, orphans, unbound ports — the same gate `validate --strict` fails on) and softer warnings (missing description/technology, elements in no view, duplicate names, undescribed relationships). Use this before treating a workspace as finished.",
            "inputSchema": {
                "type": "object",
                "properties": { "file": file_prop },
                "required": ["file"],
            },
        }),
        json!({
            "name": "diff",
            "description": "Compare two versions of a workspace as models (elements, relationships, views and decisions added, removed or changed), keyed by name rather than id so renames are detected. Compares git revisions of the same file by default, or two separate workspace files when `against` is given. Use this to review what an edit changed, or to summarize a workspace's history.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": file_prop,
                    "from": { "type": "string", "description": "Git revision to compare from. Default: HEAD." },
                    "to": { "type": "string", "description": "Git revision to compare to. Default: the file as it is on disk right now." },
                    "against": { "type": "string", "description": "Path to another workspace file to compare `file` (as it is on disk) against, instead of using git history." },
                },
                "required": ["file"],
            },
        }),
        json!({
            "name": "render",
            "description": "Render a workspace's diagrams to SVG or PNG. Use this to visually inspect a view, e.g. after editing the DSL, or to produce an image for a report. Defaults to rendering every view; pass `view` to render just one.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": file_prop,
                    "view": { "type": "string", "description": "Key of a single view to render. Default: every view in the workspace." },
                    "format": { "type": "string", "enum": ["svg", "png"], "description": "Output format. Default: svg." },
                },
                "required": ["file"],
            },
        }),
        json!({
            "name": "docs",
            "description": "Return the Structurizr DSL extension cheat sheet (the format reference for authoring workspaces): keywords, syntax and the extensions this toolchain adds beyond upstream Structurizr. Use this before writing or editing DSL from scratch, or when unsure of a keyword's syntax.",
            "inputSchema": { "type": "object", "properties": {}, "required": [] },
        }),
        json!({
            "name": "add",
            "description": "Add one DSL statement (an element, relationship, view, or body line such as `technology \"Rust\"`) inside a block of the workspace source, keeping comments and formatting. Prefer this over rewriting the file: it finds the right block, including in `!include`d files, and writes nothing if the result would not parse.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": file_prop,
                    "statement": { "type": "string", "description": "The DSL statement, e.g. `cache = container \"Cache\" \"\" \"Redis\"` or `web -> api \"calls\"`." },
                    "in": { "type": "string", "description": "Block to add into: `model` (default), `views`, or an element's identifier or name path such as `shop` or `Shop/API`." },
                },
                "required": ["file", "statement"],
            },
        }),
        json!({
            "name": "remove",
            "description": "Delete the statement declaring an element, relationship or port from the workspace source. Fails without writing if something still refers to it, unless `cascade` is set, which also removes every relationship touching it or anything inside it.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": file_prop,
                    "reference": { "type": "string", "description": "Identifier or name path: `api`, `Shop/API`, `Shop/Web App->Shop/API`." },
                    "cascade": { "type": "boolean", "description": "Also remove relationships that refer to it. Default false." },
                },
                "required": ["file", "reference"],
            },
        }),
        json!({
            "name": "rename",
            "description": "Rename a DSL identifier everywhere it is used, across `!include`d files, without touching quoted element names. Use this instead of search-and-replace, which also hits descriptions and partial words.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": file_prop,
                    "identifier": { "type": "string", "description": "The current identifier." },
                    "newIdentifier": { "type": "string", "description": "The new identifier." },
                },
                "required": ["file", "identifier", "newIdentifier"],
            },
        }),
        json!({
            "name": "format",
            "description": "Return the workspace as canonical DSL text: converts a `.json` workspace to DSL, and promotes a sketch (bare arrows) to a full `workspace { model { … } views { … } }`. Read-only; comments are not preserved, and `losses` lists anything else the text would drop.",
            "inputSchema": { "type": "object", "properties": { "file": file_prop }, "required": ["file"] },
        }),
    ]
}

mod tools {
    use super::*;

    pub fn validate(args: &Value) -> Result<Value, ToolError> {
        let file = PathBuf::from(arg_str(args, "file")?);
        let strict = arg_bool(args, "strict", false);

        let workspace = match load_workspace_detailed(&file) {
            Ok(ws) => ws,
            Err(LoadError::Parse(e)) => {
                let out = json!({
                    "valid": false,
                    "errors": parse_error_json(&e, &file),
                    "lint": [],
                });
                return Ok(json_result(out));
            }
            Err(LoadError::Other(message)) => {
                let out = json!({
                    "valid": false,
                    "errors": [{ "code": "load", "message": message }],
                    "lint": [],
                });
                return Ok(json_result(out));
            }
        };

        let errors = validation::validate(&workspace);
        let findings = structurizr_query::lint(&workspace);
        let valid = errors.is_empty() && (!strict || findings.is_empty());

        let out = json!({
            "valid": valid,
            "errors": errors.iter().map(|e| json!({
                "code": e.code(),
                "message": e.to_string(),
            })).collect::<Vec<_>>(),
            "lint": findings.iter().map(|f| json!({
                "code": f.code,
                "elementId": f.element_id,
                "name": f.name,
                "message": f.message,
            })).collect::<Vec<_>>(),
        });
        Ok(json_result(out))
    }

    pub fn digest(args: &Value) -> Result<Value, ToolError> {
        let file = PathBuf::from(arg_str(args, "file")?);
        let mut workspace = load_workspace(&file)?;
        generate_views(&mut workspace);
        Ok(text_result(structurizr_query::digest(&workspace)))
    }

    pub fn query(args: &Value) -> Result<Value, ToolError> {
        let file = PathBuf::from(arg_str(args, "file")?);
        let expression = arg_str(args, "expression")?;
        let workspace = load_workspace(&file)?;
        let selection = structurizr_query::query(expression, &workspace)
            .map_err(|e| ToolError::Execution(e.to_string()))?;
        let names = structurizr_query::element_names(&workspace);
        let paths = structurizr_query::element_paths(&workspace);
        let rels = structurizr_query::relationship_summaries(&workspace);

        let out = json!({
            "elements": selection.elements.iter().map(|id| json!({
                "id": id,
                "name": names.get(id),
                "path": paths.get(id),
            })).collect::<Vec<_>>(),
            "relationships": selection.relationships.iter().map(|id| match rels.get(id) {
                Some(r) => serde_json::to_value(r).unwrap_or_default(),
                None => json!({ "id": id }),
            }).collect::<Vec<_>>(),
        });
        Ok(json_result(out))
    }

    pub fn locate(args: &Value) -> Result<Value, ToolError> {
        let file = PathBuf::from(arg_str(args, "file")?);
        let references = arg_str_array(args, "references")?;

        let is_json = file
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("json"));
        let (mut workspace, locations) = if is_json {
            (load_workspace(&file)?, None)
        } else {
            let parsed = structurizr_dsl::parse_file_detailed(&file)
                .map_err(|e| ToolError::Execution(format!("failed to parse DSL from {}: {e}", file.display())))?;
            (parsed.workspace, Some(parsed.locations))
        };
        let generated: std::collections::HashSet<String> =
            structurizr_query::generate_views(&mut workspace).unwrap_or_default().into_iter().collect();
        let catalog = structurizr_query::Catalog::new(&workspace);

        let located = structurizr_web::locate::locate(&catalog, locations.as_ref(), &generated, &references);
        Ok(json_result(structurizr_web::locate::to_json(&located)))
    }

    pub fn lint(args: &Value) -> Result<Value, ToolError> {
        let file = PathBuf::from(arg_str(args, "file")?);
        let mut workspace = load_workspace(&file)?;
        generate_views(&mut workspace);

        let blocking = structurizr_query::lint(&workspace);
        let review = structurizr_query::review(&workspace);
        let warnings: Vec<Value> = review
            .elements
            .iter()
            .flat_map(|el| {
                el.findings.iter().filter(|f| !f.blocking).map(move |f| {
                    json!({
                        "code": f.code,
                        "message": f.message,
                        "elementId": el.id,
                        "name": el.name,
                    })
                })
            })
            .collect();

        let out = json!({
            "blocking": blocking.iter().map(|f| json!({
                "code": f.code,
                "elementId": f.element_id,
                "name": f.name,
                "message": f.message,
            })).collect::<Vec<_>>(),
            "warnings": warnings,
        });
        Ok(json_result(out))
    }

    pub fn diff(args: &Value) -> Result<Value, ToolError> {
        let file = PathBuf::from(arg_str(args, "file")?);
        let from = arg_str_opt(args, "from");
        let to = arg_str_opt(args, "to");
        let against = arg_str_opt(args, "against");

        let (before, after) = if let Some(against) = against {
            let before = load_workspace(&file)?;
            let after = load_workspace(&PathBuf::from(against))?;
            (before, after)
        } else {
            if !structurizr_web::git::is_tracked(&file) {
                return Err(ToolError::Execution(format!(
                    "{} is not tracked by git; pass `against` to compare two workspace files instead",
                    file.display()
                )));
            }
            let from_rev = from.unwrap_or("HEAD");
            let before_source = structurizr_web::git::read(&file, from_rev)
                .map_err(|e| ToolError::Execution(format!("{e:#}")))?;
            let before = parse_str(&before_source)
                .map_err(|e| ToolError::Execution(format!("failed to parse revision {from_rev}: {e}")))?;

            let after = match to {
                None | Some("working") => load_workspace(&file)?,
                Some(to_rev) => {
                    let after_source = structurizr_web::git::read(&file, to_rev)
                        .map_err(|e| ToolError::Execution(format!("{e:#}")))?;
                    parse_str(&after_source)
                        .map_err(|e| ToolError::Execution(format!("failed to parse revision {to_rev}: {e}")))?
                }
            };
            (before, after)
        };

        let diff = structurizr_query::diff(&before, &after);
        Ok(json_result(serde_json::to_value(diff).unwrap_or_default()))
    }

    pub fn render(args: &Value) -> Result<Value, ToolError> {
        let file = PathBuf::from(arg_str(args, "file")?);
        let view = arg_str_opt(args, "view");
        let format = arg_str_opt(args, "format").unwrap_or("svg");

        let mut workspace = load_workspace(&file)?;
        generate_views(&mut workspace);
        let mut diagrams = SvgExporter.export_workspace(&workspace);
        if let Some(key) = view {
            diagrams.retain(|d| d.key == key);
            if diagrams.is_empty() {
                return Err(ToolError::Execution(format!("no view with key '{key}'")));
            }
        }
        if diagrams.is_empty() {
            return Err(ToolError::Execution("workspace has no views to render".to_string()));
        }

        let content = match format {
            "png" => diagrams
                .iter()
                .map(|d| {
                    let bytes = structurizr_renderer::png::svg_to_png(&d.content)
                        .map_err(|e| ToolError::Execution(format!("rasterizing {}: {e}", d.key)))?;
                    Ok(json!({
                        "type": "image",
                        "data": BASE64.encode(bytes),
                        "mimeType": "image/png",
                    }))
                })
                .collect::<Result<Vec<_>, ToolError>>()?,
            "svg" => diagrams
                .iter()
                .map(|d| json!({ "type": "text", "text": format!("{}:\n{}", d.key, d.content) }))
                .collect(),
            other => return Err(ToolError::Invalid(format!("unknown format '{other}'; expected svg or png"))),
        };

        Ok(json!({ "content": content, "isError": false }))
    }

    pub fn add(args: &Value) -> Result<Value, ToolError> {
        let file = PathBuf::from(arg_str(args, "file")?);
        let statement = arg_str(args, "statement")?;
        let parent = arg_str_opt(args, "in").unwrap_or("model");
        crate::edit::add(&file, statement, parent)
            .map(|o| json_result(o.json))
            .map_err(|e| ToolError::Execution(format!("{e:#}")))
    }

    pub fn remove(args: &Value) -> Result<Value, ToolError> {
        let file = PathBuf::from(arg_str(args, "file")?);
        let reference = arg_str(args, "reference")?;
        let cascade = arg_bool(args, "cascade", false);
        crate::edit::remove(&file, reference, cascade)
            .map(|o| json_result(o.json))
            .map_err(|e| ToolError::Execution(format!("{e:#}")))
    }

    pub fn rename(args: &Value) -> Result<Value, ToolError> {
        let file = PathBuf::from(arg_str(args, "file")?);
        let old = arg_str(args, "identifier")?;
        let new = arg_str(args, "newIdentifier")?;
        crate::edit::rename(&file, old, new)
            .map(|o| json_result(o.json))
            .map_err(|e| ToolError::Execution(format!("{e:#}")))
    }

    pub fn format(args: &Value) -> Result<Value, ToolError> {
        let file = PathBuf::from(arg_str(args, "file")?);
        let (text, losses) = crate::edit::format(&file).map_err(|e| ToolError::Execution(format!("{e:#}")))?;
        let mut result = text_result(text);
        if !losses.is_empty() {
            result["structuredContent"] = json!({ "losses": losses });
        }
        Ok(result)
    }

    pub fn docs(_args: &Value) -> Result<Value, ToolError> {
        Ok(text_result(include_str!("../../../llms.txt")))
    }
}

/// Why a workspace file could not be loaded: a DSL parse failure keeps its
/// structured diagnostics; anything else (unreadable file, bad JSON) is
/// opaque. Mirrors `main.rs`'s `LoadError`/`load_workspace_detailed`,
/// duplicated here since this module cannot reach into `main.rs`.
enum LoadError {
    Parse(ParseError),
    Other(String),
}

fn load_workspace_detailed(path: &Path) -> Result<Workspace, LoadError> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext == "json" {
        let content = std::fs::read_to_string(path)
            .map_err(|e| LoadError::Other(format!("failed to read {}: {e}", path.display())))?;
        serde_json::from_str(&content)
            .map_err(|e| LoadError::Other(format!("failed to parse JSON from {}: {e}", path.display())))
    } else {
        parse_file(path).map_err(LoadError::Parse)
    }
}

fn parse_error_json(err: &ParseError, entry: &Path) -> Vec<Value> {
    err.diagnostics()
        .iter()
        .map(|d| {
            let file = ParseError::resolve_file(d, entry);
            json!({
                "code": d.code,
                "file": file.display().to_string(),
                "line": d.line,
                "column": d.column,
                "message": d.message,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(dispatcher: &mut Dispatcher, id: i64, method: &str, params: Value) -> Value {
        let out = dispatcher.handle(
            &json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string(),
        );
        assert_eq!(out.len(), 1, "expected exactly one response for a request");
        serde_json::from_str(&out[0]).unwrap()
    }

    #[test]
    fn initialize_returns_server_info() {
        let mut dispatcher = Dispatcher::new();
        let response = request(&mut dispatcher, 1, "initialize", json!({ "protocolVersion": "2024-11-05", "capabilities": {} }));
        assert_eq!(response["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(response["result"]["serverInfo"]["name"], "structurizrx");
        assert_eq!(response["result"]["capabilities"]["tools"], json!({}));
    }

    #[test]
    fn initialized_notification_is_ignored() {
        let mut dispatcher = Dispatcher::new();
        let out = dispatcher.handle(
            &json!({ "jsonrpc": "2.0", "method": "notifications/initialized", "params": {} }).to_string(),
        );
        assert!(out.is_empty());
    }

    #[test]
    fn tools_list_names_every_tool_with_schemas() {
        let mut dispatcher = Dispatcher::new();
        let response = request(&mut dispatcher, 2, "tools/list", json!({}));
        let tools = response["result"]["tools"].as_array().unwrap();
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(
            names,
            vec![
                "validate", "digest", "query", "locate", "lint", "diff", "render", "docs",
                "add", "remove", "rename", "format",
            ]
        );
        for tool in tools {
            assert!(tool["description"].as_str().is_some_and(|d| !d.is_empty()), "{tool}");
            assert_eq!(tool["inputSchema"]["type"], "object", "{tool}");
            assert!(tool["inputSchema"]["properties"].is_object(), "{tool}");
        }
    }

    fn write_dsl(name: &str, contents: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sx-mcp-test-{}-{}", std::process::id(), name));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("workspace.dsl");
        std::fs::write(&path, contents).unwrap();
        path
    }

    fn call_tool(dispatcher: &mut Dispatcher, name: &str, arguments: Value) -> Value {
        request(dispatcher, 3, "tools/call", json!({ "name": name, "arguments": arguments }))["result"].clone()
    }

    #[test]
    fn validate_reports_a_parse_error_with_a_line_number() {
        let path = write_dsl("validate", "workspace {\n  model {\n");
        let mut dispatcher = Dispatcher::new();
        let result = call_tool(&mut dispatcher, "validate", json!({ "file": path.to_str().unwrap() }));
        assert_eq!(result["isError"], false);
        let structured = &result["structuredContent"];
        assert_eq!(structured["valid"], false);
        let errors = structured["errors"].as_array().unwrap();
        assert_eq!(errors.len(), 1);
        assert!(errors[0]["line"].as_u64().unwrap() > 0, "{errors:?}");
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn digest_of_shop_example_contains_the_system() {
        let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../site/examples/shop.dsl");
        let mut dispatcher = Dispatcher::new();
        let result = call_tool(&mut dispatcher, "digest", json!({ "file": file.to_str().unwrap() }));
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("system Shop"), "{text}");
    }

    #[test]
    fn query_finds_shop_api() {
        let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../site/examples/shop.dsl");
        let mut dispatcher = Dispatcher::new();
        let result = call_tool(
            &mut dispatcher,
            "query",
            json!({ "file": file.to_str().unwrap(), "expression": "element.name==API" }),
        );
        assert_eq!(result["isError"], false);
        let elements = result["structuredContent"]["elements"].as_array().unwrap();
        assert!(
            elements.iter().any(|e| e["path"].as_str() == Some("Shop/API")),
            "{elements:?}"
        );
    }

    #[test]
    fn unknown_method_is_method_not_found() {
        let mut dispatcher = Dispatcher::new();
        let response = request(&mut dispatcher, 4, "textDocument/hover", json!({}));
        assert_eq!(response["error"]["code"], METHOD_NOT_FOUND);
    }

    #[test]
    fn unknown_tool_is_an_error_result_not_a_protocol_error() {
        let mut dispatcher = Dispatcher::new();
        let response = request(&mut dispatcher, 5, "tools/call", json!({ "name": "nope", "arguments": {} }));
        assert!(response.get("error").is_none(), "{response}");
        assert_eq!(response["result"]["isError"], true);
    }

    #[test]
    fn add_then_rename_edits_the_file_and_it_still_parses() {
        let file = write_dsl(
            "edit.dsl",
            "workspace {\n    model {\n        shop = softwareSystem \"Shop\" {\n            api = container \"API\"\n        }\n    }\n}\n",
        );
        let path = file.to_str().unwrap();
        let mut dispatcher = Dispatcher::new();
        let added = call_tool(
            &mut dispatcher,
            "add",
            json!({ "file": path, "statement": "db = container \"DB\"", "in": "shop" }),
        );
        assert_eq!(added["isError"], false, "{added}");
        let renamed = call_tool(
            &mut dispatcher,
            "rename",
            json!({ "file": path, "identifier": "api", "newIdentifier": "gateway" }),
        );
        assert_eq!(renamed["isError"], false, "{renamed}");
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains("db = container \"DB\""), "{text}");
        assert!(text.contains("gateway = container \"API\""), "{text}");
        structurizr_dsl::parse_str(&text).unwrap();
    }

    #[test]
    fn add_that_would_break_the_workspace_writes_nothing() {
        let original = "workspace {\n    model {\n        a = person \"A\"\n    }\n}\n";
        let file = write_dsl("broken.dsl", original);
        let mut dispatcher = Dispatcher::new();
        let result = call_tool(
            &mut dispatcher,
            "add",
            json!({ "file": file.to_str().unwrap(), "statement": "a -> nobody \"calls\"" }),
        );
        assert_eq!(result["isError"], true, "{result}");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), original);
    }

    #[test]
    fn ping_returns_an_empty_result() {
        let mut dispatcher = Dispatcher::new();
        let response = request(&mut dispatcher, 6, "ping", json!({}));
        assert_eq!(response["result"], json!({}));
    }
}
