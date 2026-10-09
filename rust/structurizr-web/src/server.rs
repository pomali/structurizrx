//! HTTP server and route handlers.

use axum::extract::ws::{Message, WebSocket};
use axum::{
    extract::{Path, RawQuery, State, WebSocketUpgrade},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
    Json, Router,
};

use structurizr_renderer::{exporter::DiagramExporter, mermaid::MermaidExporter, svg::SvgExporter};

use crate::assets::{Assets, DocsAssets};
use crate::markdown::render_markdown;
use crate::state::{AppState, BroadcastMsg, WorkspaceSummary};

// ---- Embedded templates ----
const INDEX_HTML: &str = include_str!("templates/index.html");
const WORKSPACE_HTML: &str = include_str!("templates/workspace.html");
const DIAGRAM_HTML: &str = include_str!("templates/diagram.html");
const DECISIONS_HTML: &str = include_str!("templates/decisions.html");
const DECISION_HTML: &str = include_str!("templates/decision.html");
const CANVAS_HTML: &str = include_str!("templates/canvas.html");
const GRAPH_HTML: &str = include_str!("templates/graph.html");
const PRINT_HTML: &str = include_str!("templates/print.html");
const REVIEW_HTML: &str = include_str!("templates/review.html");
const CLUSTERS_HTML: &str = include_str!("templates/clusters.html");
const DIFF_HTML: &str = include_str!("templates/diff.html");

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index_handler))
        .route("/workspace/{name}", get(workspace_handler))
        .route("/workspace/{name}/diagram/{key}", get(diagram_handler))
        .route("/workspace/{name}/decisions", get(decisions_handler))
        .route("/workspace/{name}/decisions/{id}", get(decision_handler))
        .route("/workspace/{name}/canvas", get(canvas_handler))
        .route("/workspace/{name}/graph", get(graph_handler))
        .route("/workspace/{name}/print", get(print_handler))
        .route("/workspace/{name}/review", get(review_handler))
        .route("/workspace/{name}/clusters", get(clusters_handler))
        .route("/workspace/{name}/diff", get(diff_handler))
        .route("/api/workspaces", get(api_workspaces_handler))
        .route("/api/workspace/{name}", get(api_workspace_handler))
        .route(
            "/api/workspace/{name}/decisions",
            get(api_decisions_handler),
        )
        .route(
            "/api/workspace/{name}/decisions/{id}",
            get(api_decision_handler),
        )
        .route(
            "/api/workspace/{name}/diagram/{key}/svg",
            get(api_diagram_svg_handler),
        )
        .route(
            "/api/workspace/{name}/diagram/{key}/mermaid",
            get(api_diagram_mermaid_handler),
        )
        .route("/api/workspace/{name}/graph", get(api_graph_handler))
        .route("/api/workspace/{name}/review", get(api_review_handler))
        .route("/api/workspace/{name}/clusters", get(api_clusters_handler))
        .route(
            "/api/workspace/{name}/revisions",
            get(api_revisions_handler),
        )
        .route("/api/workspace/{name}/diff", get(api_diff_handler))
        .route("/api/workspace/{name}/digest", get(api_digest_handler))
        .route("/api/workspace/{name}/query", get(api_query_handler))
        .route("/api/workspace/{name}/locate", get(api_locate_handler))
        .route("/llms.txt", get(llms_txt_handler))
        .route("/docs", get(|| async { Redirect::permanent("/docs/") }))
        .route(
            "/docs/",
            get(|| async { docs_asset_response("index.html") }),
        )
        .route("/docs/{*path}", get(docs_handler))
        .route("/static/{*path}", get(static_handler))
        .route("/ws", get(ws_handler))
        .layer(axum::middleware::from_fn(no_store_dynamic_responses))
        .with_state(state)
}

/// Everything this server produces outside `/static/` is derived from workspace
/// files that change under live reload, so it must never be cached. Without
/// this the browser applies heuristic caching to `/api/workspace/{name}` and a
/// reload can re-render the previous version of the workspace.
async fn no_store_dynamic_responses(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let is_static = request.uri().path().starts_with("/static/");
    let mut response = next.run(request).await;
    if !is_static {
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("no-store"),
        );
    }
    response
}

// ---- Page handlers ----

/// The one-page DSL extension cheat sheet (spec §9.4), for agents and humans.
async fn llms_txt_handler() -> ([(axum::http::HeaderName, &'static str); 1], &'static str) {
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; charset=utf-8",
        )],
        include_str!("../../../llms.txt"),
    )
}

