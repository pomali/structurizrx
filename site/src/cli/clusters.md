# structurizrx clusters

Cluster analysis at one level: detected communities and how well they agree
with the model's declared structure, cycles, bridges, articulation points and
per-element coupling metrics. It's the same analysis behind the viewer's
`/workspace/{name}/clusters` page.

```sh
structurizrx clusters <file> [--level <lvl>] [--include <tag>]... [--exclude <tag>]... [--no-implied] [--json]
```

| Flag | Default | Effect |
|---|---|---|
| `--level` | `container` | Project the model onto `system`, `container` or `component` |
| `--include` | — | Keep only elements carrying one of these tags (repeatable) |
| `--exclude` | — | Drop elements carrying one of these tags (repeatable) |
| `--no-implied` | off | Don't roll descendant relationships up onto their ancestors |
| `--json` | | Emit the full analysis as JSON instead of a text report |

Generated (`auto`) views are materialized first, matching `render`/`serve`.

## What each section means

- **Communities** — groups the Louvain algorithm finds by relationship
  density alone, independent of anything declared in the DSL. Each is
  compared against the structure the model *declares* (`group`, else the
  parent element): `agreeing/total agree with '<dominant group>'` shows how
  many members' declared parent matches the community's most common one.
  This is the read worth having — a community where few elements agree with
  the declared grouping is either a naming problem or an undeclared coupling.
- **Conformance findings** — the prose version of the community/declared-structure
  mismatch: elements whose actual coupling disagrees with where the DSL says
  they belong.
- **Cycles** — strongly connected components with more than one member: real
  circular dependencies, not just a bidirectional pair.
- **Bridges** — edges whose removal would disconnect the graph; each one is a
  single point of failure in the dependency structure.
- **Articulation points** — nodes whose removal would disconnect the graph,
  the node-level equivalent of a bridge.
- **Metrics table** — per element:
  - `afferent` — incoming dependencies (how many things depend on it)
  - `efferent` — outgoing dependencies (how many things it depends on)
  - `instability` — `efferent / (afferent + efferent)`; 0 is maximally
    stable (depended on, depends on nothing), 1 is maximally unstable
  - `pagerank` — relative importance by the same algorithm as web search
    ranking, computed over the dependency graph
  - `betweenness` — how often the element sits on the shortest path between
    two others; high values mark structural chokepoints

Structural algorithms (components, cycles, articulation points, bridges) come
from `petgraph`; Louvain communities and Brandes' betweenness are
implemented in `structurizr-query` directly, since petgraph has neither.
Louvain needs no random seed — it visits nodes in index order and breaks ties
towards the lowest community index — so a run is reproducible by
construction.

## Example

Real output from the bundled Big Bank example at component level:

```sh
structurizrx clusters bigbank.dsl --level component
```
```text
6 elements, 4 dependencies, 3 communities (modularity 0.406), 2 disconnected parts, 0 cycles, 4 bridges, 0 conformance findings

Communities:
  #0: Sign In Controller, Security Component [2/2 agree with 'API Application']
  #1: Accounts Summary Controller, Mainframe Banking System Facade [2/2 agree with 'API Application']
  #2: Reset Password Controller, E-mail Component [2/2 agree with 'API Application']

Bridges:
  Sign In Controller -> Security Component
  Accounts Summary Controller -> Mainframe Banking System Facade
  Reset Password Controller -> Security Component
  Reset Password Controller -> E-mail Component

Articulation points:
  Reset Password Controller
  Security Component

name                              afferent   efferent  instability  pagerank  betweenness
Sign In Controller                       0          1        1.000    0.1142       0.0000
Accounts Summary Controller              0          1        1.000    0.1142       0.0000
Reset Password Controller                0          2        1.000    0.1142       0.0000
Security Component                       2          0        0.000    0.2715       0.0000
Mainframe Banking System Facade          1          0        0.000    0.2202       0.0000
E-mail Component                         1          0        0.000    0.1656       0.0000
```

Filter to one part of the model with tags, and compare communities without
rolling up descendant relationships:

```sh
structurizrx clusters ws.dsl --level container --include Payments --no-implied --json
```
