use std::{
    collections::HashMap,
    fs,
    hash::{Hash, Hasher},
    io::{self, BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use structurizr_dsl::ParseError;
use structurizr_model::{validation, Workspace};
use structurizr_renderer::{
    dot::DotExporter, exporter::DiagramExporter, mermaid::MermaidExporter,
    plantuml::PlantUmlExporter, svg::SvgExporter,
};

pub(crate) fn run(root: PathBuf) -> Result<()> {
    let mut server = McpServer::new(root.canonicalize().unwrap_or(root));
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();

    while let Some(raw) = read_message(&mut reader)? {
        let response = server.handle(&raw);
        write_message(&mut writer, &response)?;
    }
    Ok(())
}

struct McpServer {
    root: PathBuf,
    next_transaction_id: u64,
    transactions: HashMap<String, PendingPatch>,
}

struct PendingPatch {
    file: PathBuf,
    original_hash: u64,
    new_content: String,
}

impl McpServer {
    fn new(root: PathBuf) -> Self {
        Self {
            root,
            next_transaction_id: 1,
            transactions: HashMap::new(),
        }
    }

    fn handle(&mut self, raw: &str) -> Value {
        let Ok(request) = serde_json::from_str::<Value>(raw) else {
            return rpc_error(Value::Null, -32700, "invalid JSON");
        };
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let params = request.get("params").cloned().unwrap_or(Value::Null);

        match method {
            "initialize" => rpc_ok(
                id,
                json!({
                    "protocolVersion": "2024-11-05",
                    "serverInfo": {
                        "name": "structurizrx-mcp",
                        "version": env!("CARGO_PKG_VERSION"),
                    },
                    "capabilities": {
                        "tools": {}
                    }
                }),
            ),
            "notifications/initialized" => Value::Null,
            "tools/list" => rpc_ok(id, json!({ "tools": self.tools() })),
            "tools/call" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
                match self.call_tool(&name, &args) {
                    Ok(v) => rpc_ok(id, v),
                    Err(e) => rpc_ok(id, tool_error("tool-call-failed", &format!("{:#}", e), &[], &[])),
                }
            }
            "ping" => rpc_ok(id, json!({})),
            _ => rpc_error(id, -32601, &format!("unsupported method: {method}")),
        }
    }

    fn tools(&self) -> Vec<Value> {
        vec![
            tool(
                "workspace.list",
                "List workspaces from a file or directory path",
                json!({
                    "type":"object",
                    "properties":{"path":{"type":"string","description":"Workspace file or directory (optional, default server root)"}},
                    "additionalProperties": false
                }),
            ),
            tool(
                "workspace.validate",
                "Parse, validate and lint a workspace with structured diagnostics",
                json!({
                    "type":"object",
                    "properties":{
                        "file":{"type":"string"},
                        "strict":{"type":"boolean","default":false}
                    },
                    "required":["file"],
                    "additionalProperties": false
                }),
            ),
            tool(
                "workspace.digest",
                "Get token-aware digest: small/medium/full or selector-focused",
                json!({
                    "type":"object",
                    "properties":{
                        "file":{"type":"string"},
                        "size":{"type":"string","enum":["small","medium","full"],"default":"medium"},
                        "selector":{"type":"string","description":"Optional selector for focused retrieval"}
                    },
                    "required":["file"],
                    "additionalProperties": false
                }),
            ),
            tool(
                "workspace.query",
                "Run a selector query and return matched elements/relationships",
                json!({
                    "type":"object",
                    "properties":{
                        "file":{"type":"string"},
                        "expression":{"type":"string"}
                    },
                    "required":["file","expression"],
                    "additionalProperties": false
                }),
            ),
            tool(
                "workspace.render",
                "Render diagrams to svg/mermaid/plantuml/dot and return written files",
                json!({
                    "type":"object",
                    "properties":{
                        "file":{"type":"string"},
                        "format":{"type":"string","enum":["svg","mermaid","plantuml","dot"],"default":"svg"},
                        "output":{"type":"string","description":"Output directory (optional)"}
                    },
                    "required":["file"],
                    "additionalProperties": false
                }),
            ),
            tool(
                "patch.preview",
                "Preview safe file edits and create a transaction (no write)",
                json!({
                    "type":"object",
                    "properties":{
                        "file":{"type":"string"},
                        "newText":{"type":"string"},
                        "edits":{
                            "type":"array",
                            "items":{
                                "type":"object",
                                "properties":{
                                    "start":{"$ref":"#/definitions/position"},
                                    "end":{"$ref":"#/definitions/position"},
                                    "text":{"type":"string"}
                                },
                                "required":["start","end","text"]
                            }
                        }
                    },
                    "definitions":{
                        "position":{
                            "type":"object",
                            "properties":{"line":{"type":"integer","minimum":1},"column":{"type":"integer","minimum":1}},
                            "required":["line","column"]
                        }
                    },
                    "required":["file"],
                    "additionalProperties": false
                }),
            ),
            tool(
                "patch.apply",
                "Apply a previously previewed transaction if file is unchanged",
                json!({
                    "type":"object",
                    "properties":{"transactionId":{"type":"string"}},
                    "required":["transactionId"],
                    "additionalProperties": false
                }),
            ),
        ]
    }

    fn call_tool(&mut self, name: &str, args: &Value) -> Result<Value> {
        match name {
            "workspace.list" => self.workspace_list(args),
            "workspace.validate" => self.workspace_validate(args),
            "workspace.digest" => self.workspace_digest(args),
            "workspace.query" => self.workspace_query(args),
            "workspace.render" => self.workspace_render(args),
            "patch.preview" => self.patch_preview(args),
            "patch.apply" => self.patch_apply(args),
            _ => Ok(tool_error(
                "unknown-tool",
                &format!("unknown tool '{name}'"),
                &[],
                &[String::from("Call tools/list to discover supported tool names.")],
            )),
        }
    }

    fn workspace_list(&self, args: &Value) -> Result<Value> {
        let path = optional_str(args, "path")
            .map(|s| self.resolve_path(s))
            .unwrap_or_else(|| self.root.clone());
        let entries = structurizr_web::resolver::resolve(&path)?;
        let out: Vec<Value> = entries
            .into_iter()
            .map(|entry| {
                json!({
                    "name": entry.name,
                    "displayName": entry.display_name,
                    "sourcePath": entry.source_path.display().to_string(),
                    "elementCount": count_elements(&entry.workspace),
                    "viewCount": count_views(&entry.workspace),
                })
            })
            .collect();
        Ok(tool_ok(
            json!({
                "ok": true,
                "root": self.root.display().to_string(),
                "path": path.display().to_string(),
                "workspaces": out,
            }),
            format!("{} workspace(s)", out.len()),
        ))
    }

    fn workspace_validate(&self, args: &Value) -> Result<Value> {
        let file = required_str(args, "file")?;
        let strict = args.get("strict").and_then(Value::as_bool).unwrap_or(false);
        let path = self.resolve_path(file);
        let parsed = load_workspace_with_parse_details(&path);

        let mut errors = Vec::new();
        let mut lint = Vec::new();
        let mut valid = false;

        match parsed {
            Ok(workspace) => {
                let model_errors = validation::validate(&workspace);
                for e in model_errors {
                    let code = e.code().to_string();
                    let message = e.to_string();
                    let suggestions = extract_did_you_mean(&message);
                    let next_steps = validate_next_steps(&code, &suggestions);
                    errors.push(json!({
                        "code": code,
                        "message": message,
                        "span": Value::Null,
                        "suggestions": suggestions,
                        "affectedPaths": [path.display().to_string()],
                        "nextSteps": next_steps,
                    }));
                }
                for f in structurizr_query::lint(&workspace) {
                    lint.push(json!({
                        "code": f.code,
                        "elementId": f.element_id,
                        "name": f.name,
                        "message": f.message,
                        "affectedPaths": [path.display().to_string()],
                    }));
                }
                valid = errors.is_empty();
            }
            Err(parse_issue) => {
                let suggestions = extract_did_you_mean(&parse_issue.message);
                let next_steps = validate_next_steps(&parse_issue.code, &suggestions);
                errors.push(json!({
                    "code": parse_issue.code,
                    "message": parse_issue.message,
                    "span": parse_issue.span,
                    "suggestions": suggestions,
                    "affectedPaths": [path.display().to_string()],
                    "nextSteps": next_steps,
                }));
            }
        }

        let failed = !valid || (strict && !lint.is_empty());
        let mut next_steps = vec![String::from("Fix reported errors and re-run workspace.validate.")];
        if strict && !lint.is_empty() {
            next_steps.push(String::from("Address lint findings because strict=true."));
        }
        if errors.is_empty() && lint.is_empty() {
            next_steps.clear();
        }

        Ok(tool_ok(
            json!({
                "ok": !failed,
                "file": path.display().to_string(),
                "strict": strict,
                "valid": valid,
                "errors": errors,
                "lint": lint,
                "nextSteps": next_steps,
            }),
            if failed {
                String::from("Validation failed")
            } else {
                String::from("Validation passed")
            },
        ))
    }

    fn workspace_query(&self, args: &Value) -> Result<Value> {
        let file = required_str(args, "file")?;
        let expression = required_str(args, "expression")?;
        let path = self.resolve_path(file);
        let workspace = crate::load_workspace(&path)?;
        match structurizr_query::query(expression, &workspace) {
            Ok(selection) => {
                let names = structurizr_query::element_names(&workspace);
                let elements: Vec<Value> = selection
                    .elements
                    .iter()
                    .map(|id| {
                        json!({
                            "id": id,
                            "name": names.get(id),
                        })
                    })
                    .collect();
                let relationships: Vec<String> =
                    selection.relationships.iter().cloned().collect();
                Ok(tool_ok(
                    json!({
                        "ok": true,
                        "file": path.display().to_string(),
                        "expression": expression,
                        "elements": elements,
                        "relationships": relationships,
                        "nextSteps": if selection.elements.is_empty() && selection.relationships.is_empty() {
                            vec![String::from("Try a broader selector (e.g. `*` or `->target->`).")]
                        } else {
                            Vec::<String>::new()
                        },
                    }),
                    format!(
                        "{} elements, {} relationships",
                        selection.elements.len(),
                        selection.relationships.len()
                    ),
                ))
            }
            Err(e) => {
                let message = e.to_string();
                let (code, span, next_steps) = match e {
                    structurizr_query::QueryError::Parse { offset, .. } => (
                        "query-parse",
                        json!({ "offset": offset }),
                        vec![String::from("Fix selector syntax and retry workspace.query.")],
                    ),
                    structurizr_query::QueryError::UnknownPath { .. } => (
                        "query-unknown-path",
                        Value::Null,
                        vec![String::from(
                            "Use one of the valid paths listed in the error message.",
                        )],
                    ),
                    structurizr_query::QueryError::UnknownTarget(_) => (
                        "query-unknown-target",
                        Value::Null,
                        vec![String::from(
                            "Use an existing identifier as neighborhood target.",
                        )],
                    ),
                };
                Ok(json!({
                    "isError": true,
                    "content": [{"type":"text","text": format!("[{}] {}", code, message)}],
                    "structuredContent": {
                        "ok": false,
                        "file": path.display().to_string(),
                        "expression": expression,
                        "code": code,
                        "message": message,
                        "span": span,
                        "affectedPaths": [path.display().to_string()],
                        "nextSteps": next_steps,
                    }
                }))
            }
        }
    }

    fn workspace_digest(&self, args: &Value) -> Result<Value> {
        let file = required_str(args, "file")?;
        let size = args
            .get("size")
            .and_then(Value::as_str)
            .unwrap_or("medium")
            .to_lowercase();
        let selector = optional_str(args, "selector");
        let path = self.resolve_path(file);
        let mut workspace = crate::load_workspace(&path)?;
        if let Err(e) = structurizr_query::generate_views(&mut workspace) {
            return Ok(tool_error(
                "view-generation",
                &format!("view generation failed: {e}"),
                &[path.display().to_string()],
                &[String::from("Fix view-generation errors, then call workspace.digest again.")],
            ));
        }
        let max_chars = match size.as_str() {
            "small" => Some(2_000usize),
            "full" => None,
            _ => Some(8_000usize),
        };

        if let Some(expr) = selector {
            let selection = structurizr_query::query(expr, &workspace)
                .map_err(|e| anyhow!("focused digest selector error: {}", e))?;
            let names = structurizr_query::element_names(&workspace);
            let mut lines = vec![format!("focused digest: {}", expr)];
            lines.push(String::new());
            for id in &selection.elements {
                lines.push(format!(
                    "element {} {}",
                    id,
                    names
                        .get(id)
                        .cloned()
                        .unwrap_or_else(|| String::from("<unknown>"))
                ));
            }
            for id in &selection.relationships {
                lines.push(format!("relationship {}", id));
            }
            if selection.elements.is_empty() && selection.relationships.is_empty() {
                lines.push(String::from("(no matches)"));
            }
            let focused = lines.join("\n");
            let (text, truncated) = trim_to_chars(&focused, max_chars);
            return Ok(tool_ok(
                json!({
                    "ok": true,
                    "file": path.display().to_string(),
                    "size": size,
                    "selector": expr,
                    "truncated": truncated,
                    "text": text,
                    "elementCount": selection.elements.len(),
                    "relationshipCount": selection.relationships.len(),
                }),
                text,
            ));
        }

        let digest = structurizr_query::digest(&workspace);
        let (text, truncated) = trim_to_chars(&digest, max_chars);
        Ok(tool_ok(
            json!({
                "ok": true,
                "file": path.display().to_string(),
                "size": size,
                "truncated": truncated,
                "text": text,
            }),
            text,
        ))
    }

    fn workspace_render(&self, args: &Value) -> Result<Value> {
        let file = required_str(args, "file")?;
        let format = args
            .get("format")
            .and_then(Value::as_str)
            .unwrap_or("svg")
            .to_lowercase();
        let path = self.resolve_path(file);
        let output_dir = optional_str(args, "output")
            .map(|p| self.resolve_path(p))
            .unwrap_or_else(|| self.root.join("out"));
        let mut workspace = crate::load_workspace(&path)?;
        let generated = structurizr_query::generate_views(&mut workspace)
            .map_err(|e| anyhow!("view generation: {}", e))?;
        fs::create_dir_all(&output_dir)
            .with_context(|| format!("Cannot create output dir {}", output_dir.display()))?;

        let diagrams = match format.as_str() {
            "mermaid" => MermaidExporter.export_workspace(&workspace),
            "dot" | "graphviz" => DotExporter.export_workspace(&workspace),
            "svg" => SvgExporter.export_workspace(&workspace),
            _ => PlantUmlExporter.export_workspace(&workspace),
        };

        let mut files = Vec::new();
        for d in &diagrams {
            let filename = output_dir.join(format!("{}.{}", d.key, d.extension()));
            fs::write(&filename, &d.content)
                .with_context(|| format!("Cannot write {}", filename.display()))?;
            files.push(json!({
                "key": d.key,
                "path": filename.display().to_string(),
                "bytes": d.content.len(),
                "extension": d.extension(),
            }));
        }

        Ok(tool_ok(
            json!({
                "ok": true,
                "file": path.display().to_string(),
                "format": format,
                "output": output_dir.display().to_string(),
                "generatedViews": generated,
                "diagramCount": diagrams.len(),
                "files": files,
                "nextSteps": if diagrams.is_empty() {
                    vec![String::from("No diagrams were produced; check whether the workspace has renderable views.")]
                } else {
                    Vec::<String>::new()
                },
            }),
            format!("Rendered {} diagram(s)", diagrams.len()),
        ))
    }

    fn patch_preview(&mut self, args: &Value) -> Result<Value> {
        let file = required_str(args, "file")?;
        let path = self.resolve_path(file);
        let old_content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let new_content = if let Some(new_text) = args.get("newText").and_then(Value::as_str) {
            new_text.to_string()
        } else {
            apply_range_edits(&old_content, args.get("edits").unwrap_or(&Value::Null))?
        };
        let tx_id = format!("tx-{}", self.next_transaction_id);
        self.next_transaction_id += 1;
        self.transactions.insert(
            tx_id.clone(),
            PendingPatch {
                file: path.clone(),
                original_hash: content_hash(&old_content),
                new_content: new_content.clone(),
            },
        );
        let preview = preview_diff(&old_content, &new_content, &path);
        Ok(tool_ok(
            json!({
                "ok": true,
                "transactionId": tx_id,
                "file": path.display().to_string(),
                "changed": old_content != new_content,
                "preview": preview,
                "nextSteps": ["Call patch.apply with transactionId to write this change."],
            }),
            preview,
        ))
    }

    fn patch_apply(&mut self, args: &Value) -> Result<Value> {
        let tx = required_str(args, "transactionId")?;
        let Some(pending) = self.transactions.remove(tx) else {
            return Ok(tool_error(
                "unknown-transaction",
                &format!("transaction '{tx}' not found"),
                &[],
                &[String::from("Run patch.preview first to create a transaction.")],
            ));
        };
        let current = fs::read_to_string(&pending.file)
            .with_context(|| format!("Failed to read {}", pending.file.display()))?;
        let current_hash = content_hash(&current);
        if current_hash != pending.original_hash {
            return Ok(tool_error(
                "file-changed",
                "file has changed since preview",
                &[pending.file.display().to_string()],
                &[String::from(
                    "Re-run patch.preview on the current file content before applying.",
                )],
            ));
        }
        fs::write(&pending.file, pending.new_content.as_bytes())
            .with_context(|| format!("Failed to write {}", pending.file.display()))?;
        Ok(tool_ok(
            json!({
                "ok": true,
                "applied": true,
                "file": pending.file.display().to_string(),
                "nextSteps": ["Re-run workspace.validate to verify changes."],
            }),
            format!("Applied patch to {}", pending.file.display()),
        ))
    }

    fn resolve_path(&self, raw: &str) -> PathBuf {
        let p = PathBuf::from(raw);
        if p.is_absolute() {
            p
        } else {
            self.root.join(p)
        }
    }
}

struct ParseIssue {
    code: String,
    message: String,
    span: Value,
}

fn load_workspace_with_parse_details(path: &Path) -> Result<Workspace, ParseIssue> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext == "json" {
        let content = fs::read_to_string(path).map_err(|e| ParseIssue {
            code: "io".to_string(),
            message: format!("Failed to read {}: {}", path.display(), e),
            span: Value::Null,
        })?;
        serde_json::from_str(&content).map_err(|e| ParseIssue {
            code: "json-parse".to_string(),
            message: format!("Failed to parse JSON from {}: {}", path.display(), e),
            span: Value::Null,
        })
    } else {
        structurizr_dsl::parse_file(path).map_err(|e| match e {
            ParseError::Syntax { line, col, message } => ParseIssue {
                code: "parse".to_string(),
                message,
                span: json!({ "line": line, "column": col }),
            },
            other => ParseIssue {
                code: "parse".to_string(),
                message: other.to_string(),
                span: Value::Null,
            },
        })
    }
}