async fn index_handler() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn workspace_handler(Path(name): Path<String>) -> Html<String> {
    Html(
        WORKSPACE_HTML
            .replace("{{WORKSPACE_NAME}}", &html_escape(&name))
            .replace("{{WORKSPACE_SLUG}}", &js_escape(&name)),
    )
}

async fn diagram_handler(Path((name, key)): Path<(String, String)>) -> Html<String> {
    Html(
        DIAGRAM_HTML
            .replace("{{WORKSPACE_NAME}}", &html_escape(&name))
            .replace("{{WORKSPACE_SLUG}}", &js_escape(&name))
            .replace("{{DIAGRAM_KEY}}", &js_escape(&key)),
    )
}

async fn decisions_handler(Path(name): Path<String>) -> Html<String> {
    Html(
        DECISIONS_HTML
            .replace("{{WORKSPACE_NAME}}", &html_escape(&name))
            .replace("{{WORKSPACE_SLUG_ATTR}}", &html_escape(&name))
            .replace("{{WORKSPACE_SLUG}}", &js_escape(&name)),
    )
}

async fn decision_handler(Path((name, id)): Path<(String, String)>) -> Html<String> {
    Html(
        DECISION_HTML
            .replace("{{WORKSPACE_NAME}}", &html_escape(&name))
            .replace("{{WORKSPACE_SLUG_ATTR}}", &html_escape(&name))
            .replace("{{WORKSPACE_SLUG}}", &js_escape(&name))
            .replace("{{DECISION_ID}}", &js_escape(&id)),
    )
}

async fn canvas_handler(Path(name): Path<String>) -> Html<String> {
    Html(
        CANVAS_HTML
            .replace("{{WORKSPACE_NAME}}", &html_escape(&name))
            .replace("{{WORKSPACE_SLUG_ATTR}}", &html_escape(&name))
            .replace("{{WORKSPACE_SLUG}}", &js_escape(&name)),
    )
}

async fn graph_handler(Path(name): Path<String>) -> Html<String> {
    Html(
        GRAPH_HTML
            .replace("{{WORKSPACE_NAME}}", &html_escape(&name))
            .replace("{{WORKSPACE_SLUG_ATTR}}", &html_escape(&name))
            .replace("{{WORKSPACE_SLUG}}", &js_escape(&name)),
    )
}

/// The print document page. Every view is rendered client-side by the JointJS
/// diagram and serialised there, so this handler only serves the shell.
async fn print_handler(Path(name): Path<String>) -> Html<String> {
    Html(
        PRINT_HTML
            .replace("{{WORKSPACE_NAME}}", &html_escape(&name))
            .replace("{{WORKSPACE_SLUG_ATTR}}", &html_escape(&name))
            .replace("{{WORKSPACE_SLUG}}", &js_escape(&name)),
    )
}

/// The element walkthrough / review page.
async fn review_handler(Path(name): Path<String>) -> Html<String> {
    Html(
        REVIEW_HTML
            .replace("{{WORKSPACE_NAME}}", &html_escape(&name))
            .replace("{{WORKSPACE_SLUG_ATTR}}", &html_escape(&name))
            .replace("{{WORKSPACE_SLUG}}", &js_escape(&name)),
    )
}

/// The version-comparison page.
async fn diff_handler(Path(name): Path<String>) -> Html<String> {
    Html(
        DIFF_HTML
            .replace("{{WORKSPACE_NAME}}", &html_escape(&name))
            .replace("{{WORKSPACE_SLUG_ATTR}}", &html_escape(&name))
            .replace("{{WORKSPACE_SLUG}}", &js_escape(&name)),
    )
}

/// The cluster analysis page.
async fn clusters_handler(Path(name): Path<String>) -> Html<String> {
    Html(
        CLUSTERS_HTML
            .replace("{{WORKSPACE_NAME}}", &html_escape(&name))
            .replace("{{WORKSPACE_SLUG_ATTR}}", &html_escape(&name))
            .replace("{{WORKSPACE_SLUG}}", &js_escape(&name)),
    )
}

// ---- JSON API ----

async fn api_workspaces_handler(State(state): State<AppState>) -> impl IntoResponse {
    let workspaces = state.workspaces.lock().unwrap();
    let summaries: Vec<WorkspaceSummary> = workspaces.iter().map(WorkspaceSummary::from).collect();
    Json(summaries)
}

