# Relationships and ports

A plain relationship is unchanged from upstream: `source -> destination
"description" "technology"`. StructurizrX adds `kind`, ports, naming, and
uncertainty.

```text
web -> api.rest "calls"                    // attach to a port via dot syntax
orderFlow = api -> billing "OrderPlaced" {  // named; body is optional
    kind async        // sync|async|publish|subscribe|dataflow|dependency|deploy
    status specified
    introduced billingSplit
    perspective "reliability" "at-least-once"
    tags "Critical"
    technology "Kafka"
    properties { owner "team-x" }
}
a -> b "maybe" ?                            // uncertain relationship
```

## Port-attached relationships

`source.port -> destination` or `source -> destination.port` connects to a
[named port](./element-extras.md#port--named-interaction-points) instead of
the element as a whole:

```text
web -> api.rest "calls"
api.events -> billing.orders "OrderPlaced" { kind async }
```

Attaching directly to the element (no `.port`) stays legal — that's the
lower-fidelity form, and both can coexist in the same model.

## `kind` — connector semantics

A **closed** vocabulary, richer than free-text `technology` plus a binary
interaction style:

| Kind | Use for |
|---|---|
| `sync` | Request/response, blocking call |
| `async` | Fire-and-forget, non-blocking call |
| `publish` | Emits to a topic/queue |
| `subscribe` | Consumes from a topic/queue |
| `dataflow` | Data movement without request/response semantics |
| `dependency` | Build-time dependency, not a runtime call |
| `deploy` | Deployment relationship |

`dependency` matters beyond labeling: it lets one master model serve both a
runtime view and a build-time/dependency view — filter views by `kind`
instead of maintaining two separate models. Selectors
(`relationship.kind==async`) and `auto focus ... { splitBy kind }` both key
off it.

## Naming a relationship

`orderFlow = api -> billing "OrderPlaced" { ... }` gives the relationship an
identifier, so other constructs — dynamic views, docs, perspectives — can
reference it directly instead of by description text matching.

## `?` — uncertain relationships

A trailing `?` marks a relationship as explicitly uncertain (see
[Sketch mode](./sketch-mode.md#--marking-uncertainty)) — surfaced by
`auto lint` and `validate --strict`, kept distinct from a relationship that's
simply undetailed.

Next: [Workspace-level blocks](./workspace-blocks.md) — `milestones`,
`perspectives`, `specification`, and `styles`.
