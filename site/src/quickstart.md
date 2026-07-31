# Quickstart

## The fastest path: a sketch

A **sketch** is a file with no `workspace` block — just arrows. Unknown names
become placeholder software systems automatically; `?` marks a relationship
you're not sure about yet:

```text
customer -> shop "buys things"
shop -> billing "somehow charges" ?
billing -> erp
```

This is already a complete, valid model — it parses, validates, and renders
a single landscape view with everything in it:

![Rendered landscape view of the sketch above: customer, shop, billing, erp as placeholder software systems (dashed borders), with the "somehow charges" relationship also dashed to mark it uncertain](./images/sketch/sketch.svg)

Save it as `sketch.dsl` and view it live in the browser:

```sh
structurizrx serve sketch.dsl --open
```

Edit the file and save; the browser reloads automatically.

## A full workspace

A full workspace uses the Structurizr DSL (StructurizrX reads standard
upstream DSL) plus StructurizrX's own extensions for ports, relationship
kinds, status, milestones, and generated views:

```text
workspace "Shop" {
    model {
        customer = person "Customer"
        shop = softwareSystem "Shop" {
            web = container "Web App" "Storefront" "TypeScript"
            api = container "API" "Handles requests" "Rust" {
                status implemented
                port rest "Customer REST API" { protocol "HTTPS/JSON" }
            }
            db = container "Database" "Stores data" "PostgreSQL" { tags "Database" }
            web -> api.rest "calls"
            api -> db "reads and writes" { kind sync }
        }
        customer -> web "shops on"
    }
    views {
        auto                     // generated default view set
        auto focus api           // neighborhood view around one element
        auto lint                // placeholders, orphans, unbound ports
    }
}
```

```sh
structurizrx validate ws.dsl --strict    # errors + lint findings (add --json for tooling)
structurizrx render ws.dsl --format svg --output ./out
structurizrx serve ws.dsl --port 3000 --open
```

`render --format svg` produces one file per view. The zero-config `auto`
default alone gives a landscape view plus a context view per system and a
container view per non-empty system:

![System landscape view: Customer using Shop](./images/shop/auto-landscape.svg)

![Container view of Shop: Customer, Web App, API (with its Customer REST API port) and Database, showing the calls and reads/writes relationships](./images/shop/auto-container-shop.svg)

`auto focus api` adds a neighborhood view centered on a single element — here,
everything one hop from `api` in both directions:

![Focus view around API: Web App calling API, which reads and writes to Database](./images/shop/auto-focus-api.svg)

`validate` is **strict by default**: unknown identifiers and misplaced or
misspelled keywords fail with the offending file and line (include-aware),
the accepted keywords for that context, and a "did you mean" suggestion.
Forward references are legal; `!sketch` opts a full workspace into the same
leniency sketch files get.

## Where to go next

- The full [CLI reference](./cli/README.md) for every subcommand and flag.
- The [Language reference](./language/README.md) for the complete DSL —
  ports, relationship kinds, milestones, generated views, and everything
  else used above.
