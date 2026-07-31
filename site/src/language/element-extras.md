# Element extras

Any element body (`person`, `softwareSystem`, `container`, `component`) can
carry these, all optional:

```text
api = container "API" "Order API" "Rust" {
    status implemented                       // idea|draft|specified|implemented|deprecated
    introduced billingSplit                  // milestone name (see workspace-level blocks)
    retired target
    perspective "security" "rate-limited"    // quality-attribute note
    port rest "Customer REST API" {          // named interaction point
        protocol "HTTPS/JSON"
        direction in                         // in|out|inout (default)
    }
    technology "Rust"
    description "..."
    tags "Core"
    url "https://internal-wiki/order-api"
}
```

## `status` — confidence in the design

```text
billing = softwareSystem "Billing" { status idea }
```

Orthogonal to *time*: a `status idea` element introduced at a five-year
milestone is a vague long-term intention; a `status specified` one is a
committed roadmap item. Selectors filter on it (`element.status==idea`,
see [Views](./views.md)), and styles can theme it (e.g. dashed borders for
ideas).

## `introduced` / `retired` — lifecycle

Reference a [milestone](./workspace-blocks.md#milestones) name, never a raw
date — when a plan slips, the date changes in exactly one place and every
`introduced`/`retired` stays correct:

```text
legacyCrm = softwareSystem "Legacy CRM" { retired billingSplit }
```

An element exists at milestone M iff `introduced ≤ M < retired`. Unmarked
elements exist at all times. See [Views](./views.md) for `auto asof` and
`auto delta`, which render the model at or between milestones.

## `perspective` — quality-attribute notes

```text
perspective "performance" "p99 < 50ms, 2k rps"
```

A name plus a free-text note. Perspectives are declarable on elements,
relationships, and ports. `auto perspective "security"` (see
[Views](./views.md)) renders every item carrying a given perspective plus
enough structural context to make sense of it.

## `port` — named interaction points

Answers a question plain relationships can't: *what does this element offer
or require, independent of who's currently connected*, and *which of many
inbound arrows go through the same contract*.

```text
port events "Order events" {
    protocol "Kafka"
    direction out          // in|out|inout, default inout
    description "Public, versioned event stream"
}
```

Relationships attach to a port with dot syntax (`web -> api.rest "calls"`);
attaching to the element directly stays legal — that's the low-fidelity
form. Declared-but-never-connected ports are visible and lintable
(`auto lint` flags unbound ports) — an unconsumed interface is information,
not an error.

## `technology`, `description`, `tags`, `url`

Standard Structurizr fields, unchanged: `technology` (free text, also
settable as the trailing positional argument on the declaration line),
`description`, `tags` (comma-joined string, used by selectors and `styles`),
and `url` (a link shown in the web viewer).

Next: [Relationships and ports](./relationships.md) for the same set of
extras on relationships, plus `kind` and uncertain (`?`) relationships.