async fn api_workspace_handler(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Response {
    let workspaces = state.workspaces.lock().unwrap();
    if let Some(entry) = workspaces.iter().find(|e| e.name == name) {
        match serde_json::to_string(&entry.workspace) {
            Ok(json) => (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/json")],
                json,
            )
                .into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        }
    } else {
        (
            StatusCode::NOT_FOUND,
            format!("Workspace '{}' not found", name),
        )
            .into_response()
    }
}

async fn api_decisions_handler(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Response {
    let workspaces = state.workspaces.lock().unwrap();
    if let Some(entry) = workspaces.iter().find(|e| e.name == name) {
        let decisions = entry
            .workspace
            .documentation
            .as_ref()
            .and_then(|d| d.decisions.as_ref())
            .cloned()
            .unwrap_or_default();
        match serde_json::to_string(&decisions) {
            Ok(json) => (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/json")],
                json,
            )
                .into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        }
    } else {
        (
            StatusCode::NOT_FOUND,
            format!("Workspace '{}' not found", name),
        )
            .into_response()
    }
}

async fn api_decision_handler(
    State(state): State<AppState>,
    Path((name, id)): Path<(String, String)>,
) -> Response {
    let workspaces = state.workspaces.lock().unwrap();
    if let Some(entry) = workspaces.iter().find(|e| e.name == name) {
        let decision = entry
            .workspace
            .documentation
            .as_ref()
            .and_then(|d| d.decisions.as_ref())
            .and_then(|ds| ds.iter().find(|d| d.id == id))
            .cloned();
        match decision {
            Some(mut d) => {
                let fmt = d.format.to_lowercase();
                if fmt == "markdown" || fmt.is_empty() {
                    d.content = render_markdown(&d.content);
                    d.format = "HTML".to_string();
                }
                match serde_json::to_string(&d) {
                    Ok(json) => (
                        StatusCode::OK,
                        [(header::CONTENT_TYPE, "application/json")],
                        json,
                    )
                        .into_response(),
                    Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
                }
            }
            None => (
                StatusCode::NOT_FOUND,
                format!("Decision '{}' not found", id),
            )
                .into_response(),
        }
    } else {
        (
            StatusCode::NOT_FOUND,
            format!("Workspace '{}' not found", name),
        )
            .into_response()
    }
}

/// Plain-text model digest (same output as `structurizrx digest`), sized for
/// pasting into LLM context.
///
/// `GET /api/workspace/{name}/digest`
/// Every element with its neighbourhood, views and hygiene findings.
///
/// Served from [`AppState::cached`]: the body is the same for every reader
/// until the workspace file changes, and building it costs a full index pass.
async fn api_review_handler(State(state): State<AppState>, Path(name): Path<String>) -> Response {
    let workspace = {
        let workspaces = state.workspaces.lock().unwrap();
        match workspaces.iter().find(|e| e.name == name) {
            Some(entry) => entry.workspace.clone(),
            None => return (StatusCode::NOT_FOUND, "Workspace not found").into_response(),
        }
    };

    let body = state.cached(
        &name,
        |c| c.review_json.clone(),
        |c, v| c.review_json = Some(v),
        || {
            serde_json::to_string(&structurizr_query::review(&workspace))
                .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
        },
    );

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        body.as_str().to_owned(),
    )
        .into_response()
}

/// Query parameters for the cluster analysis.
#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct ClusterParams {
    /// `softwareSystem`, `container` (default) or `component`.
    level: Option<String>,
    /// Comma-separated tag lists.
    include: Option<String>,
    exclude: Option<String>,
    /// `false` to analyse only relationships declared directly between nodes
    /// at the chosen level.
    implied: Option<String>,
}

fn tag_list(value: &Option<String>) -> Vec<String> {
    value
        .as_deref()
        .unwrap_or("")
        .split(',')
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

/// Structural analysis, communities and conformance for one workspace.
///
/// `GET /api/workspace/{name}/clusters?level=container&implied=true`
async fn api_clusters_handler(
    State(state): State<AppState>,
    Path(name): Path<String>,
    axum::extract::Query(params): axum::extract::Query<ClusterParams>,
) -> Response {
    let workspace = {
        let workspaces = state.workspaces.lock().unwrap();
        match workspaces.iter().find(|e| e.name == name) {
            Some(entry) => entry.workspace.clone(),
            None => return (StatusCode::NOT_FOUND, "Workspace not found").into_response(),
        }
    };

    let options = structurizr_query::ClusterOptions {
        level: structurizr_query::Level::parse(params.level.as_deref().unwrap_or("container")),
        include_tags: tag_list(&params.include),
        exclude_tags: tag_list(&params.exclude),
        implied: params.implied.as_deref() != Some("false"),
    };

    // The analysis depends on the options as well as the workspace, so they
    // are part of the cache key.
    let key = format!(
        "{}|{}|{}|{}",
        options.level.kind_name(),
        options.include_tags.join(","),
        options.exclude_tags.join(","),
        options.implied
    );

    let body = state.cached(
        &name,
        |c| c.cluster_json.get(&key).cloned(),
        |c, v| {
            c.cluster_json.insert(key.clone(), v);
        },
        || {
            serde_json::to_string(&structurizr_query::cluster(&workspace, &options))
                .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
        },
    );

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        body.as_str().to_owned(),
    )
        .into_response()
}

