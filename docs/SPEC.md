# StructurizrX Architecture Description System — Draft Spec

Status: **implemented**, revision 3 (2026-07-04; status updated 2026-07-20).
All six phases of §11 are implemented and on `main` (one commit per phase; see
git history), followed by an agent-experience hardening pass (strict-by-default
parsing with did-you-mean errors, structured lint, `validate --json`,
include-aware error locations). Open questions are resolved (§0, §12); new
questions raised during evolution get added to §12 before being settled.

## 0. Decisions log

Settled in discussion on 2026-07-04:

- **Upstream Structurizr interop is not a requirement.** We keep the ability to
  *read* upstream DSL (cheap — parser exists, fixture tests stay), but our
  extensions are designed for clarity, not upstream validity. A lossy
  `--compat structurizr` export may exist as nice-to-have.
- **Relationship `kind` is a closed vocabulary** to start.
- **Strict-by-default** for full workspaces; `!sketch` is opt-in.
- **Enterprise scope**: groups + custom elements are sufficient for now; no
  capability/value-stream constructs.
- **Connector folding** (§5.3) stands as proposed (no strong preference voiced).
- Books (e.g. *Just Enough Software Architecture*) are treated as inspiration,
  not standards. Every construct below is justified by the question it lets a
  reader answer, not by citation. Ports/connectors specifically are common
  practice across the component-and-connector literature and ADLs
  (ISO/IEC/IEEE 42010, *Documenting Software Architectures*, ACME/AADL), so
  they are well-trodden — but they earn their place here on utility.
- **Mermaid and PlantUML are export targets, not the authoring format**;
  LikeC4's predicate-view semantics are adopted (§6.1); authoring stays a
  Structurizr-style dialect (§2).
- **Master model confirmed over diagrams-as-source** (§2.1): diagrams are
  decomposed into a selection (stored as a query) plus presentation residue
  (stored layout/styles keyed to stable IDs).
- **Custom element kinds via aliases** (§3.1): a `specification` block defines
  vocabulary sugar desugaring to base C4 kind + tags + styles; the model layer
  keeps the fixed C4 hierarchy. Structural custom kinds deferred indefinitely.
- **Time via named milestones** (§8): lifecycles reference milestone names,
  dates are optional labels updated in one place; `asof <date>` may later
  resolve to the nearest milestone as sugar.
- **`splitBy` emits separate views**; the combined kind-styled diagram is
  simply the default when `splitBy` is absent.
- **`owner` and `layer` are the only blessed well-known properties** (§6.2):
  stored in the plain properties map, but tooling-aware (default `rollup`
  partition, digest column, optional lint). The list does not grow casually.

## 1. Problem statement

We want a system for specifying and communicating software / system / enterprise
architecture that is:

1. **LLM-native** — an LLM can read, write, and refactor models without burning
   context on format instructions, and can self-correct from tool feedback.
2. **Human-first** — plain text, terse, diffable, learnable in ten minutes.
3. **Progressive-fidelity** — the same format holds a napkin sketch
   ("there's a shop, it talks to billing somehow") and a detailed spec
   (typed ports, connector semantics, quality-attribute annotations, attached
   docs), including **expectations at different points in time** (§8).
4. **Model/view separated** — one master model is the source of truth;
   views are *selections over the model*, mostly generated, never
   hand-maintained drawings.

## 2. Base format: the alternatives, honestly compared

### 2.1 Why a master model at all (vs. diagrams as the source)

A single diagram *is* a partial model; the difference between "a set of
diagrams" and "a model" is not the kind of content but what happens under
multiplicity and change:

1. **Identity** — in a model, `api` in two views is the same entity
   (rename propagation, impact analysis, consistency checks all depend on
   this). Two boxes labeled "API" in two standalone diagrams have no
   machine-checkable connection, so the union of diagrams is not well-defined.
