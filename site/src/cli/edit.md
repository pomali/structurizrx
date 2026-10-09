# structurizrx add / remove / rename

Surgical edits to DSL source — the three edits that are easy to get wrong by
hand: putting a statement inside the right block, deleting a declaration
together with what refers to it, and renaming an identifier everywhere,
including across `!include`d files. Comments and formatting outside the
touched lines are kept.

Every edit is written, then the workspace is re-parsed; if the result
doesn't parse, the original files are restored and the parse errors are
reported instead — a workspace never goes from valid to broken through these
commands.

Targets are addressed by DSL **identifier** (`api`) or by **name path**
(`Shop/API`, `Shop/Web App->Shop/API`), same as [`locate`](./locate.md).

## add

```sh
structurizrx add <file> <statement> [--in <ref>] [--json]
```

| Flag | Default | Effect |
|---|---|---|
| `--in` | `model` | Block to add into: `model`, `views`, or an element reference |
| `--json` | | Emit `{added: {file, line}}` instead of text |

`statement` is one DSL line — an element, a relationship, a view, or a body
line like `technology "Rust"` inside an existing element.

```sh
structurizrx add ws.dsl 'db = container "Database" "Stores orders" "Postgres"' --in shop
```
```text
added at ws.dsl:6
```

```sh
structurizrx add ws.dsl 'api -> db "reads and writes"' --in model
```
```text
added at ws.dsl:10
```

```sh
structurizrx add ws.dsl 'cache = container "Cache" "Redis"' --in shop --json
```
```json
{"added":{"file":"ws.dsl","line":7}}
```

## remove

```sh
structurizrx remove <file> <reference> [--cascade] [--json]
```

| Flag | Effect |
|---|---|
| `--cascade` | Also remove every relationship touching the element or anything inside it |
| `--json` | Emit `{removed: [{what, file, line, endLine}, ...]}` instead of text |

Without `--cascade`, removing something still referenced by a relationship
fails and nothing is written:

```sh
structurizrx remove ws.dsl gateway
```
```text
Error: the edit would leave the workspace unparseable, so nothing was changed:
  ws.dsl:8:19: unknown element identifier 'gateway' in relationship
(use --cascade to also remove the relationships that refer to it)
```

With `--cascade`, the element's own relationships are removed alongside it,
across whichever file each one is declared in:

```sh
structurizrx remove ws.dsl gateway --cascade
```
```text
removed container gateway (parts/extra.dsl:2)
removed relationship Shop/Web App -> Shop/API "calls" (ws.dsl:8)
```

```sh
structurizrx remove ws.dsl cache --json
```
```json
{"removed":[{"endLine":7,"file":"ws.dsl","line":7,"what":"container cache"}]}
```

## rename

```sh
structurizrx rename <file> <identifier> <new-identifier> [--json]
```

Rewrites the identifier everywhere it's used as a token — declarations,
relationship endpoints, port references (`api.rest`) — across every
`!include`d file. **Quoted element names are untouched**: renaming the
identifier `api` to `gateway` doesn't change `container "API"` to
`container "Gateway"`.

```sh
structurizrx rename ws.dsl api gateway
```
```text
renamed api -> gateway (2 occurrences in 2 files)
  ws.dsl: 1
  parts/extra.dsl: 1
```

```sh
structurizrx rename ws.dsl db database --json
```
```json
{"renamed":{"files":[{"file":"ws.dsl","replacements":2}],"from":"db","to":"database"}}
```

## Multi-file example

A two-file workspace, entry file plus an `!include`d part:

`ws.dsl`:
```text
workspace "Shop" {
    model {
        customer = person "Customer"
        shop = softwareSystem "Shop" {
            !include parts/extra.dsl
        }
        customer -> webapp "shops on"
        webapp -> api "calls"
    }
    views {
        systemContext shop "context" {
            include *
            autoLayout
        }
    }
}
```

`parts/extra.dsl`:
```text
webapp = container "Web App" "Storefront" "TypeScript"
api = container "API" "Handles orders" "Rust"
```

`structurizrx rename ws.dsl api gateway` rewrites the identifier in both
files — the declaration in `parts/extra.dsl` and the relationship endpoint in
`ws.dsl` — while leaving `container "API"`'s quoted name exactly as it was:

```text
webapp = container "Web App" "Storefront" "TypeScript"
gateway = container "API" "Handles orders" "Rust"
```
