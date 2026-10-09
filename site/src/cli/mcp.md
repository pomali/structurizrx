# structurizrx mcp

Serve `structurizrx`'s read and edit commands as [Model Context
Protocol](https://modelcontextprotocol.io) tools over stdio, for agent hosts
(Claude Desktop, Claude Code, Cursor, Codex, …) that would otherwise have to
shell out to the CLI and parse its output.

```sh
structurizrx mcp
```

It's a hand-rolled JSON-RPC 2.0 server, newline-delimited JSON on
stdin/stdout: nothing but protocol messages goes to stdout, diagnostics go to
stderr. `initialize` and `tools/list` work like any MCP server:

```sh
printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"x","version":"0"}}}\n{"jsonrpc":"2.0","id":2,"method":"tools/list"}\n' | structurizrx mcp
```

## Tools

| Tool | Does |
|---|---|
| `validate` | Parse and validate a workspace; parse errors with file/line/column, model validation errors, lint findings. `strict` also treats lint findings as failing |
| `digest` | Compact plain-text model summary, sized for an LLM's context — use to get oriented before querying or editing |
| `query` | Run a selector expression and return matching elements/relationships as structured data |
| `locate` | Find where elements, ports, relationships, views or decisions are declared, by name path or viewer link — the bridge from `query`/`digest` results to an editable source line |
| `lint` | Model hygiene: blocking findings (the same gate `validate --strict` fails on) plus softer warnings |
| `diff` | Compare two versions of a workspace as models, keyed by name so renames are detected; git revisions by default, or `against` another file |
| `render` | Render a workspace's diagrams to SVG or PNG; all views by default, or one `view` |
| `docs` | The DSL cheat sheet — keywords, syntax, extensions — for writing or editing DSL from scratch |
| `add` | Add one DSL statement inside a block, keeping comments and formatting; writes nothing if the result wouldn't parse |
| `remove` | Delete an element/relationship/port's declaring statement; `cascade` also removes relationships that refer to it |
| `rename` | Rename a DSL identifier everywhere, across `!include`d files, without touching quoted element names |
| `format` | Canonical DSL text: JSON converted to DSL, a sketch promoted to a full workspace; read-only, reports what would be lost |

Every tool takes `file` (absolute, or resolved against the server's *working
directory* — see below). Run `tools/list` for the exact input schema of each,
including the free-text selector/reference grammar `query` and `locate` take.

## `file` is resolved against the server's cwd

`file` arguments are not resolved against the client's workspace or the
conversation's notion of "current file" — they're resolved against the
directory `structurizrx mcp` itself was launched from. An agent host that
runs the server from an unpredictable directory (or the user's home
directory) can silently miss the intended file. **Passing an absolute path
is always safest.**

## Configuration

### Claude Code

```sh
claude mcp add structurizrx -- structurizrx mcp
```

### Claude Desktop

Add to `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "structurizrx": {
      "command": "structurizrx",
      "args": ["mcp"]
    }
  }
}
```

### Cursor

Add to `.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "structurizrx": {
      "command": "structurizrx",
      "args": ["mcp"]
    }
  }
}
```
