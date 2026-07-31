# Sketch mode

A file with **no `workspace` block** is a sketch: bare statements are
implicitly wrapped in `workspace { model { ... } }`, and any identifier that's
used but never declared is auto-created as a placeholder software system
(tagged `Placeholder`).

```text
customer -> shop "buys things"
shop -> billing "somehow charges" ?
billing -> erp
```

This is a complete, valid model — it parses, validates, and renders a single
landscape view with everything in it. Placeholders participate in all
standard views like any other software system; there's nothing special you
need to do to "finish" one later — just declare it properly
(`shop = softwareSystem "Shop" { ... }`) under the same identifier and it
picks up wherever the placeholder left off.

## `?` — marking uncertainty

A trailing `?` on a relationship marks it explicitly *uncertain*, distinct
from merely undetailed:

```text
shop -> billing "somehow charges" ?
```

`?` is meaningful, not just decorative: `structurizrx validate --strict` and
`auto lint` both surface `?`-marked items as findings, so an agent (or a
human) can grep a large model for "things I said I wasn't sure about."

## Opting a full workspace into sketch leniency

Inside a full `workspace { ... }` block, strictness is the default: an
undeclared identifier is a parse error, same as any other unknown-keyword
error. Add `!sketch` at the top of the file to opt that workspace into the
same auto-vivification bare sketch files get:

```text
!sketch
workspace "Shop" {
    model {
        customer -> shop "buys things"   // shop auto-created, no error
    }
}
```

Dotted identifiers (port references like `api.rest`) must always resolve to
a *declared* element, even in sketch mode — auto-vivification only applies to
bare identifiers.

Next: [Element extras](./element-extras.md) — status, lifecycle, ports, and
the other optional annotations available on any element.