// ---- Version comparison ----

/// The workspace file's history, for the revision pickers.
///
/// `GET /api/workspace/{name}/revisions`
///
/// 409 (rather than 404) when the workspace is not in git: the workspace
/// exists, it just has no history to offer, and the page says so.
async fn api_revisions_handler(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Response {
    let Some(path) = workspace_path(&state, &name) else {
        return (
            StatusCode::NOT_FOUND,
            format!("Workspace '{}' not found", name),
        )
            .into_response();
    };

    if !crate::git::is_tracked(&path) {
        return (
            StatusCode::CONFLICT,
            format!(
                "{} is not tracked by git, so it has no versions to compare. \
                 Commit it (or open a workspace inside a repository) and reload.",
                path.display()
            ),
        )
            .into_response();
    }

    match crate::git::history(&path, 200) {
        Ok(history) => Json(history).into_response(),
        Err(e) => (
            StatusCode::CONFLICT,
            format!("No git history for {}: {e:#}", path.display()),
        )
            .into_response(),
    }
}

#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct DiffParams {
    /// The earlier revision. Any git revision, defaulting to `HEAD`.
    from: Option<String>,
    /// The later revision, or `working` (the default) for the file on disk.
    to: Option<String>,
}

/// One side of a comparison, as the page labels it.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DiffSide {
    /// The revision as it was asked for (`HEAD~3`, a sha, or `working`).
    rev: String,
    /// Full sha, absent for the working copy.
    sha: Option<String>,
}

/// Compare two versions of the workspace.
///
/// `GET /api/workspace/{name}/diff?from=<rev>&to=<rev|working>`
///
/// Both sides are parsed and compared as *models*, not as text: see
/// [`structurizr_query::diff`] for why the comparison is keyed on element
/// paths rather than ids.
async fn api_diff_handler(
    State(state): State<AppState>,
    Path(name): Path<String>,
    axum::extract::Query(params): axum::extract::Query<DiffParams>,
) -> Response {
    let (Some(path), Some(current)) = (
        workspace_path(&state, &name),
        current_workspace(&state, &name),
    ) else {
        return (
            StatusCode::NOT_FOUND,
            format!("Workspace '{}' not found", name),
        )
            .into_response();
    };

    let from = params.from.unwrap_or_else(|| "HEAD".to_string());
    let to = params.to.unwrap_or_else(|| crate::git::WORKING.to_string());
    let key = format!("{from}|{to}");

    // The working copy is whatever the server last parsed, so a comparison
    // involving it is only valid until the next reload — which is exactly when
    // the whole derived cache is dropped.
    let load = |rev: &str| -> Result<structurizr_model::Workspace, String> {
        if rev == crate::git::WORKING {
            return Ok(current.clone());
        }
        let source = crate::git::read(&path, rev).map_err(|e| format!("{e:#}"))?;
        crate::resolver::workspace_from_source(
            &source,
            crate::resolver::is_json(&path),
            &format!("revision {rev}"),
        )
        .map_err(|e| format!("{e:#}"))
    };

    let before = match load(&from) {
        Ok(ws) => ws,
        Err(e) => return (StatusCode::BAD_REQUEST, e).into_response(),
    };
    let after = match load(&to) {
        Ok(ws) => ws,
        Err(e) => return (StatusCode::BAD_REQUEST, e).into_response(),
    };

    let side = |rev: &str| DiffSide {
        rev: rev.to_string(),
        sha: if rev == crate::git::WORKING {
            None
        } else {
            crate::git::resolve(&path, rev).ok()
        },
    };

    let body = state.cached(
        &name,
        |c| c.diff_json.get(&key).cloned(),
        |c, v| {
            c.diff_json.insert(key.clone(), v);
        },
        || {
            let payload = serde_json::json!({
                "from": side(&from),
                "to": side(&to),
                "diff": structurizr_query::diff(&before, &after),
            });
            serde_json::to_string(&payload).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
        },
    );

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        body.as_str().to_owned(),
    )
        .into_response()
}

