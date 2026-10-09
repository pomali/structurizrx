# structurizrx query

Run a [selector expression](../language/views.md#selectors) against a
workspace and print the matching elements/relationships.

```sh
structurizrx query <file> <expression> [--json]
```

| Flag | Effect |
|---|---|
| `--json` | Emit `{elements: [{id, name, path}], relationships: [{id, source, destination, description?, technology?, kind?, status?, introduced?, retired?}]}` instead of a text listing |

`expression` is parsed with `allow_hyphen_values`, so expressions containing
`-` (like `->api->`) don't need extra escaping.

Elements print with their name path; relationships print as `A -> B "desc"
[kind]` name-path triples, `[kind]` only shown when the relationship sets one:

```sh
structurizrx query ws.dsl "->api->"
```
```text
element  3  container "Web App"  Shop/Web App
element  4  container "API"  Shop/API
element  6  container "Database"  Shop/Database
relationship  7  Shop/Web App -> Shop/API.Customer REST API "calls"
relationship  8  Shop/API -> Shop/Database "reads and writes" [sync]
```

```sh
structurizrx query ws.dsl "->api->" --json
```
```json
{
  "elements": [
    { "id": "3", "name": "container \"Web App\"", "path": "Shop/Web App" },
    { "id": "4", "name": "container \"API\"", "path": "Shop/API" },
    { "id": "6", "name": "container \"Database\"", "path": "Shop/Database" }
  ],
  "relationships": [
    { "id": "7", "source": "Shop/Web App", "destination": "Shop/API.Customer REST API", "description": "calls" },
    { "id": "8", "source": "Shop/API", "destination": "Shop/Database", "description": "reads and writes", "kind": "sync" }
  ]
}
```

A bad expression exits non-zero with the engine's error text, which names
the valid selector paths — the same feedback loop `validate` gives for parse
errors. See the [language reference](../language/views.md) for the full
selector grammar (`element.status==idea`, `relationship.kind==async`,
`a && b`, `!a`, neighborhood syntax `->x->`, and so on).