fn validate_next_steps(code: &str, suggestions: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    match code {
        "parse" => out.push(String::from(
            "Fix syntax at the reported span and re-run workspace.validate.",
        )),
        "unknown-element" => out.push(String::from(
            "Declare the missing identifier or rename the relationship endpoint.",
        )),
        "unknown-port" => out.push(String::from(
            "Create the referenced port on the endpoint element or update the port name.",
        )),
        "unknown-milestone" => out.push(String::from(
            "Declare the milestone in `milestones { ... }` or remove the reference.",
        )),
        _ => out.push(String::from("Fix the reported issue and re-run workspace.validate.")),
    }
    if let Some(first) = suggestions.first() {
        out.push(format!("Try replacing with suggested identifier: '{}'.", first));
    }
    out
}

fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
    })
}

fn tool_ok(structured: Value, text: String) -> Value {
    json!({
        "isError": false,
        "content": [{"type":"text","text": text}],
        "structuredContent": structured,
    })
}

fn tool_error(code: &str, message: &str, affected_paths: &[String], next_steps: &[String]) -> Value {
    json!({
        "isError": true,
        "content": [{"type":"text","text": format!("[{}] {}", code, message)}],
        "structuredContent": {
            "ok": false,
            "code": code,
            "message": message,
            "affectedPaths": affected_paths,
            "nextSteps": next_steps,
        }
    })
}