2. **Absence semantics** — a missing edge in a diagram is ambiguous
   (doesn't exist? not shown here?). A model is closed-world within its
   fidelity level, which is what makes lint, impact analysis, and deltas
   computable. Omitting an edge from a view is presentation; omitting it from
   a diagram-as-source destroys the fact.
3. **Normalization** — diagrams-as-source state each fact once per diagram
   that shows it (update anomaly; this is why wiki diagrams rot). The model is
   the normalized form; views are materialized projections.
4. **Derivation asymmetry** — model → diagram is a cheap deterministic
   projection (selection + layout); diagram → model is entity resolution and
   conflict reconciliation, i.e. guesswork.

Any tooling that made a set of diagram files behave consistently would have to
construct this shared-identity, closed-world structure internally — the model
doesn't disappear, it just gets hidden in a denormalized encoding. What
diagrams genuinely add — curation, emphasis, layout — is kept, decomposed into
a *selection* (stored as a query, §6) and *presentation residue* (stored x/y
and styles keyed to stable IDs). The accepted cost is losing WYSIWYG
(a view must be generated to be seen), mitigated by deterministic generation,
the live-reload server, and Mermaid/PlantUML as export targets. Sketch mode
(§4.1) covers the degenerate case where model and diagram coincide: one file
of arrows is both.

### 2.2 Authoring format candidates

Candidates: Mermaid, PlantUML (C4 macros), LikeC4, Structurizr DSL, a new
custom language.

| | Master model (single source, many views) | Views as queries | LLM training-data familiarity | Terseness | Existing support in this repo |
|---|---|---|---|---|---|
| Mermaid | ✗ — each diagram is a standalone drawing | ✗ | very high | high | exporter |
| PlantUML + C4 macros | ✗ — diagram-per-file, macro calls | ✗ | high | low (verbose macros) | exporter |
| LikeC4 | ✓ | ✓ (predicates) | low–medium (niche, growing) | high | none (upstream is TypeScript) |
| Structurizr DSL | ✓ | partial (filtered views, expressions) | medium–high | high | full parser + model + renderers |
| Custom language | ✓ (by design) | ✓ (by design) | zero | whatever we choose | none |

**Mermaid and PlantUML are disqualified as the authoring format** by
requirement 4 alone: they are *diagram* languages, not *model* languages. There
is no way to state a fact once and derive views from it — you'd redraw the same
relationship in every diagram, and they drift. Their enormous value is as
**outputs**: a generated view exported to Mermaid renders natively in GitHub
READMEs and PRs, which is where humans and LLMs actually encounter diagrams.
That pipeline (model → generated view → Mermaid/PlantUML in docs) gets us
Mermaid's ubiquity without its statelessness. Both exporters already exist here.

**LikeC4 is the closest existing system to this spec** — model-first,
predicate-based views, user-defined element kinds, deployment and dynamic
views. We adopt its best ideas outright: the view-predicate semantics (§6.1)
and, as a possible later extension, specification-defined custom element kinds
(§11 Q1). We do not adopt its syntax wholesale because (a) this repo already
has a complete Structurizr DSL toolchain and test fixtures, (b) Structurizr DSL
has materially more LLM training-data presence, and (c) chasing a moving
TypeScript upstream costs more than borrowing its design.

**Decision: author in a Structurizr-DSL *dialect*.** The parser continues to
accept upstream Structurizr DSL (existing fixtures keep passing), and our
extensions reuse its lexical style (braces, `ident = type "name"` declarations)
so LLM priors carry over. Since upstream interop is not a requirement, we do
**not** contort extensions to also be valid upstream — where Structurizr's
design is awkward (comma-joined tag strings, positional quoted arguments) new
constructs use explicit keywords instead.

What we deliberately do **not** do:

- No new general-purpose modeling language (UML/SysML/ArchiMate scope creep).
- No YAML/JSON authoring format — verbose for graphs, quoting-error-prone, and
  the prior-knowledge advantage is weak for a bespoke schema.
- No mandatory metadata. Every extension below is optional; the minimum valid
  model is a list of arrows (§4.1).

## 3. Concept inventory

Everything in the system is one of:

| Concept | Origin | New here? |
|---|---|---|
| Element (person, system, container, component, custom) | C4/Structurizr | no |
| Relationship | C4/Structurizr | enriched (§5) |
| **Port** | C&C viewtype practice | **yes** (§5.1) |
| **Connector** (n-ary) | C&C viewtype practice | convention + tooling (§5.3) |
| Group / layer | Structurizr groups | selector support (§6) |
| Perspective (quality-attribute lens) | Structurizr | promoted + view support (§7) |
| Status / fidelity | — | **yes** (§4.2) |
| **Milestone** (time) | — | **yes** (§8) |
| View | Structurizr/LikeC4 | mostly **generated** (§6) |
| Docs / ADRs attached to elements | Structurizr | no (prose lives in markdown) |

The division of labor for "detailed spec/documentation": the DSL holds
*structure* (things, interfaces, connections, attributes); prose specs live as
markdown documents attached to elements (`!docs`, `!adrs`). We never try to
express paragraphs in the DSL.

### 3.1 Kind aliases

Domain vocabulary without touching the model layer: a `specification` block
defines element-kind aliases that desugar to a base C4 kind plus tags (and
optionally default technology/styles):

```text
specification {
    kind queue container { tags "Queue,Connector" }
    kind lambda container { tags "Serverless" technology "AWS Lambda" }
}
model {
    orders = queue "Order queue" "Kafka"
}
```

`orders` is stored as a plain container tagged `Queue,Connector`; renderers and
upstream-compatible JSON see nothing new. Selectors resolve aliases:
`element.kind==queue` matches elements declared through the alias (tracked via
a `kind` property). This captures most of the value of LikeC4's user-defined
kinds at zero model-schema cost; *structural* custom kinds (own nesting rules,
own JSON shape) are deferred indefinitely.

## 4. Progressive fidelity

### 4.1 Sketch mode

A file that contains no `workspace` block is a **sketch**: bare statements are
implicitly wrapped in `workspace { model { ... } }`, and identifiers that are
used but never declared are auto-created as placeholder software systems
(tagged `Placeholder` — a software system is the natural "thing we haven't
detailed yet", and it participates in all standard views). Dotted identifiers
must resolve to declared elements even in sketch mode.

```text
customer -> shop "buys things"
shop -> billing "somehow charges" ?
billing -> erp
```

This is a complete, valid model. It renders, it validates, it can be queried.
`?` on a relationship (or element) marks it explicitly *uncertain* — kept
distinct from merely-undetailed.

Inside a full `workspace` file, strictness is the default (undeclared
identifier = parse error, as today). A `!sketch` directive at the top of a
workspace opts into auto-vivification for that file.

### 4.2 Status

Any element or relationship can carry a lifecycle status:

```text
billing = softwareSystem "Billing" {
    status idea        // idea | draft | specified | implemented | deprecated
}
```

Stored as a first-class optional field (not a tag), defaulting to unset.
Selectors can filter on it (`status==idea`), styles can theme it (dashed
"sketchy" borders for ideas by default), and `structurizrx validate
--min-status specified` can gate CI on fidelity.

Status is *confidence in the design*; it is orthogonal to *time* (§8). A
`status idea` element introduced at the five-year milestone is a vague
long-term intention; a `status specified` one is a committed roadmap item.

Refinement over time uses C4's own mechanism: the identifier stays stable while
the element gains internals (a placeholder becomes a `softwareSystem`, later
grows `container`s). No separate "refines" edge; stable IDs are the contract.

### 4.3 Multi-file

`!include <path>` splits large models; recommended layout is one file per
bounded context / subsystem plus a root workspace file. This is also the LLM
editing unit — an agent edits one subsystem file without loading the whole
enterprise.

## 5. Relationships, ports, connectors

### 5.1 Ports

A **port** is a named interaction point on any element. The question it
answers, which plain relationships cannot: *what does this element offer or
require, independent of who is currently connected* — and *which of the twelve
inbound arrows go through the same contract*.

```text
api = container "API" {
    port rest "Customer REST API" {
        protocol "HTTPS/JSON"
        direction in           // in | out | inout (default)
        description "Public API, versioned, rate-limited"
    }
    port events "Order events" {
        protocol "Kafka"
        direction out
    }
}
```

Relationships may attach to ports with dot syntax; attaching to the element
directly stays legal (that's the low-fidelity form):

```text
web -> api.rest "calls"
api.events -> billing.orders "OrderPlaced" { kind async }
```

Model change: `ports: Option<Vec<Port>>` on element structs;
`source_port_id` / `destination_port_id` on `Relationship`. A port is a
`ModelItem` (id, tags, properties, perspectives) plus name, protocol,
direction, description. Unbound ports (declared, never connected) are visible
and lintable — declared-but-unconsumed interfaces are information, not errors.

### 5.2 Relationship kinds

Relationships gain an optional `kind` — a **closed** vocabulary of connector
semantics, richer than free-text `technology` + binary `interactionStyle`:

```text
api -> db "reads/writes" { kind sync }
api -> queue { kind publish }
worker -> queue { kind subscribe }
web -> cdn { kind dataflow }
frontend -> designSystem { kind dependency }   // build-time, not runtime
```

Initial vocabulary: `sync`, `async`, `publish`, `subscribe`, `dataflow`,
`dependency`, `deploy`. Selectors and view splitting key off it (§6).
`dependency` matters: it lets one master model serve both runtime and
build-time viewtypes — views filter by kind instead of us maintaining two
models.

Relationships can also be **named** (`orderFlow = api.events -> billing ...`)
so dynamic views, docs, and perspectives can reference them.

### 5.3 N-ary connectors

A bus, broker, or shared database is modeled as a normal element tagged as a
connector:

```text
bus = container "Event bus" "Kafka" {
    tags "Connector"
}
```

No new grammar. The tooling makes it worthwhile: views can **collapse**
connector-tagged elements, replacing `a -publish-> bus -subscribe-> b` with a
derived `a -> b` edge labeled via the bus (§6.3). High-level views show intent;
detailed views show the machinery.

## 6. Views as selections

The master model is authored; views are declared as **selections**, and the
common ones need zero declaration at all.

### 6.1 The induced-subgraph rule

A view is fundamentally a *set of elements*, however that set is produced.
Given the set:

- every model relationship whose two endpoints are both in the set is included
  automatically (minus explicit `exclude`s);
- ancestors of included elements are pulled in as boundary boxes for rendering
  context.

So a hand-written `include a b c` is already a query — the author lists
elements, the system derives the edges. This matches LikeC4's predicate
semantics and upstream Structurizr behavior, and it is the reason views never
have to enumerate relationships. Selectors (§6.2) and generators (§6.3) are
just increasingly powerful ways to produce the element set.

### 6.2 Selectors — declarative filters

One expression language used by `include` / `exclude`, generator arguments, and
the CLI `query` command:

```text
->api->                 // neighborhood: api + direct neighbors (exists today)
element.tag==Database
element.kind==container
element.status==idea
element.layer==domain           // layer = group name or `layer` property
element.perspective==security   // carries a perspective with that name
element.parent==shop            // direct children; parent^==shop for transitive
element.technology==Kafka
element.property.owner==checkout-team
relationship.kind==async
relationship.tag==critical
a && b, a || b, !a              // boolean combinators
```

**Well-known properties.** Exactly two entries of the free-form `properties`
map are blessed with tooling awareness: `owner` (default `rollup` partition,
digest column, optional unowned-element lint) and `layer` (layer views,
layer-order lint). They remain ordinary properties in storage; blessing means
documentation plus defaults, and the list does not grow casually.

Selectors *filter*; they cannot compute anything that requires walking the
graph. That's what generators are for.

### 6.3 Generators — computed views

Each generator exists because some stakeholder question requires graph
computation, not filtering. The catalog, organized by the question it answers:

**"What is this made of, and what surrounds it?"** — the C4 zoom ladder.

```text
views {
    auto           // landscape + context per system + container view per
                   // non-empty system + component view per non-empty container
}
```

Zero-config default when the `views` block is absent or says `auto`.
Sketch-mode files get a single all-elements view.

**"What breaks if I change X?" / "What does X need?"** — reachability.

```text
auto focus api {
    depth 2                // default 1; * = transitive closure
    direction in           // in = impact analysis (who depends on me)
                           // out = dependency analysis (what do I need)
                           // both (default)
    splitBy kind           // one *separate view* per relationship kind
                           // present; also: splitBy tag | layer
}
```

Without `splitBy`, `focus` emits a single combined view in which kinds are
distinguished by arrow styling (§10) — so "split into views" and "group
visually in one view" are the same mechanism with and without the modifier.

`focus` is the "selected object and all its relationships, split into views"
generator. Direction matters: impact analysis is *inbound transitive*,
dependency analysis is *outbound transitive*; a plain neighborhood is neither.

**"How are X and Y connected at all?"** — path enumeration.

```text
auto paths user db        // all simple paths user → db, e.g. "how does a
                          // request reach the database"
```

**"Where does concern C live?"** — cross-cutting slices (perspective,
technology, layer, tag, status). Mostly sugar over selectors + induced rule:

```text
auto perspective "security"     // all items carrying that perspective, plus
                                // structural context (§7)
auto layer "domain"
auto slice relationship.kind==dataflow   // elements induced by matching rels
auto slice element.status==idea          // everything still hypothetical
```

**"Who talks across team/partition boundaries?"** — partition rollup.

```text
auto rollup                   // defaults to the blessed `owner` property:
                              // one node per owner, members merged, edges
                              // aggregated; also: rollup group, rollup layer,
                              // rollup element.property.<any>
```

The Conway view: internals of each partition disappear, only cross-partition
edges remain. Works for teams (`owner` property), layers, or groups — and it
generalizes enterprise-level views without new element types.

**"How does scenario S unfold?"** — dynamic views (already exist; ordered
interactions, unchanged).

**"What runs where?"** — deployment views (already exist, unchanged).

**"What changes between now and milestone M?"** — temporal (§8).

```text
auto asof billingSplit          // model state at that milestone
auto delta now billingSplit     // migration view: added/removed highlighted
```

**"What's unfinished or inconsistent?"** — model hygiene, and the LLM
feedback loop:

```text
auto lint       // placeholder elements, ?-marked items, unbound ports,
                // orphan elements, layer-order violations (if layers declare
                // an order), relationships to retired elements
```

**Modifiers** applicable to any view:

```text
collapse element.tag==Connector   // fold n-ary connectors (§5.3);
                                  // can also be set workspace-wide
```

Generated views get deterministic keys (`focus-api-async`,
`delta-now-billingSplit`) so links and stored layout survive regeneration.
Hand-authored views (existing syntax) remain for curated diagrams; stored x/y
layout continues to take precedence over auto-layout, as today.

Considered and deliberately cut for now: code-level views (component→source
mapping — needs a scanner, different project), cost/capacity views (not
structural), auto-sequenced scenarios (ordering can't be inferred from a static
model).

## 7. Quality attributes

Build on Structurizr **perspectives** (already in this repo's model: name /
description / value on any `ModelItem`) rather than inventing a construct:

```text
api -> db "reads/writes" {
    kind sync
    perspective "performance" "p99 < 50ms, 2k rps"
    perspective "availability" "degrades read-only if db is down"
}
```

Additions:

1. Perspectives are declarable on **relationships and ports**, not just
   elements.
2. Optional workspace-level perspective registry, so `auto perspective *` can
   enumerate them and validation can catch typos:
   ```text
   perspectives {
       security "STRIDE-reviewed boundaries"
       performance
   }
   ```
3. `auto perspective "X"`: all items carrying X, plus enough structural context
   (ancestors) to render coherently.

Full quality-attribute *scenarios* (stimulus/response/measure) stay in attached
markdown docs, linked from the perspective description — structure in DSL,
prose in docs, per §3.

## 8. Time: modeling expectations at different horizons

The model should hold not just the system as it is, but as we expect it to be
in a month, in five months, in five years — without maintaining N divergent
workspace copies that drift apart.

### 8.1 Milestones

A workspace declares named, ordered milestones (dates are optional labels;
declaration order is the ordering):

```text
milestones {
    mvp          "2026-08"
    billingSplit "2026-12"  "Billing extracted from the monolith"
    target       "2031"     "Target architecture"
}
```

`now` is an implicit milestone preceding all declared ones. Lifecycles always
reference milestone *names*, never raw dates — when a plan slips, the date
changes in exactly one place and every `introduced`/`retired` stays correct.
If all milestones carry dates, `asof <date>` may later be added as sugar
resolving to the latest milestone at or before that date.

### 8.2 Element and relationship lifecycles

Any element or relationship can state when it enters and leaves the
architecture:

```text
billing = softwareSystem "Billing" {
    introduced billingSplit
    status specified
}
legacyCrm = softwareSystem "Legacy CRM" {
    retired billingSplit
}
monolith -> billingDb "reads directly" {
    retired billingSplit        // this edge disappears when billing splits out
}
```

Unmarked items exist at all times. An item exists at milestone M iff
`introduced ≤ M < retired`. That's the whole mechanism — two keywords, and one
master model spans the roadmap. (This is ArchiMate's plateau/migration idea,
reduced to its useful core.)

### 8.3 Temporal views

- `auto asof <milestone>` — the model as of that milestone. `asof now` filters
  out everything future.
- `auto delta <m1> <m2>` — union of both states with additions and removals
  visually marked (default styling: green added, red/struck removed). This *is*
  the migration plan diagram.
- Every other generator accepts an optional `asof` (e.g.
  `auto focus api { asof target }`).
- Default rendering of non-temporal views is `asof now` — future elements
  don't pollute current diagrams unless asked for. `validate` warns on
  relationships that connect items with non-overlapping lifetimes.

## 9. LLM affordances

Product features, not language features — they matter as much as the syntax:

1. **`structurizrx digest <ws>`** — compact plain-text model summary (elements
   one per line with qualified ids, relationship triples with kinds, ports,
   perspectives, milestones) designed to be pasted into LLM context. Target: an
   enterprise-sized model digests to a few KB.
2. **`structurizrx query <ws> "<selector>"`** — run a §6.2 expression, get
   matching elements/relationships as text or JSON. Lets an agent explore a big
   model without reading every file.
3. **Validation with machine-fixable errors** — line/col, expected-token, and
   "did you mean" for unknown identifiers, so a writing agent converges in one
   or two retries. `validate --strict` also runs the §6.3 lint set.
4. **One-page cheat sheet** — the *entire* extension surface (ports, kind,
   status, sketch mode, milestones, generators, selectors) must fit one page,
   shipped as `llms.txt` and served by `structurizrx serve`. If an extension
   doesn't fit on the page, the extension is too big.
5. **Deterministic output** — stable ordering in JSON/digest/exports so diffs
   are reviewable and agents can verify their own edits.

## 10. Compatibility rules

- DSL: continues to *read* upstream Structurizr DSL (fixture tests keep
  passing). Our extensions need not be valid upstream.
- JSON: our schema is a superset shape of Structurizr's (new fields optional,
  `skip_serializing_if = None`); an optional lossy `--compat structurizr`
  export folds extensions into `properties` if ever needed.
- Renderers: Mermaid/PlantUML/DOT exporters are first-class *outputs* — kinds
  map to arrow styles, ports degrade to edge labels or small nodes, delta views
  degrade to +/- prefixes in labels. SVG gets first-class treatment (port
  glyphs on element borders, added/removed styling).

## 11. Implementation plan

Phases are independently shippable; each ends green (`cargo test`, fixtures
intact). **All six phases are implemented** (kept below as the record of
scope); remaining known gaps are tracked in the repository README's Status
section.

1. **Done.** **Model extensions** (`structurizr-model`): `Port`, `Relationship` gains
   `kind` / port ids / optional identifier, `status`, `introduced`/`retired` +
   `Milestone`, perspective registry, perspectives on relationships/ports.
   JSON round-trip tests.
2. **Done.** **DSL extensions** (`structurizr-dsl`): `port` blocks, dotted port refs,
   `kind` / `status` / `perspective` / `introduced` / `retired` in bodies,
   named relationships, `milestones` block, `specification` kind aliases,
   `!sketch` + bare-sketch-file parsing, `?` markers, `!include`.
3. **Done.** **Selector engine** (new `structurizr-query` module/crate): parse + evaluate
   §6.2 expressions; wire into `include`/`exclude`; `structurizrx query`.
4. **Done.** **View generation**: default set, `focus`, `paths`, `slice` /
   `perspective` / `layer`, `rollup`, `asof` / `delta`, `lint`, `splitBy`,
   `collapse`, deterministic view keys.
5. **Done.** **LLM affordances**: `digest`, validation/lint upgrades, `llms.txt` cheat
   sheet, deterministic-ordering audit.
6. **Done.** **Rendering**: port glyphs in SVG, kind→arrow-style mapping in all
   exporters, status theming, delta styling, web viewer index grouped by
   generator/perspective/milestone.

Beyond this original plan, an agent-experience hardening pass (2026-07-20)
made the parser strict by default with did-you-mean errors and include-aware
locations, added a structured lint API and `validate --json`, and exposed
`digest`/`query` over the web server's JSON API.

## 12. Open questions

None currently — all questions from revisions 1–2 are resolved; see the
decisions log (§0). New questions raised during implementation get added here
before being settled.