/// The file a workspace was loaded from, cloned out so the lock is not held
/// across the git commands that follow.
fn workspace_path(state: &AppState, name: &str) -> Option<std::path::PathBuf> {
    let workspaces = state.workspaces.lock().unwrap();
    workspaces
        .iter()
        .find(|e| e.name == name)
        .map(|e| e.source_path.clone())
}

fn current_workspace(state: &AppState, name: &str) -> Option<structurizr_model::Workspace> {
    let workspaces = state.workspaces.lock().unwrap();
    workspaces
        .iter()
        .find(|e| e.name == name)
        .map(|e| e.workspace.clone())
}

async fn api_digest_handler(State(state): State<AppState>, Path(name): Path<String>) -> Response {
    let workspaces = state.workspaces.lock().unwrap();
    let Some(entry) = workspaces.iter().find(|e| e.name == name) else {
        return (
            StatusCode::NOT_FOUND,
            format!("Workspace '{}' not found", name),
        )
            .into_response();
    };
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        structurizr_query::digest(&entry.workspace),
    )
        .into_response()
}

/// The whole workspace as one graph — every element, view and documentation
/// artefact as a node, every relationship, containment and membership as a
/// link. Backs the universe-graph page.
///
/// `GET /api/workspace/{name}/graph`
async fn api_graph_handler(State(state): State<AppState>, Path(name): Path<String>) -> Response {
    let workspaces = state.workspaces.lock().unwrap();
    let Some(entry) = workspaces.iter().find(|e| e.name == name) else {
        return (
            StatusCode::NOT_FOUND,
            format!("Workspace '{}' not found", name),
        )
            .into_response();
    };
    Json(structurizr_query::graph(&entry.workspace)).into_response()
}

#[derive(serde::Deserialize)]
struct QueryExprParams {
    expr: String,
}

/// Run a selector expression (spec §6.2) against the live workspace.
///
/// `GET /api/workspace/{name}/query?expr=element.tag==Database`
///
/// Returns `{elements: [{id, name}], relationships: [id]}`; a bad expression
/// returns 400 with the engine's error text (which names valid paths).
async fn api_query_handler(
    State(state): State<AppState>,
    Path(name): Path<String>,
    axum::extract::Query(params): axum::extract::Query<QueryExprParams>,
) -> Response {
    let workspaces = state.workspaces.lock().unwrap();
    let Some(entry) = workspaces.iter().find(|e| e.name == name) else {
        return (
            StatusCode::NOT_FOUND,
            format!("Workspace '{}' not found", name),
        )
            .into_response();
    };
    match structurizr_query::query(&params.expr, &entry.workspace) {
        Ok(selection) => {
            let names = structurizr_query::element_names(&entry.workspace);
            let out = serde_json::json!({
                "elements": selection.elements.iter().map(|id| serde_json::json!({
                    "id": id,
                    "name": names.get(id),
                })).collect::<Vec<_>>(),
                "relationships": selection.relationships.iter().collect::<Vec<_>>(),
            });
            Json(out).into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

/// Where references are declared — `structurizrx locate --json` against the
/// live workspace. References come in `sel=` exactly as the viewer's page hash
/// carries them (each URI-encoded, joined with `,`), so a selection can be
/// passed on as-is. Backs the viewer's "open in editor" links.
///
/// `GET /api/workspace/{name}/locate?sel=Shop%2FAPI,User-%3EShop%2FAPI`
async fn api_locate_handler(
    State(state): State<AppState>,
    Path(name): Path<String>,
    RawQuery(query): RawQuery,
) -> Response {
    let workspaces = state.workspaces.lock().unwrap();
    let Some(entry) = workspaces.iter().find(|e| e.name == name) else {
        return (
            StatusCode::NOT_FOUND,
            format!("Workspace '{}' not found", name),
        )
            .into_response();
    };
    let references =
        structurizr_query::parse_viewer_link(&format!("#{}", query.unwrap_or_default()))
            .map(|link| link.selection)
            .unwrap_or_default();
    let catalog = structurizr_query::Catalog::new(&entry.workspace);
    let located = crate::locate::locate(
        &catalog,
        entry.locations.as_ref(),
        &std::collections::HashSet::new(),
        &references,
    );
    Json(crate::locate::to_json(&located)).into_response()
}

// ---- Static assets ----

/// Render a single diagram as an SVG using the built-in Rust renderer.
///
/// `GET /api/workspace/{name}/diagram/{key}/svg`
///
/// Returns `image/svg+xml` on success, or a plain-text error with an
/// appropriate HTTP status code on failure.
async fn api_diagram_svg_handler(
    State(state): State<AppState>,
    Path((name, key)): Path<(String, String)>,
) -> Response {
    render_diagram(
        &state,
        &name,
        &key,
        &SvgExporter,
        "image/svg+xml; charset=utf-8",
    )
}

/// The Mermaid source for a single diagram.
///
/// `GET /api/workspace/{name}/diagram/{key}/mermaid`
///
/// Returns `text/plain` on success, or a plain-text error with an appropriate
/// HTTP status code on failure.
async fn api_diagram_mermaid_handler(
    State(state): State<AppState>,
    Path((name, key)): Path<(String, String)>,
) -> Response {
    render_diagram(
        &state,
        &name,
        &key,
        &MermaidExporter,
        "text/plain; charset=utf-8",
    )
}

/// Look up a workspace, export it with `exporter` and return the diagram whose
/// key matches, or a 404 explaining which lookup failed.
fn render_diagram(
    state: &AppState,
    name: &str,
    key: &str,
    exporter: &dyn DiagramExporter,
    content_type: &'static str,
) -> Response {
    let workspaces = state.workspaces.lock().unwrap();
    let Some(entry) = workspaces.iter().find(|e| e.name == name) else {
        return (
            StatusCode::NOT_FOUND,
            format!("Workspace '{}' not found", name),
        )
            .into_response();
    };

    let diagrams = exporter.export_workspace(&entry.workspace);
    let Some(diagram) = diagrams.into_iter().find(|d| d.key == key) else {
        return (
            StatusCode::NOT_FOUND,
            format!("Diagram '{}' not found in workspace '{}'", key, name),
        )
            .into_response();
    };

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, content_type)],
        diagram.content,
    )
        .into_response()
}