fn rpc_ok(id: Value, result: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"result":result})
}

fn rpc_error(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}

fn required_str<'a>(obj: &'a Value, key: &str) -> Result<&'a str> {
    obj.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing required string '{}'", key))
}

fn optional_str<'a>(obj: &'a Value, key: &str) -> Option<&'a str> {
    obj.get(key).and_then(Value::as_str)
}

fn trim_to_chars(input: &str, max_chars: Option<usize>) -> (String, bool) {
    let Some(max) = max_chars else {
        return (input.to_string(), false);
    };
    if input.chars().count() <= max {
        return (input.to_string(), false);
    }
    let mut out = String::new();
    for (i, ch) in input.chars().enumerate() {
        if i >= max {
            break;
        }
        out.push(ch);
    }
    out.push_str("\n… [truncated]");
    (out, true)
}

fn count_elements(ws: &Workspace) -> usize {
    let mut n = 0usize;
    n += ws.model.people.as_ref().map_or(0, Vec::len);
    n += ws.model.software_systems.as_ref().map_or(0, Vec::len);
    n += ws.model.custom_elements.as_ref().map_or(0, Vec::len);
    if let Some(systems) = &ws.model.software_systems {
        for s in systems {
            n += s.containers.as_ref().map_or(0, Vec::len);
            if let Some(containers) = &s.containers {
                for c in containers {
                    n += c.components.as_ref().map_or(0, Vec::len);
                }
            }
        }
    }
    n
}

