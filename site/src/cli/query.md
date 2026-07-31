# structurizrx query

Run a [selector expression](../language/views.md#selectors) against a
workspace and print the matching elements/relationships.

```sh
structurizrx query <file> <expression> [--json]
```

| Flag | Effect |
|---|---|
| `--json` | Emit `{elements: [{id, name}], relationships: [id]}` instead of a text listing |

`expression` is parsed with `allow_hyphen_values`, so expressions containing
`-` (like `->api->`) don't need extra escaping.

```sh
structurizrx query ws.dsl "element.tag==Database"
```
```text
element  6  container "Database"
```

```sh
structurizrx query ws.dsl "->api->" --json
```
```json
{
  "elements": [
    { "id": "3", "name": "container \"Web App\"" },
    { "id": "4", "name": "container \"API\"" },
    { "id": "6", "name": "container \"Database\"" }
  ],
  "relationships": ["7", "8"]
}
```

A bad expression exits non-zero with the engine's error text, which names
the valid selector paths — the same feedback loop `validate` gives for parse
errors. See the [language reference](../language/views.md) for the full
selector grammar (`element.status==idea`, `relationship.kind==async`,
`a && b`, `!a`, neighborhood syntax `->x->`, and so on).
