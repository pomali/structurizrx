# Introduction

StructurizrX is a Rust toolchain for describing software architecture as a
**plain-text model** and generating diagrams from it — a re-implementation of
[Structurizr](https://structurizr.com/) and the
[C4 model](https://c4model.com/), evolving into an **LLM-native architecture
description system**: one master model, views as queries over it, and tool
feedback precise enough that both humans and LLM agents can self-correct from
error messages alone.

```text
structurizrx validate ws.dsl
parse error at line 5, column 17: unknown element identifier 'shoop'
in relationship (did you mean 'shop'?)
```

## Why a master model

Diagrams rot because they state each fact once per diagram. If `api` calls
`billing` and that fact lives only inside a hand-drawn box-and-arrow picture,
nothing stops a second diagram from drifting out of sync the moment the
relationship changes. StructurizrX keeps **the model as the single source of
truth** and treats views as *selections over it* — stored as queries
(`auto focus api`, `auto slice element.status==idea`), generated
deterministically, never hand-maintained.

Mermaid and PlantUML remain excellent **outputs** — a generated view exported
to Mermaid renders natively in GitHub READMEs and PRs — but neither is a model
language: there's no way to state a fact once and derive many views from it.
StructurizrX's DSL is the model layer; Mermaid/PlantUML/DOT/SVG are projections
of it.

## Progressive fidelity

The same format holds a napkin sketch ("there's a shop, it talks to billing
somehow") and a detailed spec (typed ports, connector semantics,
quality-attribute annotations, milestones). Start with arrows:

```text
customer -> shop "buys things"
shop -> billing "somehow charges" ?
```

and grow into a full model with containers, ports, relationship kinds, and
generated views, all under the same identifiers — a placeholder becomes a
`softwareSystem`, later grows `container`s, without ever renaming anything
downstream.

## Where to go next

- [Install](./install.md) and [Quickstart](./quickstart.md) to get running.
- The [CLI reference](./cli/README.md) for every subcommand.
- The [Language reference](./language/README.md) for the full DSL — both the
  parts inherited from upstream Structurizr and StructurizrX's own extensions
  (ports, relationship kinds, status, milestones, generated views).

The full extended design, including the reasoning behind each extension, lives
in [`docs/SPEC.md`](https://github.com/pomali/structurizrx/blob/main/docs/SPEC.md)
in the repository.