/// Serve the pre-built mdBook documentation site embedded from `site/book/`
/// (see `assets::DocsAssets`). `structurizr-web` never builds the book
/// itself — it only serves whatever `mdbook build site` last produced.
async fn docs_handler(Path(path): Path<String>) -> Response {
    docs_asset_response(&path)
}

fn docs_asset_response(path: &str) -> Response {
    match DocsAssets::get(path) {
        Some(content) => {
            let mime = mime_from_path(path);
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, mime)],
                content.data.as_ref().to_vec(),
            )
                .into_response()
        }
        None if path == "index.html" => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            "<h1>Docs not built yet</h1><p>Run <code>mdbook build site</code> \
             from the repository root, then restart <code>structurizrx serve</code>.</p>",
        )
            .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            format!("Doc page not found: {}", path),
        )
            .into_response(),
    }
}

async fn static_handler(Path(path): Path<String>) -> Response {
    match Assets::get(&path) {
        Some(content) => {
            let mime = mime_from_path(&path);
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, mime)],
                content.data.as_ref().to_vec(),
            )
                .into_response()
        }
        None => (StatusCode::NOT_FOUND, format!("Asset not found: {}", path)).into_response(),
    }
}

fn mime_from_path(path: &str) -> &'static str {
    if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if path.ends_with(".wasm") {
        "application/wasm"
    } else if path.ends_with(".js") {
        "application/javascript; charset=utf-8"
    } else if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".gif") {
        "image/gif"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".woff") {
        "font/woff"
    } else if path.ends_with(".woff2") {
        "font/woff2"
    } else if path.ends_with(".ttf") {
        "font/ttf"
    } else if path.ends_with(".json") {
        "application/json"
    } else {
        "application/octet-stream"
    }
}

// ---- WebSocket live reload ----

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| ws_session(socket, state))
}

async fn ws_session(mut socket: WebSocket, state: AppState) {
    let mut rx = state.tx.subscribe();
    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(BroadcastMsg::Reload) => {
                        let payload = r#"{"type":"reload"}"#;
                        if socket.send(Message::Text(payload.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(_)) => {} // ignore client messages
                    _ => break,       // client closed
                }
            }
        }
    }
}

// ---- Helpers ----

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn js_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('"', "\\\"")
}
