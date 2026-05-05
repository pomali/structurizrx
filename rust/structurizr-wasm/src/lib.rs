//! `structurizr-wasm` — render Structurizr diagrams from Rust/WASM.
//!
//! This crate exposes a small, ergonomic API that works both as a native Rust
//! library **and** as a WebAssembly module (via [`wasm-bindgen`]).
//!
//! # Outputs
//!
//! | Function | Output | Works on |
//! |---|---|---|
//! | [`render_svg`] | JSON array of `{key, content}` SVG strings | native + WASM |
//! | [`render_first_svg`] | SVG string of the first diagram | native + WASM |
//! | [`render_png`] | JSON array of `{key, png}` data-URL strings | native + WASM |
//! | [`render_first_png`] | raw PNG bytes of the first diagram | native + WASM |
//! | [`render_to_canvas`] | draws onto an HTML `<canvas>` by DOM id | **WASM only** |
//!
//! # WASM usage (JavaScript / TypeScript)
//!
//! ```js
//! import init, {
//!     render_svg, render_png, render_first_svg,
//!     render_first_png, render_to_canvas
//! } from './structurizr_wasm.js';
//!
//! await init();
//!
//! // Render all diagrams to SVG
//! const diagrams = JSON.parse(render_svg(workspaceJson));
//! document.getElementById('diagram').innerHTML = diagrams[0].content;
//!
//! // Render the first diagram as a PNG data-URL
//! const [{ png }] = JSON.parse(render_png(workspaceJson));
//! document.getElementById('img').src = png;
//!
//! // Draw the first diagram onto a <canvas id="my-canvas">
//! render_to_canvas('my-canvas', workspaceJson);
//! ```
//!
//! # Native Rust usage
//!
//! ```rust
//! use structurizr_wasm::{render_first_svg_str, render_first_png_bytes};
//!
//! // workspace_json is a Structurizr JSON string
//! # let workspace_json = r#"{"name":"Test","model":{},"views":{},"documentation":{},"decisions":[]}"#;
//! let svg: String = render_first_svg_str(workspace_json).unwrap();
//! let png: Vec<u8> = render_first_png_bytes(workspace_json).unwrap();
//! ```

use wasm_bindgen::prelude::*;

use structurizr_model::Workspace;
use structurizr_renderer::{exporter::DiagramExporter, svg::SvgExporter};

// ── Core helpers (native-compatible, return String errors) ────────────────────

fn parse_workspace_str(json: &str) -> Result<Workspace, String> {
    serde_json::from_str(json).map_err(|e| e.to_string())
}

