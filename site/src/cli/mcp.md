# structurizrx mcp

Run a first-class MCP server over stdio so LLM agents can call StructurizrX
tools directly instead of shelling out.

```sh
structurizrx mcp [path]
```

| Argument | Default | Effect |
|---|---|---|
| `path` | `.` | Base directory used to resolve relative paths from tool arguments |

The server speaks JSON-RPC on stdio (Content-Length framed), implements
`initialize`, `tools/list`, and `tools/call`, and exposes these tools:

- `workspace.list`
- `workspace.validate`
- `workspace.digest` (`size=small|medium|full`, optional `selector` focus)
- `workspace.query`
- `workspace.render`
- `patch.preview` (transaction-only, no write)
- `patch.apply` (write gated by transaction + file-integrity check)

All tool results include structured JSON for machine use (`structuredContent`)
plus text content for human logs. Error results include stable codes, affected
paths, and actionable `nextSteps` for repair loops.
