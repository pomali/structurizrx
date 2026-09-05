# structurizrx serve

Serve a workspace, or a directory of workspaces, in a local web browser with
live reload.

```sh
structurizrx serve [path] [--port <n>] [--open]
```

| Argument/Flag | Default | Effect |
|---|---|---|
| `path` | `.` | A `.dsl`/`.json` file, or a directory containing one or more workspaces |
| `--port`, `-p` | `3000` | TCP port to listen on |
| `--open` | off | Open the browser automatically after starting |

Like `render`, `serve` materializes generated (`auto`) views before
rendering. Editing and saving a watched file reloads the browser via a
WebSocket automatically — no manual refresh.

## Routes

| Route | What it is |
|---|---|
| `GET /` | Workspace list |
| `GET /workspace/{name}` | Workspace overview |
| `GET /workspace/{name}/diagram/{key}` | A single diagram |
| `GET /workspace/{name}/graph` | Interactive relationship graph |
| `GET /workspace/{name}/decisions` | ADR list (from `!adrs`) |
| `GET /workspace/{name}/decisions/{id}` | A single ADR |
| `GET /workspace/{name}/graph` | Universe graph — the whole workspace as one force-directed graph |
| `GET /workspace/{name}/canvas` | In-browser WASM rendering demo |
| `GET /docs/` | This documentation site |
| `GET /llms.txt` | The DSL cheat sheet, as plain text |

## JSON API

The same server exposes a JSON API mirroring the CLI, useful for agents
working against a live server:

| Route | Equivalent to |
|---|---|
| `GET /api/workspaces` | workspace list with counts |
| `GET /api/workspace/{name}` | the full workspace JSON (`export`) |
| `GET /api/workspace/{name}/decisions[/{id}]` | ADR data |
| `GET /api/workspace/{name}/diagram/{key}/svg` | `render --format svg`, one diagram |
| `GET /api/workspace/{name}/diagram/{key}/mermaid` | `render --format mermaid`, one diagram |
| `GET /api/workspace/{name}/graph` | the universe graph as `{nodes, links}` JSON |
| `GET /api/workspace/{name}/digest` | [`digest`](./digest.md) |
| `GET /api/workspace/{name}/query?expr=...` | [`query`](./query.md) |

## Universe graph

`GET /workspace/{name}/graph` shows the entire workspace as one
force-directed graph, in the spirit of Obsidian's graph view: every element,
view, ADR and documentation section is a node, and every relationship,
containment, deployment instance and view membership is a link. It is the
one page that shows how everything in a workspace hangs together, rather
than one C4 scope at a time.

| Interaction | Effect |
|---|---|
| hover | Highlights the node and its immediate neighbours, dimming the rest |
| click | Opens the details panel (kind, description, technology, tags, connections) |
| double-click | Focuses the node's neighbourhood — a local graph, depth adjustable 1–5 |
| drag a node | Pins it where you drop it; click it without moving to release |
| drag / scroll | Pan and zoom; `f` zooms to fit, `Esc` clears focus and selection |
| `/` | Jumps to the search box; matches stay bright while everything else dims |

The **Nodes** and **Links** checkboxes filter what the graph contains.
Views, decisions and documentation sections start switched off — they belong
to the workspace, but they crowd out the model on first sight. The **Forces**
sliders (repel, link distance, link force, centre, node size) tune the
layout live.

Nodes are coloured by kind and sized by how many connections they have, so
the hubs of a model are visible before you read a single label.