/// Render all diagrams in a Structurizr workspace JSON as a JSON array of
/// `{"key": "…", "content": "<svg>…</svg>"}` objects.
///
/// Returns an error string on failure (e.g. invalid JSON input).
pub fn render_svg_str(workspace_json: &str) -> Result<String, String> {
    let workspace = parse_workspace_str(workspace_json)?;
    let diagrams = SvgExporter.export_workspace(&workspace);
    let result: Vec<_> = diagrams
        .iter()
        .map(|d| serde_json::json!({ "key": d.key, "content": d.content }))
        .collect();
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

/// Render the **first** diagram in a workspace JSON as a raw SVG string.
///
/// Returns an empty string when the workspace contains no views, or an error
/// string on failure.
pub fn render_first_svg_str(workspace_json: &str) -> Result<String, String> {
    let workspace = parse_workspace_str(workspace_json)?;
    let diagrams = SvgExporter.export_workspace(&workspace);
    Ok(diagrams.into_iter().next().map(|d| d.content).unwrap_or_default())
}

/// Render all diagrams in a workspace JSON as PNG data-URLs.
///
/// Returns a JSON array of `{"key": "…", "png": "data:image/png;base64,…"}`
/// objects, or an error string on failure.
pub fn render_png_str(workspace_json: &str) -> Result<String, String> {
    let workspace = parse_workspace_str(workspace_json)?;
    let diagrams = SvgExporter.export_workspace(&workspace);
    let mut result = Vec::new();
    for d in &diagrams {
        let png_bytes = structurizr_renderer::png::svg_to_png(&d.content)?;
        let b64 = base64_encode(&png_bytes);
        result.push(serde_json::json!({
            "key": d.key,
            "png": format!("data:image/png;base64,{}", b64),
        }));
    }
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

/// Render the **first** diagram in a workspace JSON as raw PNG bytes.
///
/// Returns an empty `Vec` when the workspace contains no views, or an error
/// string on failure.
pub fn render_first_png_bytes(workspace_json: &str) -> Result<Vec<u8>, String> {
    let svg = render_first_svg_str(workspace_json)?;
    if svg.is_empty() {
        return Ok(Vec::new());
    }
    structurizr_renderer::png::svg_to_png(&svg)
}

// ── WASM-bindgen shims (convert String errors to JsValue) ─────────────────────

/// Render all diagrams in a Structurizr workspace JSON as a JSON array of
/// `{"key": "…", "content": "<svg>…</svg>"}` objects.
///
/// Pass the returned string through `JSON.parse()` in JavaScript.
#[wasm_bindgen]
pub fn render_svg(workspace_json: &str) -> Result<String, JsValue> {
    render_svg_str(workspace_json).map_err(|e| JsValue::from_str(&e))
}

/// Render the **first** diagram in a workspace JSON as a raw SVG string.
///
/// Returns an empty string when the workspace contains no views.
#[wasm_bindgen]
pub fn render_first_svg(workspace_json: &str) -> Result<String, JsValue> {
    render_first_svg_str(workspace_json).map_err(|e| JsValue::from_str(&e))
}

/// Render all diagrams in a workspace JSON as PNG data-URLs.
///
/// Returns a JSON array of `{"key": "…", "png": "data:image/png;base64,…"}`
/// objects.  Pass the result through `JSON.parse()` in JavaScript.
///
/// The PNG is produced by rasterizing the SVG with
/// [`resvg`](https://crates.io/crates/resvg) — no external tooling required.
#[wasm_bindgen]
pub fn render_png(workspace_json: &str) -> Result<String, JsValue> {
    render_png_str(workspace_json).map_err(|e| JsValue::from_str(&e))
}

/// Render the **first** diagram in a workspace JSON as raw PNG bytes.
///
/// In JavaScript/WASM the returned value is a `Uint8Array`.
/// Returns an empty array when the workspace contains no views.
#[wasm_bindgen]
pub fn render_first_png(workspace_json: &str) -> Result<Vec<u8>, JsValue> {
    render_first_png_bytes(workspace_json).map_err(|e| JsValue::from_str(&e))
}

// ── Canvas (WASM only) ────────────────────────────────────────────────────────

/// Draw the **first** diagram from a workspace JSON onto an HTML `<canvas>`
/// element identified by `canvas_id`.
///
/// The canvas dimensions are set to match the diagram's SVG viewport.  The
/// diagram is rasterized via `resvg` and written as pixel data using the
/// Canvas 2D `putImageData` API.
///
/// This function is **only available when compiled to WebAssembly**.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn render_to_canvas(canvas_id: &str, workspace_json: &str) -> Result<(), JsValue> {
    use wasm_bindgen::JsCast;
    use web_sys::{HtmlCanvasElement, ImageData};

    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window object"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("no document object"))?;

    let canvas = document
        .get_element_by_id(canvas_id)
        .ok_or_else(|| JsValue::from_str(&format!("element '{}' not found", canvas_id)))?
        .dyn_into::<HtmlCanvasElement>()
        .map_err(|_| JsValue::from_str("element is not an HtmlCanvasElement"))?;

    let svg = render_first_svg_str(workspace_json).map_err(|e| JsValue::from_str(&e))?;
    if svg.is_empty() {
        return Ok(());
    }

    // Rasterize the SVG to RGBA pixels using resvg/tiny-skia.
    let (width, height, rgba) = structurizr_renderer::png::svg_to_rgba(&svg)
        .map_err(|e| JsValue::from_str(&e))?;

    canvas.set_width(width);
    canvas.set_height(height);

    let ctx = canvas
        .get_context("2d")?
        .ok_or_else(|| JsValue::from_str("could not get 2d context"))?
        .dyn_into::<web_sys::CanvasRenderingContext2d>()?;

    // Wrap RGBA bytes in a Uint8ClampedArray and create an ImageData.
    // tiny-skia outputs premultiplied RGBA; for fully-opaque diagrams
    // (alpha = 255 everywhere) this equals straight RGBA.
    let clamped = js_sys::Uint8ClampedArray::new_with_length(rgba.len() as u32);
    clamped.copy_from(&rgba);
    let image_data =
        ImageData::new_with_js_u8_clamped_array_and_sh(&clamped, width, height)?;
    ctx.put_image_data(&image_data, 0.0, 0.0)
}

// ── Base64 encoder ────────────────────────────────────────────────────────────

/// Encode `data` as a standard Base64 string (RFC 4648, with padding).
fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        result.push(ALPHABET[((n >> 18) & 0x3f) as usize] as char);
        result.push(ALPHABET[((n >> 12) & 0x3f) as usize] as char);
        result.push(if chunk.len() > 1 {
            ALPHABET[((n >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        result.push(if chunk.len() > 2 {
            ALPHABET[(n & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    result
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal workspace JSON with one system-landscape view.
    fn sample_workspace_json() -> &'static str {
        r#"{
            "name": "Test",
            "model": {
                "people": [{"id":"1","name":"Alice"}],
                "softwareSystems": [{"id":"2","name":"MySystem"}]
            },
            "views": {
                "systemLandscapeViews": [{"key":"Landscape"}]
            },
            "documentation": {},
            "decisions": []
        }"#
    }

    #[test]
    fn render_svg_str_returns_json_array() {
        let json = render_svg_str(sample_workspace_json()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_array());
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["key"], "Landscape");
        assert!(arr[0]["content"].as_str().unwrap().starts_with("<svg"));
    }

    #[test]
    fn render_first_svg_str_returns_svg_string() {
        let svg = render_first_svg_str(sample_workspace_json()).unwrap();
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("Alice"));
        assert!(svg.contains("MySystem"));
    }

    #[test]
    fn render_first_png_bytes_returns_png_bytes() {
        let bytes = render_first_png_bytes(sample_workspace_json()).unwrap();
        // PNG magic: 0x89 P N G
        assert_eq!(&bytes[..4], &[0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn render_png_str_returns_data_urls() {
        let json = render_png_str(sample_workspace_json()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        let png_url = arr[0]["png"].as_str().unwrap();
        assert!(png_url.starts_with("data:image/png;base64,"));
    }

    #[test]
    fn invalid_json_returns_error() {
        assert!(render_first_svg_str("not json").is_err());
        assert!(render_first_png_bytes("not json").is_err());
    }

    #[test]
    fn empty_workspace_returns_empty() {
        let empty = r#"{"name":"Empty","model":{},"views":{},"documentation":{},"decisions":[]}"#;
        assert_eq!(render_first_svg_str(empty).unwrap(), "");
        assert!(render_first_png_bytes(empty).unwrap().is_empty());
    }

    #[test]
    fn base64_encode_roundtrip() {
        let data = b"Hello, World!";
        assert_eq!(base64_encode(data), "SGVsbG8sIFdvcmxkIQ==");
    }
}
