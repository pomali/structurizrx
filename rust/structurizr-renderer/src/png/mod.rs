//! PNG rasterization of SVG diagrams.
//!
//! Uses [`resvg`] (pure Rust, WASM-compatible) to rasterize SVG strings into
//! PNG bytes or raw RGBA pixel data.
//!
//! # Example
//!
//! ```rust
//! use structurizr_renderer::png::svg_to_png;
//!
//! let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
//!   <rect width="100" height="100" fill="red"/>
//! </svg>"#;
//! let png_bytes = svg_to_png(svg).unwrap();
//! assert!(!png_bytes.is_empty());
//! ```

use resvg::{tiny_skia, usvg};

// ── Public API ────────────────────────────────────────────────────────────────

/// Convert an SVG string into PNG-encoded bytes.
///
/// Returns an error string on failure (parse error, zero-sized SVG, etc.).
pub fn svg_to_png(svg_content: &str) -> Result<Vec<u8>, String> {
    let pixmap = rasterize(svg_content)?;
    pixmap.encode_png().map_err(|e| e.to_string())
}

/// Convert an SVG string into raw RGBA pixel data.
///
/// Returns `(width, height, rgba_bytes)`.  The pixel data uses
/// **premultiplied** alpha (tiny-skia's native format).  For the fully-opaque
/// diagrams produced by this library every pixel has alpha = 255, so
/// premultiplied and straight alpha are identical in practice.
///
/// This is the preferred path for Canvas rendering in WASM: the caller can
/// wrap the returned bytes in a `Uint8ClampedArray` and create an
/// `ImageData` directly.
pub fn svg_to_rgba(svg_content: &str) -> Result<(u32, u32, Vec<u8>), String> {
    let pixmap = rasterize(svg_content)?;
    Ok((pixmap.width(), pixmap.height(), pixmap.data().to_vec()))
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn rasterize(svg_content: &str) -> Result<tiny_skia::Pixmap, String> {
    let opt = usvg::Options::default();
    let tree =
        usvg::Tree::from_str(svg_content, &opt).map_err(|e| e.to_string())?;

    let width = tree.size().width().ceil() as u32;
    let height = tree.size().height().ceil() as u32;

    let mut pixmap = tiny_skia::Pixmap::new(width.max(1), height.max(1))
        .ok_or_else(|| "Failed to allocate pixmap".to_string())?;

    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

    Ok(pixmap)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_SVG: &str = concat!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="50" height="50">"#,
        r#"<rect width="50" height="50" fill="blue"/></svg>"#
    );

    #[test]
    fn png_bytes_start_with_png_magic() {
        let bytes = svg_to_png(SIMPLE_SVG).unwrap();
        // PNG magic bytes: 0x89 P N G
        assert_eq!(&bytes[..4], &[0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn rgba_dimensions_match_svg() {
        let (w, h, data) = svg_to_rgba(SIMPLE_SVG).unwrap();
        assert_eq!(w, 50);
        assert_eq!(h, 50);
        assert_eq!(data.len() as u32, w * h * 4);
    }

    #[test]
    fn invalid_svg_returns_error() {
        let result = svg_to_png("not svg at all");
        assert!(result.is_err());
    }
}
