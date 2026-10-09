# structurizrx validate

Parse and validate a `.dsl` or `.json` workspace file.

```sh
structurizrx validate <file> [--strict] [--json]
```

| Flag | Effect |
|---|---|
| `--strict` | Also fail (non-zero exit) on lint findings — placeholders, uncertain (`?`) items, orphan elements, unbound ports |
| `--json` | Emit machine-readable JSON instead of text: `{valid, errors: [{code, file, line, column, message}], lint: [{code, elementId, name, message}]}` |

Without `--json`, parse/validation errors and (with `--strict`) lint findings
print to stderr as `file:line:col: message`, one per error, and the process
exits non-zero on failure. On success it prints `✓ Workspace '<name>' is
valid`.

Errors are **strict by default** even without `--strict`: an unknown element
identifier, or a misplaced/misspelled keyword, is always a hard parse error —
with the offending file and line (include-aware across `!include`d files),
the accepted keywords for that context, and a "did you mean" suggestion.
`--strict` only adds the *lint* pass (things that parse fine but indicate an
unfinished model) to what fails the command.

Every parse error in the file is reported in one run, up to 20, instead of
stopping at the first one — useful when cleaning up a file with several
mistakes at once:

```sh
structurizrx validate broken.dsl
```
```text
broken.dsl:3:20: unknown keyword 'persn' in model; expected one of: person, softwareSystem, group, enterprise, deploymentEnvironment, element, properties, or a relationship 'a -> b'; did you mean 'person'?
broken.dsl:6:19: unknown keyword 'containr' in softwareSystem body; expected one of: container, group, description, technology, url, tags, properties, perspective, perspectives, port, status, introduced, retired; did you mean 'container'?
broken.dsl:8:9: unknown element identifier 'customer' in relationship
broken.dsl:9:19: unknown element identifier 'ap' in relationship
✗ 4 parse errors in broken.dsl
```

Each `--json` error carries a stable `code` — one of `unknown-identifier`,
`unknown-keyword`, `unexpected-token`, `unclosed-block`, `include`, `syntax`,
`io`, `load` — plus `file`, `line`, `column` and `message`:

```sh
structurizrx validate broken.dsl --json
```
```json
{
  "errors": [
    {
      "code": "unknown-keyword",
      "column": 20,
      "file": "broken.dsl",
      "line": 3,
      "message": "unknown keyword 'persn' in model; expected one of: person, softwareSystem, group, enterprise, deploymentEnvironment, element, properties, or a relationship 'a -> b'; did you mean 'person'?"
    },
    {
      "code": "unknown-identifier",
      "column": 9,
      "file": "broken.dsl",
      "line": 8,
      "message": "unknown element identifier 'customer' in relationship"
    }
  ],
  "lint": [],
  "valid": false
}
```

```sh
structurizrx validate ws.dsl --strict --json
```
```json
{
  "valid": true,
  "errors": [],
  "lint": [
    { "code": "unbound-port", "elementId": "api.rest", "name": "rest", "message": "port 'rest' is never connected" }
  ]
}
```
