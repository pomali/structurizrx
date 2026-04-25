//! HTTP server and route handlers.

use axum::{
    extract::{Path, State, WebSocketUpgrade},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::get,
    Json, Router,
};
use axum::extract::ws::{Message, WebSocket};

use structurizr_renderer::{exporter::DiagramExporter, svg::SvgExporter};

use crate::assets::Assets;
use crate::markdown::render_markdown;
use crate::state::{AppState, BroadcastMsg, WorkspaceSummary};

// ---- Embedded templates ----
const INDEX_HTML: &str = include_str!("templates/index.html");
const WORKSPACE_HTML: &str = include_str!("templates/workspace.html");
const DIAGRAM_HTML: &str = include_str!("templates/diagram.html");
const DECISIONS_HTML: &str = include_str!("templates/decisions.html");
const DECISION_HTML: &str = include_str!("templates/decision.html");

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index_handler))
        .route("/workspace/{name}", get(workspace_handler))
        .route("/workspace/{name}/diagram/{key}", get(diagram_handler))
        .route("/workspace/{name}/decisions", get(decisions_handler))
        .route("/workspace/{name}/decisions/{id}", get(decision_handler))
        .route("/api/workspaces", get(api_workspaces_handler))
        .route("/api/workspace/{name}", get(api_workspace_handler))
        .route("/api/workspace/{name}/decisions", get(api_decisions_handler))
        .route("/api/workspace/{name}/decisions/{id}", get(api_decision_handler))
        .route("/api/workspace/{name}/diagram/{key}/svg", get(api_diagram_svg_handler))
        .route("/static/{*path}", get(static_handler))
        .route("/ws", get(ws_handler))
        .with_state(state)
}

// ---- Page handlers ----

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
            .replace("{{WORKSPACE_SLUG}}", &js_escape(&name)),
    )
}

async fn decision_handler(Path((name, id)): Path<(String, String)>) -> Html<String> {
    Html(
        DECISION_HTML
            .replace("{{WORKSPACE_NAME}}", &html_escape(&name))
            .replace("{{WORKSPACE_SLUG}}", &js_escape(&name))
            .replace("{{DECISION_ID}}", &js_escape(&id)),
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
        (StatusCode::NOT_FOUND, format!("Workspace '{}' not found", name)).into_response()
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
        (StatusCode::NOT_FOUND, format!("Workspace '{}' not found", name)).into_response()
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
            None => (StatusCode::NOT_FOUND, format!("Decision '{}' not found", id)).into_response(),
        }
    } else {
        (StatusCode::NOT_FOUND, format!("Workspace '{}' not found", name)).into_response()
    }
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
    let workspaces = state.workspaces.lock().unwrap();
    let Some(entry) = workspaces.iter().find(|e| e.name == name) else {
        return (StatusCode::NOT_FOUND, format!("Workspace '{}' not found", name)).into_response();
    };

    let diagrams = SvgExporter.export_workspace(&entry.workspace);
    let Some(diagram) = diagrams.into_iter().find(|d| d.key == key) else {
        return (
            StatusCode::NOT_FOUND,
            format!("Diagram '{}' not found in workspace '{}'", key, name),
        )
            .into_response();
    };

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "image/svg+xml; charset=utf-8")],
        diagram.content,
    )
        .into_response()
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
    if path.ends_with(".js") {
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

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
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
