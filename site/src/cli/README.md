# CLI reference

The `structurizrx` binary (package `structurizr-cli`) accepts both `.dsl` and
`.json` workspace files for every subcommand that takes a `file` argument.

| Command | What it does |
|---|---|
| [`validate`](./validate.md) | Parse + validate; `--strict` also fails on lint findings; `--json` emits structured output |
| [`lint`](./lint.md) | Model hygiene: blocking findings, warnings and dependency cycles |
| [`render`](./render.md) | Export diagrams (materializes generated views first) |
| [`serve`](./serve.md) | Live-reloading web viewer with a JSON API |
| [`digest`](./digest.md) | Compact plain-text model + view summary, sized for LLM context |
| [`query`](./query.md) | Run a selector expression against a workspace |
| [`locate`](./locate.md) | Find where elements, relationships and views are declared, by path or from a viewer link |
| [`diff`](./diff.md) | Compare two versions of a workspace as models |
| [`clusters`](./clusters.md) | Community/coupling analysis: conformance findings, cycles, bridges, metrics |
| [`graph`](./graph.md) | The whole workspace as one graph, as JSON or DOT |
| [`fmt`](./fmt.md) | Canonical DSL text: JSON→DSL, sketch→workspace, `--check`/`--write` |
| [`add`/`remove`/`rename`](./edit.md) | Surgical, re-parse-checked edits to DSL source |
| [`export`](./export.md) | Workspace JSON (superset of the Structurizr JSON schema) |
| `export-site` | Single-page SVG technical report |
| `export-viewer` | Portable interactive viewer with bundled assets and workspace JSON |
| [`mcp`](./mcp.md) | Serve these commands as MCP tools over stdio |
| [`docs`](./docs.md) | Print the DSL cheat sheet |

There's also `structurizrx lsp`, which runs the DSL language server over
stdio for editor integration (see the VS Code extension under
`editors/vscode` in the repository) — it has no user-facing flags and isn't
covered further here.

## Global behavior

- `--version` prints the binary version; `--help` (or a subcommand's
  `--help`) prints usage.
- File loading picks the DSL parser or the JSON deserializer based on the
  file extension (`.json` vs anything else, treated as DSL).
