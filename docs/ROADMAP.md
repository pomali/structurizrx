# StructurizrX — assessment and roadmap

Written 2026-09-19 against v0.2.0 (commit 46a5daa), after reading the docs,
spec and skills, running the test suite (301 tests pass), exercising
`validate` / `digest` / `query` / `render` on `site/examples/shop.dsl` and
`rust/examples/big-bank-plc`, and screenshotting the viewer pages.

## Verdict

The foundation is strong: a strict parser with good error text, a real query
engine, deterministic generated views, and a web viewer that is more capable
than upstream Structurizr's own (review, clusters, diff, universe graph). The
weak spots are consistency between the surfaces (CLI vs viewer, digest vs
render) and the gap between "agents can read the model" and "agents can safely
change it".

## Defects found while trying it

- **`validate` reports only the first parse error.** A file with two mistakes
  needs two round trips. The `--json` output has no `file` / `line` / `column`
  fields, only a message string, and every parse error carries the code
  `parse`. Spec §9.3 ("machine-fixable errors") is not quite met. Error
  recovery at statement boundaries plus structured locations is the single
  biggest win for agent convergence.
- **`query` prints relationships as bare ids** (`relationship 7`), in both text
  and JSON. It should print the same `A -> B "desc" [kind]` triples the digest
  uses.
- **`digest` view counts ignore implied relationships.** The shop landscape
  digests as `2 elements, 0 rels`, yet the render shows the lifted "shops on"
  edge. The skill tells agents to trust the digest to confirm intent, so it
  must agree with the render.
- **Server-side SVG has label collisions.** In the Big Bank container view,
  edge labels sit on top of the Mobile App box and the SPA subtitle, and the
  port label overlaps the arrowhead. The client (dagre) render is fine, so
  `render`, `export-site` and the README pictures show a worse diagram than
  `serve` does. Either fix label placement in the Sugiyama layout or make the
  CLI use the same layout as the viewer.
- **`render` and `serve` disagree about which views exist.** Deployment and
  dynamic views show in the viewer but are skipped by the exporters (known
  gap).
- **Polish:** review, clusters and diff pages are light-themed while the
  workspace page is dark. Every workspace's sidebar carries a "Canvas demo"
  entry. Page titles say "Structurizr". `auto lint` yields an empty
  `auto-lint` view even when the model is clean. `.claude-plugin/plugin.json`
  says 0.1.1 while the CLI is 0.2.0. GitHub issues are disabled, so users have
  nowhere to report any of this.

## Directions, ranked by value over effort

1. **CLI parity with the viewer.** `diff`, `review`, `clusters` and `graph`
   exist only behind HTTP. `structurizrx diff ws.dsl --from HEAD~1` printing
   model changes is a ready-made CI comment and an agent's "what did I just
   change" check. `structurizrx lint` could absorb the review checks and the
   cycle detection the clusters page already computes.
2. **A model-to-DSL emitter, then edit commands.** There is no formatter and
   no way to write the model back. Everything downstream is blocked on this:
   `fmt`, promoting a sketch to a workspace, LSP rename across includes and
   dotted references, and agent-safe edits such as
   `structurizrx add relationship a b "desc"` or `rename api gateway`. Agents
   today edit with text replacement and rely on validate to catch the damage.
3. **An MCP server.** The `serve` JSON API and the skill already define the
   surface. Wrapping validate, digest, query, locate, diff and render-to-PNG
   as MCP tools makes the tool usable from Cursor, Codex, Copilot and Claude
   Desktop, not just Claude Code.
4. **Interactive "views as queries" in the viewer.** The differentiator, and
   the browser already has everything needed: a selector box on the workspace
   page that renders the selection live, with a "copy as `auto slice …`"
   button. It also teaches the selector language by play.
5. **Grow lint into architecture rules.** Four lint codes today. `layer` is
   already a blessed property, so a layering rule (`ui` may call `domain`,
   never `data`), a no-cycles rule, a required-owner rule, and
   cross-system container relationships lacking a system-level relationship
   are all cheap and are what teams actually gate on.
6. **VS Code live preview.** A diagram panel next to the file is the expected
   feature of a DSL extension. `structurizr-wasm` already renders in the
   browser, so a webview preview is mostly wiring.
7. **Close the eval loop.** `structurizr-evals` grades but never runs an
   agent. A runner that invokes an agent per task and records retries-to-clean
   turns "LLM-native" from a claim into a number and shows which error
   messages to fix next.
8. **Onboarding gaps.** `init` with a template, shell completions,
   `render --format png` in the CLI (the PNG code lives only in the WASM
   crate), and a short "why this over Structurizr, LikeC4, Mermaid" docs page.
9. **Upstream reach.** Workspace `extends` is common in real Structurizr
   estates and is one of the unsupported fixture features (67/84 parse).
   Supporting it opens existing workspaces as a migration path.

## Status (2026-09-19)

Done (committed 2026-10-09):

- **All four CLI defects.** `validate` recovers at statement boundaries and
  reports up to 20 errors per run with structured `code`/`file`/`line`/
  `column`. `query` prints name-path relationship triples. Generated views
  include implied relationships, so digest counts match the render (and the
  auto context/container views now pull in neighbours connected only through
  children). SVG edge and port labels avoid element boxes, arrowheads and
  each other.
- **1. CLI parity:** `diff`, `lint`, `clusters`, `graph`.
- **2. Emitter and edits:** `structurizr_dsl::emit` (round-trip tested on
  every example), `fmt`, and textual `add` / `remove` / `rename` that re-parse
  and roll back on failure.
- **3. MCP server:** `structurizrx mcp` with twelve tools, edits included.
- Also: `render --format png` (it rendered no text until system fonts were
  loaded), the site CLI reference for every new command.

Found along the way, not fixed:

- `?` on a relationship tags it `Uncertain`, but lint's `uncertain` check
  only looks at elements, so sketches never report it despite the docs.
  Fixing it changes what `validate --strict` fails on.
- Unknown keywords inside a view body (`inklude *`) are ignored silently,
  unlike every other block.
- The LSP rename still works on single tokens in one file; it could now use
  the CLI's rename logic.

## Suggested order

Fix the four CLI defects first (multi-error validate with structured
locations, relationship triples in `query`, implied relationships in digest
counts, SVG label placement). They are small, on the path every agent walks,
and already promised by the spec. Then build the emitter, because items 2, 6
and LSP rename all depend on it.
