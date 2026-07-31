# Core structure

A full workspace has three top-level blocks:

```text
workspace "Name" "Optional description" {
    model {
        // people, systems, containers, components, relationships
    }
    views {
        // hand-authored views, or `auto` generators
    }
}
```

## The element hierarchy

Standard C4 elements, unchanged from upstream Structurizr:

```text
model {
    customer = person "Customer" "A person who buys things"

    shop = softwareSystem "Shop" "Sells things online" {
        web = container "Web App" "Storefront" "TypeScript"
        api = container "API" "Handles requests" "Rust" {
            controller = component "OrderController" "Handles order requests"
        }
    }

    customer -> shop "Uses"
    web -> api "Calls"
}
```

- `person` — a human user, inside or outside the system landscape
- `softwareSystem` — the top-level unit of the C4 model
- `container` — inside a `softwareSystem`; an application, service, database,
  etc.
- `component` — inside a `container`; a logical grouping of code

Every element declaration follows `identifier = kind "Name" ["Description"]
["Technology"] { ... }` (the trailing positional arguments and the body are
both optional). The identifier is how everything else — relationships,
views, ports — refers to this element, and it's stable across refinement:
a placeholder can later grow into a full `softwareSystem` with containers
without any downstream reference changing.

## Groups

`group` clusters elements (for layout and for selectors like
`element.layer==<group>`) without changing the model hierarchy:

```text
softwareSystem "Shop" {
    group "Core" {
        web = container "Web App"
        api = container "API"
    }
    group "Data" {
        db = container "Database"
    }
}
```

## Enterprise and deployment nodes

`enterprise "Name" { ... }` scopes a block of `softwareSystem`/`person`
declarations as internal to the named enterprise (vs. external actors
declared outside it) — used by system landscape views to distinguish
"inside our organization" from third parties.

`deploymentEnvironment "Name" { deploymentNode "Name" { containerInstance
api; infrastructureNode "Load Balancer"; } }` describes what runs where, for
deployment views — unchanged from upstream Structurizr.

## Comments

`//` and `#` start a line comment, and `/* … */` spans multiple lines. Unlike
upstream Structurizr — where `//`/`#` are comments only when they are the
first non-whitespace on a line — StructurizrX also accepts them **inline**,
after other tokens:

```text
autolayout lr        # inline note, ignored to end of line
background #1168bd    // the hex color is kept; this trailing comment is not
```

To keep hex colors and variable interpolation unambiguous, an inline `#`
starts a comment only when it is followed by whitespace or the end of the
line. `#1168bd` (a color value) and `#{VAR}` (interpolation) are therefore
never mistaken for comments. An inline `//` always starts a comment; unquoted
urls such as `https://example.com/theme.json` keep their `//` because it is
part of the word, not a standalone token.

Next: [Sketch mode](./sketch-mode.md) — the zero-ceremony way to start a model
with no `workspace` block at all.
