# structurizrx fmt

Print a workspace as canonical DSL: stable ordering and layout, a `.json`
workspace converted to DSL, a [sketch](../language/sketch-mode.md) promoted
to a full `workspace { model { … } views { … } }`.

```sh
structurizrx fmt <file> [--output <path>] [--write] [--check] [--force]
```

| Flag | Effect |
|---|---|
| `--output`, `-o` | Write the result to this file instead of stdout |
| `--write` | Rewrite `<file>` in place |
| `--check` | Exit 1 if the file isn't already in canonical form (CI); no output written |
| `--force` | With `--write`, overwrite even when comments or other things would be lost |

`--write` and `--check` are mutually exclusive; `--force` requires `--write`.

## Promoting a sketch

A sketch (bare arrows, no `workspace` block) formats into a full workspace,
with placeholders declared as `softwareSystem`s and a generated
`systemLandscape` view:

```sh
structurizrx fmt sketch.dsl
```
Input (`site/examples/sketch.dsl`):
```text
customer -> shop "buys things"
shop -> billing "somehow charges" ?
billing -> erp
```
Output:
```text
workspace "Sketch" {

    model {
        customer = softwareSystem "customer" "" "Placeholder"
        shop = softwareSystem "shop" "" "Placeholder"
        billing = softwareSystem "billing" "" "Placeholder"
        erp = softwareSystem "erp" "" "Placeholder"
        customer -> shop "buys things"
        shop -> billing "somehow charges" ?
        billing -> erp
    }

    views {
        systemLandscape "sketch" {
            include billing customer erp shop
        }
    }
}
```

## Converting JSON to DSL

`fmt` accepts `.json` workspaces too — useful for round-tripping a workspace
exported by `export`, or one produced by other Structurizr tooling, back
into readable DSL:

```sh
structurizrx export shop.dsl --output shop.json
structurizrx fmt shop.json
```
```text
workspace "Shop" {

    model {
        customer = person "Customer"
        shop = softwareSystem "Shop" {
            webApp = container "Web App" "Storefront" "TypeScript"
            api = container "API" "Handles requests" "Rust" {
                status implemented
                port customerRestApi "Customer REST API" {
                    protocol "HTTPS/JSON"
                }
            }
            database = container "Database" "Stores data" "PostgreSQL" "Database"
            webApp -> api.customerRestApi "calls"
            api -> database "reads and writes" {
                kind sync
            }
        }
        customer -> webApp "shops on"
    }
    ...
}
```

## `--write` and what it refuses to lose

`fmt` doesn't preserve comments, so `--write` (and `--check`, implicitly)
refuses to overwrite a file that has any, or that uses `!include`,
`!docs`/`!adrs`, constants (`!const`/`!var`), or `specification` kind
aliases — anything the canonical rewrite would drop or normalize away:

```sh
structurizrx fmt commented.dsl --write
```
```text
Error: not rewriting commented.dsl because comments are not preserved.
Use --output to write elsewhere, or --force to accept the loss.
```

```sh
structurizrx fmt commented.dsl --write --force
```
```text
formatted commented.dsl
```

## CI: enforce canonical form

```sh
structurizrx fmt ws.dsl --check
```
```text
ws.dsl is not in canonical form (run `structurizrx fmt --write`)
```
Exits 0 silently when the file is already canonical.
