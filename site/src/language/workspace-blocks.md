# Workspace-level blocks

These sit directly inside `workspace { ... }`, alongside `model` and `views`.

## `milestones` — named points in time

```text
milestones {
    mvp          "2026-08"
    billingSplit "2026-12"  "Billing extracted from the monolith"
    target       "2031"     "Target architecture"
}
```

Ordered by declaration (not by date); dates are optional labels. An implicit
`now` milestone precedes all declared ones. Elements and relationships
reference milestones by name via `introduced`/`retired`
(see [Element extras](./element-extras.md#introduced--retired--lifecycle))
— never a raw date, so a slipped plan only needs updating in one place.
[Views](./views.md) can render `auto asof <milestone>` or
`auto delta <m1> <m2>`.

## `perspectives` — the quality-attribute registry

```text
perspectives {
    security "STRIDE-reviewed boundaries"
    performance
}
```

An optional registry of perspective names (a description is optional). This
lets `auto perspective *` enumerate every registered perspective and lets
validation catch typos in `perspective "..."` annotations elsewhere in the
model — registration itself is optional; unregistered perspective names still
work, just without typo-checking.

## `specification` — kind aliases

Domain vocabulary without a new element type in the model. A `specification`
block maps an alias to a base C4 kind plus default tags/technology:

```text
specification {
    kind queue container { tags "Queue,Connector" technology "Kafka" }
    kind lambda container { tags "Serverless" technology "AWS Lambda" }
}
model {
    shop = softwareSystem "Shop" {
        orders = queue "Order queue"    // stored as a plain container
    }                                   // tagged Queue,Connector
}
```

`orders` is a completely ordinary `container` underneath — renderers and
upstream-compatible JSON see nothing new — but selectors can match the alias
directly (`element.kind==queue`). Container/component-level aliases (like
`queue` above, aliasing `container`) can only be used **nested inside a
`softwareSystem`/`container` body**, matching where a plain `container`
/`component` declaration would go; `person`/`softwareSystem`-level aliases are
used at the top of `model`, same as a plain `person`/`softwareSystem` would
be.

## `styles` (inside `views`)

Tag-based visual overrides, unchanged from upstream Structurizr — every
exporter (SVG, PlantUML, Mermaid, DOT) respects these:

```text
views {
    auto
    styles {
        element "Queue" {
            background "#ff0000"
            shape hexagon
        }
        relationship "Critical" {
            thickness 4
            color "#ff0000"
            dashed true
        }
    }
}
```

`element "<tag>" { ... }` accepts `shape`, `background`, `color`/`colour`,
`stroke`, `fontSize`, `border`, `opacity`, `width`, `height`.
`relationship "<tag>" { ... }` accepts `thickness`, `color`/`colour`,
`fontSize`, `lineStyle`, `routing`, `opacity`, `dashed`, `position`. Both key
off the element/relationship's `tags` — any element or relationship carrying
the named tag picks up the style.

Next: [Views](./views.md) — hand-authored views, selectors, and the `auto`
generator family.
