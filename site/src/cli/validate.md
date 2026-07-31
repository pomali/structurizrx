# structurizrx validate

Parse and validate a `.dsl` or `.json` workspace file.

```sh
structurizrx validate <file> [--strict] [--json]
```

| Flag | Effect |
|---|---|
| `--strict` | Also fail (non-zero exit) on lint findings — placeholders, uncertain (`?`) items, orphan elements, unbound ports |
| `--json` | Emit machine-readable JSON instead of text: `{valid, errors: [{code, message}], lint: [{code, elementId, name, message}]}` |

Without `--json`, parse/validation errors and (with `--strict`) lint findings
print to stderr with stable error codes, and the process exits non-zero on
failure. On success it prints `✓ Workspace '<name>' is valid`.

Errors are **strict by default** even without `--strict`: an unknown element
identifier, or a misplaced/misspelled keyword, is always a hard parse error —
with the offending file and line (include-aware across `!include`d files),
the accepted keywords for that context, and a "did you mean" suggestion.
`--strict` only adds the *lint* pass (things that parse fine but indicate an
unfinished model) to what fails the command.

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
