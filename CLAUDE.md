# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project overview

A Rust re-implementation of [Structurizr](https://structurizr.com/) — a C4 model architecture diagramming toolchain — evolving into an LLM-native architecture description system. It can parse the Structurizr DSL, export to multiple diagram formats (SVG, PNG, PlantUML, Mermaid, DOT), and serve a live-reloading local web viewer. All Rust code lives under `rust/`. The user-facing documentation site (install/quickstart/CLI/language reference) is an mdBook project under `site/`, unrelated to the internal `docs/SPEC.md` design doc.

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
cargo test -p structurizr-lsp

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

`wasm-pack` always builds in release mode by shelling out to its own `cargo build --target wasm32-unknown-unknown`. Cargo's build-directory lock is keyed by profile name only, not by target triple, so if the *outer* build invoking `cargo build -p structurizr-web` (or anything depending on it, like `structurizr-cli`) is itself `--release`, that nested build would contend for the same `target/release/.cargo-lock` the outer build already holds — a self-deadlock (the outer build is blocked inside its own build script waiting on `wasm-pack`, which is blocked waiting for the lock). `build.rs` avoids this by pointing the nested build at its own `CARGO_TARGET_DIR` (`rust/target/wasm-pack`). Don't remove that `.env("CARGO_TARGET_DIR", ...)` call — debug builds happen not to collide (different lock file), so the deadlock only reproduces on `--release`, which makes it easy to miss.

### Docs site build

The mdBook project under `site/` is built separately from `cargo build` — `structurizr-web`'s `build.rs` never invokes mdBook, it only `mkdir -p`s `site/book/` so the `DocsAssets` rust-embed derive doesn't fail to compile before the book has been built for the first time.

```sh
cargo install mdbook   # if not already available
./site/build.sh        # regenerates site/src/images/*.svg from site/examples/*.dsl
                        # (via structurizr-cli render), then runs `mdbook build site`
```

`site/build.sh` also exports every `site/demo/*.dsl` with `export-viewer` into `site/book/demo/<name>/` (the docs' interactive demo). `DocsAssets` excludes `demo/` so that 17 MB copy of the viewer assets is never embedded in the binary. The exported page runs `templates/workspace.html` with `STATIC_EXPORT = true`, which hides everything that needs the server (tool pages, `/api/` buttons, locate, live reload) — gate any new server-backed control on it.

`site/render-examples.sh` (called by `build.sh`) is the only place example diagrams are produced; `site/src/images/` and `site/book/` are both build artifacts (`.gitignore`d), not checked in. `.github/workflows/docs.yml` runs `site/build.sh` on pushes to `main` that touch `site/**` and deploys `site/book/` to GitHub Pages via `actions/deploy-pages`; the repo's Pages source must be set to "GitHub Actions" (Settings → Pages) for that to publish.

## Architecture

The workspace dependency graph flows one way: `structurizr-model` ← `structurizr-dsl` / `structurizr-renderer` ← `structurizr-wasm` / `structurizr-web` ← `structurizr-cli`.

### `structurizr-model`
Pure data types that mirror the [Structurizr JSON schema](https://structurizr.com/json). Every struct derives `Serialize`/`Deserialize` with `#[serde(rename_all = "camelCase")]`. The `Workspace` struct is the root type used everywhere else.

### `structurizr-dsl`
Hand-written lexer (`lexer.rs`) → parser (`parser.rs`) → `Workspace`. The public API is `parse_file(path)` and `parse_str(dsl)`; `emit(ws)` / `emit_with_identifiers(ws, register)` (`emit.rs`) go the other way, model → canonical DSL, and are round-trip tested against every example in `tests/emit.rs`.

The parser recovers at statement boundaries (`Parser::recover_statement`, called from the workspace, model, softwareSystem, container and views loops), so one parse reports every mistake as `ParseError::Multiple`. Use `ParseError::errors()` / `diagnostics()` rather than matching on a single `Syntax` value; `Syntax` carries the `!include`d `file` separately instead of prefixing it to the message. An `IdentifierRegister` (`identifier_register.rs`) tracks DSL-variable-to-element-id bindings during parsing.

`parse_file_detailed` / `parse_str_detailed` also return `SourceLocations` (`source.rs`): model id → file, start line/col and end line of the declaring statement, resolved through `!include`s. Every id the parser reads from source must be allocated through `next_id()` (anchors on the last consumed token — the element keyword) or `next_id_from(token)` (relationships anchor on their source token, since a text-block description moves the last token to a later line); ids it synthesizes (relationships replicated onto deployment instances) use `next_derived_id()` and have no location — follow `linked_relationship_id`.

Integration tests use DSL fixture files from `original-java/structurizr-dsl/src/test/resources/dsl/` — that directory is part of the repo and must stay present for those tests to pass.

### `structurizr-renderer`
Implements the `DiagramExporter` trait:
```rust
pub trait DiagramExporter {
    fn export_workspace(&self, workspace: &Workspace) -> Vec<Diagram>;
}
```
Exporters: `SvgExporter`, `PlantUmlExporter`, `MermaidExporter`, `DotExporter`. All four respect workspace element styles (tag-based colour/stroke overrides). The optional `png` feature gates the `png` module, which exposes `svg_to_png(svg: &str) -> Vec<u8>` and `svg_to_rgba()` via `resvg` — there is no separate `PngExporter` struct. This feature is enabled by `structurizr-wasm` and not by any other crate.

The SVG renderer does its own layout — a simplified Sugiyama hierarchical layout (longest-path layering → barycentric ordering → coordinate assignment), falling back to a grid when there are no edges. Auto-layout is skipped only when every element in the view has a stored `x`/`y` position; if some but not all do, auto-layout still runs for the whole view and the stored positions are then re-applied on top, so unpositioned elements don't collapse onto `(0, 0)`.

### `structurizr-wasm`
Thin `wasm-bindgen` shim over the native renderer. Exposes `render_svg`, `render_png`, `render_first_svg`, `render_first_png`, and `render_to_canvas` (WASM-only). The crate is both a `cdylib` (WASM) and `rlib` (native tests).

`wasm-bindgen` is pinned to the **exact** version `=0.2.118` — do not change this without updating the `wasm-pack` lock as well.

### `structurizr-web`
Axum HTTP server serving a workspace browser at `http://localhost:<port>`. Key routes:
- `GET /workspace/{name}` — workspace overview page
- `GET /workspace/{name}/diagram/{key}` — SVG diagram page
- `GET /workspace/{name}/decisions` — ADR list
- `GET /workspace/{name}/canvas` — Canvas demo (requires WASM build)
- `GET /workspace/{name}/clusters` — cluster analysis: summary, conformance findings, community cards, a community-ordered dependency-structure matrix and a metrics table (`assets/js/structurizr-clusters.js` + `templates/clusters.html` + `assets/css/structurizr-clusters.css`), fed by `/api/workspace/{name}/clusters`. Analysis options live in the query string (`?level=&include=&exclude=&implied=`) so a particular analysis is a link
- `GET /workspace/{name}/review` — element walkthrough: filterable element list plus per-element detail, findings and neighbourhood (`assets/js/structurizr-review.js` + `templates/review.html` + `assets/css/structurizr-review.css`), fed by `/api/workspace/{name}/review`. Review marks live in the URL hash (`#r=<ids>&f=<ids>&e=<id>`), not on the server, so a part-finished review is a shareable link and nothing is written next to the workspace file
- `GET /workspace/{name}/diff` — version comparison: two git revisions of the workspace compared as *models* (`assets/js/structurizr-diff.js` + `templates/diff.html` + `assets/css/structurizr-diff.css`), fed by `/api/workspace/{name}/revisions` and `/api/workspace/{name}/diff`. The pair being compared lives in the query string (`?from=&to=`, `to=working` meaning the file on disk) so a comparison is a link
- `GET /workspace/{name}/print` — print document: every view as one paginated sheet, built client-side (`assets/js/structurizr-print.js` + `templates/print.html` + `assets/css/structurizr-print.css`) and printed from the browser. It captures the *rendered* JointJS diagram via `exportCurrentDiagramToSVG`, not the server-side `SvgExporter` output — the two use different layout engines, so only the client render matches the screen. Fit-to-page is applied in JS after measuring each sheet, because the exported SVG's inline `width`/`height` override any stylesheet rule
- Single-view printing (the workspace page's printer button / `p`, and Ctrl/Cmd+P on the workspace and diagram pages) is separate: `assets/js/structurizr-print-view.js` + `assets/css/structurizr-print-view.css` export the current view and swap it in for the app chrome under `@media print`. Don't merge it with `structurizr-print.js` (the multi-view document). The workspace page's toolbar uses a Bootstrap modal rather than a dropdown, because the vendored `bootstrap-5.3.7.min.js` doesn't include Popper
- Selection: a click selects an element or relationship on the workspace and diagram pages (via `structurizr.ui.Diagram#onCellClicked`/`setSelection`, only when the diagram is not editable) and a node or relationship on the graph page; shift/⌘-click adds. `assets/js/structurizr-selection.js` keeps it in the URL hash as references (`#<view>&sel=<ref>,…` on the workspace page, `#sel=…` elsewhere), restored after every render or reload. References are canonical paths, never ids, and their grammar must match `structurizr-query/src/reference.rs`, which `structurizrx locate` resolves them with
- `GET /workspace/{name}/graph` — universe graph: the whole workspace as one force-directed graph (`assets/js/structurizr-universe-graph.js` + `templates/graph.html`), fed by `/api/workspace/{name}/graph`
- `GET /docs/` — the mdBook documentation site (see "Docs site build" above), served from `assets::DocsAssets` (embeds `site/book/`, separate from the workspace-viewer `assets::Assets` embed); `GET /docs` 308-redirects to it
- `GET /api/workspace/{name}/diagram/{key}/svg` — raw SVG
- `GET /api/workspace/{name}/graph` — `structurizr_query::graph` output as JSON
- `GET /api/workspace/{name}/review` — `structurizr_query::review` output as JSON, served from the `AppState` derived cache
- `GET /api/workspace/{name}/clusters` — `structurizr_query::cluster` output as JSON; cached per workspace *and* per option set, since the same workspace yields a different analysis per level and filter
- `GET /api/workspace/{name}/revisions` — the workspace's git history (`git.rs`); 409 when the file is not tracked
- `GET /api/workspace/{name}/diff?from=&to=` — `structurizr_query::diff` of two revisions, cached per revision pair
- `GET /api/workspace/{name}/locate?sel=` — `locate.rs` (shared with `structurizrx locate`): where the selection's references are declared, with `absolutePath` per location for the viewer's `vscode://file/…` "open in editor" links. `WorkspaceEntry.locations` holds the parser's `SourceLocations` for this
- `WS /ws` — live-reload WebSocket

`git.rs` is the only place that touches git: it shells out to the `git` binary for the workspace's history (`git log` over the entry file *and* every `!include`d file, so a multi-file workspace's history is not just the entry file's) and for a revision's contents (`git show`, with `!include`s spliced from *the same revision* — the DSL parser resolves includes against the filesystem, which would mix one revision's entry file with today's parts). Nothing there writes: no temp file next to the workspace, no index or worktree change.

`AppState` holds a per-workspace `DerivedCache` for artefacts that cost a full index pass to build (`AppState::cached`). The watcher drops the whole cache via `invalidate_derived()` immediately after swapping in freshly parsed workspaces — if you add a derived artefact, add it to `DerivedCache` rather than computing it per request.

HTML templates live in `structurizr-web/src/templates/`. Static assets (CSS, JS, icons, WASM output) are embedded at compile time via `rust-embed` from `structurizr-web/assets/`. The WASM output files land in `assets/wasm/` and are produced by the `build.rs` script.

### `structurizr-lsp`
Language server for the DSL, built on the `structurizr-dsl` lexer/parser. All logic lives in `core.rs` (`Core`), which is synchronous and runtime-agnostic. A `file:` document is parsed with `parse_str_detailed_at` so its `!include`s resolve (the buffer text, not the file on disk); the outline, go-to-definition into included files and diagnostic anchoring all come from the parser's `SourceLocations`. The WASM build has no filesystem and parses without a path. Two front ends drive it: `backend.rs` (tower-lsp over stdio, behind the default `stdio` feature, used by `structurizrx lsp`) and `jsonrpc.rs` (`Dispatcher` — one message in, messages out), which is what compiles for wasm32. tower-lsp/tokio must stay behind the `stdio` feature; neither builds for `wasm32-unknown-unknown`.

### `structurizr-lsp-wasm`
`wasm-bindgen` shim exposing `jsonrpc::Dispatcher` to JavaScript as `LspServer.handle(message) -> string` (a JSON array of outgoing messages). The host owns the message loop; there is no transport in the crate. Built by the VS Code extension's `npm run build:wasm` (`wasm-pack --target nodejs` → `editors/vscode/wasm/`), which lets the extension run the language server with no `structurizrx` binary installed.

### `structurizr-query`
Selector-expression engine (spec §6.2), view generation (`generate_views`, spec §6.3) and the whole-workspace graph projection (`graph`, in `graph.rs` — every element/view/ADR as a node, every relationship/containment/instance/membership as a link; backs the web universe-graph page). Depends only on `structurizr-model`; used by `structurizr-cli` and `structurizr-web`.

`index.rs` is the shared graph index: one pass over the model resolving hierarchy, adjacency (`outgoing`/`incoming`/`children`/`degree`) and view membership (`views_for`). The selector engine, `lint` and `review` all read it rather than re-traversing the workspace. Ordering is deterministic throughout — entries are in model order and adjacency is `Vec`s of indices, never `HashMap` iteration — so anything layered on top can be reproduced run to run. It indexes the static model only (people, systems, containers, components, custom elements); deployment nodes and instances are deliberately absent, matching the selector engine's element kinds. `graph.rs` still does its own traversal and is the projection that *does* include deployment; folding it onto the index would mean widening the index's scope first.

`cluster.rs` is the cluster analysis behind the web clusters page: it projects the model onto one level (`Level`), optionally rolling descendant relationships up onto their ancestor, then reports structure (weakly connected components, cycles as SCCs, articulation points, bridges), per-node metrics (afferent/efferent coupling, instability, PageRank, betweenness) and Louvain communities — and compares those communities against the structure the model *declares* (`group`, else the parent element). The conformance findings are the output worth reading; everything else is input to them. Structural algorithms come from `petgraph`; Louvain and Brandes' betweenness are implemented here because petgraph has neither. Louvain needs no seed: it visits nodes in index order and breaks ties towards the lowest community index, so runs are reproducible by construction rather than by configuration.

`diff.rs` compares two versions of a workspace as models — the read model behind the web version-comparison page. Everything is keyed on an element's **canonical path** (ancestor names + own name) and a relationship's two end paths, never on ids: the DSL assigns ids in parse order, so the same element has a different id in two revisions as soon as anything is declared before it. The consequence is that a rename reads as a removal plus an addition; `Diff::renames` scores the likely pairs (unchanged description, then parent, then technology) and only reports a pair when one candidate wins outright.

`reference.rs` resolves references by canonical path (`Shop/API`, `Shop/API.http`, `A->B "desc"`, `view:<key>`, `decision:<id>`) and parses viewer links (`#<view>&sel=<ref>,…`). Its `Catalog` covers deployment elements too (rooted at their environment), and an undeclared pair resolves to the descendant relationships implying it. Like `diff.rs`, it never keys on ids.

`review.rs` is the read model behind the web review page: every element with its neighbourhood, the views showing it, and hygiene findings. Its checks are deliberately separate from `lint.rs` — `lint` is the gate that `validate --strict` fails on, so adding a review check (`missing-description`, `missing-technology`, `not-in-any-view`, `no-relationships`, `duplicate-name`, `relationship-undescribed`) can never change the exit status of an existing workspace. `lint`'s findings are folded into the review output marked `blocking: true`.

### `structurizr-cli`
Entry point `structurizrx`. Subcommands: `validate [--strict]`, `lint`, `render` (svg/png/mermaid/plantuml/dot), `export`, `export-site`, `export-viewer`, `digest`, `query`, `locate`, `diff`, `clusters`, `graph`, `fmt`, `add`, `remove`, `rename`, `serve`, `lsp`, `mcp`, `docs`. Accepts both `.dsl` and `.json` workspace files. `render` and `serve` materialize generated (`auto`) views before rendering.

- `locate` (`locate.rs`) prints where a path or viewer link's selection is declared; the resolution and JSON shape live in `structurizr_web::locate`, shared with the server's locate API.
- `diff`, `lint`, `clusters`, `graph` (`diff.rs`, `lint.rs`, `clusters.rs`, `graph.rs`) are the CLI faces of the viewer's diff, review, clusters and universe-graph pages; they call the same `structurizr_query` / `structurizr_web::git` functions, so keep the two in step.
- `edit.rs` holds `add` / `remove` / `rename` / `fmt`. The edits are textual (comments and layout survive), addressed through the parser's `SourceLocations`, and every edit is written, re-parsed and rolled back on failure. They return an `Outcome` rather than printing, because `mcp.rs` calls them too.
- `mcp.rs` is a hand-written MCP server (newline-delimited JSON-RPC over stdio, no SDK). Its tools wrap the commands above; add a tool there when you add a command agents should reach.


### `editors/vscode`
VS Code extension. `README.md` is marketplace description. Technicalities go to `DEVELOPER.md`.