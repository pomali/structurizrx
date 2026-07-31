# structurizrx render

Export diagrams from a workspace file to disk.

```sh
structurizrx render <file> [--format <fmt>] [--output <dir>]
```

| Flag | Default | Effect |
|---|---|---|
| `--format` | `plantuml` | One of `svg`, `mermaid`, `plantuml`, `dot`/`graphviz` |
| `--output`, `-o` | `.` | Output directory (created if missing) |

`render` first **materializes generated (`auto`) views** — the same step
`serve` performs — so `auto`, `auto focus`, `auto lint`, etc. in the
`views` block are expanded before export. Each diagram is written as
`<output>/<view-key>.<extension>`.

Not every exporter supports every view type; unsupported views are skipped
with a warning rather than silently vanishing:

| Format | Renders |
|---|---|
| `svg` | system landscape, system context, container, component |
| `mermaid` | system landscape, system context, container, component |
| `dot`/`graphviz` | system landscape, system context |
| `plantuml` (default) | system landscape, system context, container |

```sh
structurizrx render ws.dsl --format svg --output ./out
```
```text
Generated views: systemlandscape, systemcontext-shop, container-shop
Written: ./out/systemlandscape.svg
Written: ./out/systemcontext-shop.svg
Written: ./out/container-shop.svg
Warning: 1 view(s) skipped (svg exporter does not support: 1 dynamic)
```
