# VS Code extension — feature plan

Where the extension stands today: syntax highlighting (TextMate) plus five
language-server features — diagnostics, hover, completion, go-to-definition and
document symbols. The server runs either as the `structurizrx` binary over
stdio or as the bundled WASM build in the extension host.

Features below are grouped by tier. Tier 2 is implemented; the rest are
proposals.

## Tier 1 — diagram preview (not started)

The biggest gap: `structurizr-wasm` already renders SVG/PNG and is proven in
the web viewer, but the extension cannot draw a diagram.

1. **Live preview webview** — `Structurizr: Open Preview (to the Side)`,
   re-rendering on a debounce as you type. Needs a second `wasm-pack` build of
   `structurizr-wasm` (`--target nodejs` alongside the LSP one, or `--target
   web` to render inside the webview).
2. **View picker + preview↔source sync** — view-key dropdown in the preview
   toolbar; clicking an element reveals its declaration (`AnalyzedDocument`
   already carries `id_to_pos`), and moving the cursor highlights the matching
   element in the preview.
3. **Export commands** — `Export Diagram as SVG/PNG/PlantUML/Mermaid/DOT`
   straight through to the existing exporters. PNG needs the `png` feature that
   `structurizr-wasm` already enables.

## Tier 2 — LSP features over the existing index (implemented)

These all reuse `DocumentState`'s `declarations` / `references` maps and the
`IdentifierRegister`, so they are cheap relative to their value.

4. **Rename symbol** — `textDocument/rename` plus `prepareRename`. Same lookup
   as go-to-definition, applied to every occurrence of the identifier.
5. **Find all references / document highlight** — `textDocument/references` and
   `textDocument/documentHighlight` over the same index. Makes
   go-to-definition feel finished.
6. **Semantic tokens** — `textDocument/semanticTokens/full`, driven by the DSL
   lexer, fixing the places where the TextMate grammar guesses wrong
   (`!include`, expressions, multi-line descriptions).
7. **Context-aware completion** — `Core::completion` used to ignore the
   position entirely and return every keyword in every keyword set plus every
   identifier. Now scoped to the enclosing block, with element identifiers
   after `->`.

## Tier 3 — workflow integration (not started)

8. **Serve integration** — `Structurizr: Start Server` runs `structurizrx serve
   --port` as a task and opens the live-reload viewer in a Simple Browser tab.
   Gate on the binary runtime being available.
9. **Snippets** — `workspace` / `model` / `views` scaffolds, `systemContext`,
   `container`, deployment blocks. `contributes.snippets` needs no code.
10. **Validate on save with `--strict`** — surfaced through a task and problem
    matcher.
11. **Query panel** — expose `structurizr-query`'s selector engine as a
    command: type a selector, get matching elements listed and highlighted.
    Nothing else in the ecosystem has this.
12. **Formatter** — `documentFormattingProvider` for consistent indentation.
    The most expensive item here: it needs a pretty-printer in
    `structurizr-dsl`, which does not exist yet.

## Tier 4 — reach (not started)

13. **Web extension target** — the WASM runtime is Node-host only today. A
    `--target web` build plus a `browser` entry point would make the extension
    work on vscode.dev and github.dev with nothing installed.
14. **`!include` awareness** — multi-file go-to-definition and correct
    diagnostic line numbers across includes. Blocks any real multi-file
    workspace.

## Suggested order

Tier 2 first (done — one index, four features). Then 1 → 3 → 8, since the
preview is the reason to install the extension and the exports make it useful.
Tier 4 last: both items are structural.
