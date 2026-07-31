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
| `GET /workspace/{name}/decisions` | ADR list (from `!adrs`) |
| `GET /workspace/{name}/decisions/{id}` | A single ADR |
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
| `GET /api/workspace/{name}/digest` | [`digest`](./digest.md) |
| `GET /api/workspace/{name}/query?expr=...` | [`query`](./query.md) |
