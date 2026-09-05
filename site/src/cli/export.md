# structurizrx export

Export a workspace to JSON.

```sh
structurizrx export <file> [--output <path>]
```

| Flag | Default | Effect |
|---|---|---|
| `--output`, `-o` | `workspace.json` | Output file path |

The output is StructurizrX's model JSON — a superset shape of the upstream
[Structurizr JSON schema](https://structurizr.com/json): every new field
(ports, `kind`, `status`, milestones, perspectives on relationships/ports) is
optional and omitted when unset, so tooling that only understands the
upstream schema still gets a workspace it can read; it just won't see the
extensions.

```sh
structurizrx export ws.dsl --output ws.json
```
```text
Exported workspace to ws.json
```

`export` does **not** materialize generated (`auto`) views the way
`render`/`serve`/`digest` do — it exports the workspace's model and views
exactly as authored (or as separately generated and re-imported).

## Static report

Export a portable, server-free single-page technical report with a top-level
index and all rendered SVG diagrams:

```sh
structurizrx export-site ws.dsl --output ./site
```

Open `site/index.html` in any browser. Generated views are materialized, just
as they are for `render` and `serve`.

## Static viewer

Export the interactive viewer as a portable application with the workspace JSON
and every required web asset (including images and JointJS):

```sh
structurizrx export-viewer ws.dsl --output ./viewer
```

Serve `viewer/` with any static file server and open `index.html`.
