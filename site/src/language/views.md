# Views

The master model is authored; views are declared as **selections** over it,
and the common ones need zero declaration at all — an empty or absent
`views` block gets the zero-config default set.

## The induced-subgraph rule

A view is fundamentally a *set of elements*, however that set is produced.
Given the set, every model relationship whose two endpoints are both in the
set is included automatically, and ancestors of included elements are pulled
in as boundary boxes for rendering context. This is why a hand-written
`include a b c` never has to enumerate relationships — the system derives
the edges.

## Selectors

One expression language, used by `include`/`exclude`, generator arguments,
and `structurizrx query`:

```text
element.tag==Database
element.kind==container
element.status==idea
element.layer==domain           // layer = group name, or the `layer` property
element.parent==shop            // direct children
element.technology==Kafka
element.property.owner==checkout-team
relationship.kind==async
relationship.tag==critical
->api->                         // neighborhood: api + direct neighbors
a && b, a || b, !a              // boolean combinators
```

Two properties get special tooling awareness beyond the generic
`element.property.<name>` lookup: `owner` (default `rollup` partition, a
digest column, and an optional unowned-element lint) and `layer` (layer
views, layer-order lint). They're still stored as ordinary properties.

Selectors *filter*; they can't compute anything requiring a graph walk —
that's what generators are for.

## The `auto` generator family

Each generator answers a specific stakeholder question. All of them can
appear any number of times in a `views` block, and generated views get
deterministic keys (`auto-focus-api`, `auto-context-shop`) so links and
stored layout survive regeneration.

```text
views {
    auto           // zoom ladder: landscape + context per system + container
                   // view per non-empty system + component view per
                   // non-empty container. Zero-config default when the
                   // `views` block is absent or just says `auto`.
}
```

**"What breaks if I change X? What does X need?"** — reachability:

```text
auto focus api {
    depth 2                // default 1; unset = 1
    direction in           // in = impact analysis (who depends on me)
                           // out = dependency analysis (what do I need)
                           // both (default when direction is omitted)
    splitBy kind           // one *separate view* per relationship kind present
}
```

Without `splitBy`, `focus` emits a single combined view.

**"How are X and Y connected at all?"** — path enumeration:

```text
auto paths web db          // all simple paths web → db
```

**"Where does concern C live?"** — cross-cutting slices:

```text
auto perspective "security"     // everything carrying that perspective
auto layer "domain"
auto slice relationship.kind==dataflow
auto slice element.status==idea
```

`layer` groups elements for this kind of slicing; see
[`layers.dsl`](https://github.com/pomali/structurizrx/blob/main/site/examples/layers.dsl)
for a worked example that groups a system into `"Core"` and `"Data"` and
generates one `auto layer` view per group.

**"What's unfinished or inconsistent?"** — model hygiene:

```text
auto lint       // placeholder elements, ?-marked items, unbound ports,
                // orphan elements
```

**"What changes between now and milestone M?"** — temporal, keyed off
[`milestones`](./workspace-blocks.md#milestones-named-points-in-time):

```text
auto asof billingSplit          // model state at that milestone;
                                // `asof now` filters out everything future
auto delta now billingSplit     // union of both states — a migration/diff view
```

> **Not yet materialized:** `auto rollup` (the partition/Conway-view
> generator described in `docs/SPEC.md` §6.3) parses successfully but
> currently emits nothing — `generate_views` accepts the syntax and prints a
> note that it was skipped, rather than producing a view. Likewise the
> `collapse` modifier for folding n-ary connectors isn't implemented yet.
> Check the repository README's Status section for the current gap list
> before relying on either.

## Dynamic and deployment views

Dynamic views (ordered interaction scenarios) and deployment views exist and
parse using standard upstream Structurizr syntax, unchanged.

A dynamic view numbers a sequence of `source -> destination "description"`
steps in the order they're declared, for answering "what happens, in what
order, when this use case runs":

```text
dynamic shop "checkout" "Placing an order" {
    customer -> web "Clicks 'Buy now'"
    web -> api "POST /orders"
    api -> db "Inserts order row"
    autoLayout
}
```

A deployment view selects from a `deploymentEnvironment` — the
infrastructure a system runs on (`deploymentNode`, `containerInstance`,
`infrastructureNode`) — rather than from the model's structure:

```text
production = deploymentEnvironment "Production" {
    deploymentNode "Amazon Web Services" {
        deploymentNode "EC2 instance" {
            apiInstance = containerInstance api
        }
    }
}
views {
    deployment shop "Production" { include * }
}
```

Full runnable examples:
[`checkout-flow.dsl`](https://github.com/pomali/structurizrx/blob/main/site/examples/checkout-flow.dsl)
and
[`deployment.dsl`](https://github.com/pomali/structurizrx/blob/main/site/examples/deployment.dsl).

> **Not yet rendered:** both view types validate and digest correctly, but
> none of the four exporters (SVG, Mermaid, PlantUML, DOT) draws them yet —
> `render` silently skips them with a warning today. The container view
> below is the same underlying model, shown via a view type that *does*
> render, as a stand-in until dynamic/deployment rendering lands.

![Container view of Shop: Web App calling API, which reads and writes to Database](./images/checkout-flow/auto-container-shop.svg)

Next: [Documentation and decisions](./documentation.md) for `!adrs`/`!include`.