fn count_views(ws: &Workspace) -> usize {
    let mut n = 0usize;
    n += ws.views.system_landscape_views.as_ref().map_or(0, Vec::len);
    n += ws.views.system_context_views.as_ref().map_or(0, Vec::len);
    n += ws.views.container_views.as_ref().map_or(0, Vec::len);
    n += ws.views.component_views.as_ref().map_or(0, Vec::len);
    n += ws.views.dynamic_views.as_ref().map_or(0, Vec::len);
    n += ws.views.deployment_views.as_ref().map_or(0, Vec::len);
    n += ws.views.filtered_views.as_ref().map_or(0, Vec::len);
    n += ws.views.image_views.as_ref().map_or(0, Vec::len);
    n += ws.views.custom_views.as_ref().map_or(0, Vec::len);
    n
}

fn extract_did_you_mean(message: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = message.as_bytes();
    let mut i = 0usize;
    while i + 13 < bytes.len() {
        if message[i..].starts_with("did you mean '") {
            let start = i + 13;
            if let Some(end_rel) = message[start..].find('\'') {
                out.push(message[start..start + end_rel].to_string());
                i = start + end_rel + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn apply_range_edits(content: &str, edits_value: &Value) -> Result<String> {
    let edits = edits_value
        .as_array()
        .ok_or_else(|| anyhow!("provide either `newText` or an `edits` array"))?;
    let mut normalized = Vec::new();
    for edit in edits {
        let start = edit
            .get("start")
            .ok_or_else(|| anyhow!("edit.start is required"))?;
        let end = edit
            .get("end")
            .ok_or_else(|| anyhow!("edit.end is required"))?;
        let text = edit
            .get("text")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("edit.text must be a string"))?;
        let s_line = start.get("line").and_then(Value::as_u64).ok_or_else(|| anyhow!("start.line missing"))? as usize;
        let s_col = start.get("column").and_then(Value::as_u64).ok_or_else(|| anyhow!("start.column missing"))? as usize;
        let e_line = end.get("line").and_then(Value::as_u64).ok_or_else(|| anyhow!("end.line missing"))? as usize;
        let e_col = end.get("column").and_then(Value::as_u64).ok_or_else(|| anyhow!("end.column missing"))? as usize;
        let start_offset = line_col_to_offset(content, s_line, s_col)?;
        let end_offset = line_col_to_offset(content, e_line, e_col)?;
        if end_offset < start_offset {
            return Err(anyhow!("edit range end precedes start"));
        }
        normalized.push((start_offset, end_offset, text.to_string()));
    }
    normalized.sort_by_key(|(start, _, _)| *start);
    for pair in normalized.windows(2) {
        if pair[0].1 > pair[1].0 {
            return Err(anyhow!("overlapping edits are not allowed"));
        }
    }
    let mut out = content.to_string();
    for (start, end, replacement) in normalized.into_iter().rev() {
        out.replace_range(start..end, &replacement);
    }
    Ok(out)
}

fn line_col_to_offset(content: &str, line: usize, column: usize) -> Result<usize> {
    if line == 0 || column == 0 {
        return Err(anyhow!("line/column are 1-based"));
    }
    let mut current_line = 1usize;
    let mut line_start = 0usize;
    for (idx, ch) in content.char_indices() {
        if current_line == line {
            line_start = if current_line == 1 { 0 } else { line_start };
            break;
        }
        if ch == '\n' {
            current_line += 1;
            line_start = idx + ch.len_utf8();
        }
    }
    if current_line != line {
        if line == 1 {
            line_start = 0;
        } else {
            return Err(anyhow!("line {} out of range", line));
        }
    }
    let line_tail = &content[line_start..];
    let mut char_count = 1usize;
    for (rel_idx, ch) in line_tail.char_indices() {
        if char_count == column {
            return Ok(line_start + rel_idx);
        }
        if ch == '\n' {
            return Err(anyhow!("column {} out of range for line {}", column, line));
        }
        char_count += 1;
    }
    if char_count == column {
        Ok(content.len())
    } else {
        Err(anyhow!("column {} out of range for line {}", column, line))
    }
}

fn preview_diff(old: &str, new: &str, path: &Path) -> String {
    if old == new {
        return format!("No changes for {}", path.display());
    }
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();
    let mut first = 0usize;
    let min = old_lines.len().min(new_lines.len());
    while first < min && old_lines[first] == new_lines[first] {
        first += 1;
    }
    let old_snip = old_lines.get(first).copied().unwrap_or("");
    let new_snip = new_lines.get(first).copied().unwrap_or("");
    format!(
        "--- {}\n+++ {}\n@@ line {} @@\n-{}\n+{}",
        path.display(),
        path.display(),
        first + 1,
        old_snip,
        new_snip
    )
}

fn content_hash(content: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

fn read_message<R: BufRead>(reader: &mut R) -> Result<Option<String>> {
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            return Ok(None);
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some(v) = trimmed.strip_prefix("Content-Length:") {
            content_length = Some(v.trim().parse::<usize>()?);
        }
    }
    let len = content_length.ok_or_else(|| anyhow!("missing Content-Length header"))?;
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body)?;
    Ok(Some(String::from_utf8(body)?))
}

fn write_message<W: Write>(writer: &mut W, value: &Value) -> Result<()> {
    if value.is_null() {
        return Ok(());
    }
    let body = serde_json::to_string(value)?;
    write!(writer, "Content-Length: {}\r\n\r\n{}", body.len(), body)?;
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_to_chars_marks_truncation() {
        let (text, truncated) = trim_to_chars("abcdef", Some(3));
        assert!(truncated);
        assert!(text.starts_with("abc"));
    }

    #[test]
    fn extract_suggestion_from_message() {
        let suggestions = extract_did_you_mean("unknown element (did you mean 'shop'?)");
        assert_eq!(suggestions, vec!["shop"]);
    }
}
