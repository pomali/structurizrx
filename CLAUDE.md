# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project overview

A Rust re-implementation of [Structurizr](https://structurizr.com/) — a C4 model architecture diagramming toolchain — evolving into an LLM-native architecture description system. It can parse the Structurizr DSL, export to multiple diagram formats (SVG, PNG, PlantUML, Mermaid, DOT), and serve a live-reloading local web viewer. All Rust code lives under `rust/`.

The extended design (ports, relationship kinds, milestones, sketch mode, generated views) is specified in `docs/SPEC.md` — read it before design or implementation work on those features. Its §0 decisions log records settled design decisions; notably, upstream Structurizr interop is a non-goal (we read upstream DSL, but our extensions need not be valid upstream).

## Build & test commands

All `cargo` commands should be run from the `rust/` directory.

```sh
cd rust

# Build everything
cargo build

# Build the CLI binary (package structurizr-cli, binary name structurizrx)
cargo build -p structurizr-cli

# Run all tests
cargo test

# Run tests for a single crate
cargo test -p structurizr-dsl
cargo test -p structurizr-renderer
cargo test -p structurizr-wasm

# Run a specific test
cargo test -p structurizr-renderer svg_exporter_relationships

# Lint
cargo clippy

# Run the CLI
cargo run -p structurizr-cli -- validate path/to/workspace.dsl
cargo run -p structurizr-cli -- render path/to/workspace.dsl --format svg --output ./out
cargo run -p structurizr-cli -- export path/to/workspace.dsl
cargo run -p structurizr-cli -- serve path/to/workspace.dsl --port 3000 --open
```

### WASM build

`structurizr-web`'s `build.rs` invokes `wasm-pack` automatically if it is installed. Without it the web server still builds but the `/workspace/<name>/canvas` demo page shows a setup message.

```sh
cargo install wasm-pack
# then `cargo build -p structurizr-web` picks it up automatically
```

## Architecture

The workspace dependency graph flows one way: `structurizr-model` ← `structurizr-dsl` / `structurizr-renderer` ← `structurizr-wasm` / `structurizr-web` ← `structurizr-cli`.

### `structurizr-model`
Pure data types that mirror the [Structurizr JSON schema](https://structurizr.com/json). Every struct derives `Serialize`/`Deserialize` with `#[serde(rename_all = "camelCase")]`. The `Workspace` struct is the root type used everywhere else.

### `structurizr-dsl`
Hand-written lexer (`lexer.rs`) → parser (`parser.rs`) → `Workspace`. The public API is `parse_file(path)` and `parse_str(dsl)`. An `IdentifierRegister` (`identifier_register.rs`) tracks DSL-variable-to-element-id bindings during parsing.

Integration tests use DSL fixture files from `original-java/structurizr-dsl/src/test/resources/dsl/` — that directory is part of the repo and must stay present for those tests to pass.

### `structurizr-renderer`
Implements the `DiagramExporter` trait:
```rust
pub trait DiagramExporter {
    fn export_workspace(&self, workspace: &Workspace) -> Vec<Diagram>;
}
```
Exporters: `SvgExporter`, `PlantUmlExporter`, `MermaidExporter`, `DotExporter`. All four respect workspace element styles (tag-based colour/stroke overrides). The optional `png` feature gates the `png` module, which exposes `svg_to_png(svg: &str) -> Vec<u8>` and `svg_to_rgba()` via `resvg` — there is no separate `PngExporter` struct. This feature is enabled by `structurizr-wasm` and not by any other crate.

The SVG renderer does its own layout — a simplified Sugiyama hierarchical layout (longest-path layering → barycentric ordering → coordinate assignment), falling back to a grid when there are no edges. Layout is only run when no stored `x`/`y` positions exist on `ElementView`s.

### `structurizr-wasm`
Thin `wasm-bindgen` shim over the native renderer. Exposes `render_svg`, `render_png`, `render_first_svg`, `render_first_png`, and `render_to_canvas` (WASM-only). The crate is both a `cdylib` (WASM) and `rlib` (native tests).

`wasm-bindgen` is pinned to the **exact** version `=0.2.118` — do not change this without updating the `wasm-pack` lock as well.

### `structurizr-web`
Axum HTTP server serving a workspace browser at `http://localhost:<port>`. Key routes:
- `GET /workspace/{name}` — workspace overview page
- `GET /workspace/{name}/diagram/{key}` — SVG diagram page
- `GET /workspace/{name}/decisions` — ADR list
- `GET /workspace/{name}/canvas` — Canvas demo (requires WASM build)
- `GET /api/workspace/{name}/diagram/{key}/svg` — raw SVG
- `WS /ws` — live-reload WebSocket

HTML templates live in `structurizr-web/src/templates/`. Static assets (CSS, JS, icons, WASM output) are embedded at compile time via `rust-embed` from `structurizr-web/assets/`. The WASM output files land in `assets/wasm/` and are produced by the `build.rs` script.

### `structurizr-query`
Selector-expression engine (spec §6.2) and view generation (`generate_views`, spec §6.3). Depends only on `structurizr-model`; used by `structurizr-cli` and `structurizr-web`.

### `structurizr-cli`
Entry point `structurizrx`. Subcommands: `validate [--strict]`, `render`, `export`, `digest`, `query`, `serve`. Accepts both `.dsl` and `.json` workspace files. `render` and `serve` materialize generated (`auto`) views before rendering.
