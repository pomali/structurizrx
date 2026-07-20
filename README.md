# StructurizrX

A Rust toolchain for describing software architecture as a **plain-text model**
and generating diagrams from it — a re-implementation of the
[Structurizr](https://structurizr.com/) / [C4 model](https://c4model.com/)
toolchain, evolving into an **LLM-native architecture description system**:
one master model, views as queries over it, and tool feedback precise enough
that both humans and LLM agents can self-correct from error messages alone.

```
structurizrx validate ws.dsl
parse error at line 5, column 17: unknown element identifier 'shoop'
in relationship (did you mean 'shop'?)
```

## Quick start

The fastest way to try it is a **sketch** — a file with no `workspace` block,
just arrows. Unknown names become placeholder systems; `?` marks uncertainty:

```
customer -> shop "buys things"
shop -> billing "somehow charges" ?
```

```sh
structurizrx serve sketch.dsl --open     # live-reloading diagram in the browser
```

A full workspace uses the Structurizr DSL (StructurizrX reads standard
upstream DSL) plus extensions for ports, relationship kinds, status,
milestones, and generated views:

```
workspace "Shop" {
    model {
        customer = person "Customer"
        shop = softwareSystem "Shop" {
            web = container "Web App" "Storefront" "TypeScript"
            api = container "API" "Rust" {
                status implemented
                port rest "Customer REST API" { protocol "HTTPS/JSON" }
            }
            db = container "Database" "PostgreSQL" { tags "Database" }
            web -> api.rest "calls"
            api -> db "reads and writes" { kind sync }
        }
        customer -> web "shops on"
    }
    views {
        auto                     // generated default view set
        auto focus api           // neighborhood view around one element
        auto lint                // placeholders, orphans, unbound ports
    }
}
```

```sh
structurizrx validate ws.dsl --strict    # errors + lint findings (add --json for tooling)
structurizrx render ws.dsl --format svg --output ./out
structurizrx serve ws.dsl --port 3000 --open
```

## Install

With Homebrew:

```sh
brew tap pomali/structurizrx https://github.com/pomali/structurizrx
brew install structurizrx
```

Or build from source (Rust toolchain required):

```sh
git clone https://github.com/pomali/structurizrx
cd structurizrx/rust
cargo build --release -p structurizr-cli    # binary: target/release/structurizrx
```

## Commands

| Command | What it does |
|---|---|
| `validate <ws> [--strict] [--json]` | Parse + validate; `--strict` also fails on lint findings; `--json` emits `{valid, errors[], lint[]}` with stable codes |
| `render <ws> --format svg\|mermaid\|plantuml\|dot` | Export diagrams (materializes generated views first) |
| `serve <ws\|dir> [--port N] [--open]` | Live-reloading web viewer with a JSON API |
| `digest <ws>` | Compact plain-text model + view summary, sized for LLM context |
| `query <ws> "<selector>" [--json]` | Run a selector expression, e.g. `element.tag==Database` or `->api->2` |
| `export <ws>` | Workspace JSON (superset of the Structurizr JSON schema) |
| `docs` | Print the DSL cheat sheet ([llms.txt](llms.txt)) |

Errors are **strict by default**: unknown identifiers, misplaced or misspelled
keywords fail with the offending file and line (include-aware), the accepted
keywords for that context, and a "did you mean" suggestion. Forward references
are legal; `!sketch` opts into leniency.

The `serve` API mirrors the CLI for agents working against a live server:
`/llms.txt`, `/api/workspaces`, `/api/workspace/{name}` (model JSON),
`…/digest`, `…/query?expr=`, and `…/diagram/{key}/svg`.

## Design

The extended design is specified in [docs/SPEC.md](docs/SPEC.md). The short
version: diagrams rot because they state each fact once per diagram. Here the
**model is the single source of truth** and views are *selections over it* —
stored as queries (`auto focus api`, `auto slice element.status==idea`),
generated deterministically, never hand-maintained. The
[llms.txt](llms.txt) cheat sheet is the one-page format reference (also served
by `structurizrx serve` and printed by `structurizrx docs`).

## Repository layout

All Rust code lives under [`rust/`](rust/) as a Cargo workspace:

- **`structurizr-model`** — data types mirroring (and extending) the Structurizr JSON schema
- **`structurizr-dsl`** — hand-written lexer/parser for the DSL dialect
- **`structurizr-query`** — selector engine, view generation, lint, digest
- **`structurizr-renderer`** — SVG (own layout engine), PlantUML, Mermaid, DOT exporters
- **`structurizr-web`** — Axum server: workspace browser, live reload, JSON API
- **`structurizr-wasm`** — wasm-bindgen shim for in-browser rendering
- **`structurizr-cli`** — the `structurizrx` binary

`original-java/` vendors upstream Structurizr sources; its DSL test fixtures
serve as the compatibility corpus (67/84 upstream fixtures parse; the rest use
features that are out of scope — archetypes, workspace `extends`, URL includes).

## Status

Experimental and moving fast. The spec's phases 1–6 (model extensions, DSL
extensions, selector engine, generated views, LLM affordances, rendering) are
implemented. Notable gaps: deployment and dynamic views parse but are not yet
rendered by the exporters, and there is no model→DSL emitter yet.

## License

[Apache-2.0](LICENSE)
