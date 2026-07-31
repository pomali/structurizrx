# Language reference

StructurizrX reads standard Structurizr DSL (`workspace`/`model`/`views`,
`person`, `softwareSystem`, `container`, `component`, `group`, `deployment`,
`styles`, `!docs`, `!adrs`, `!include`) plus a set of extensions designed to
fit the same lexical style — braces, `ident = type "name"` declarations — so
an LLM's priors about Structurizr DSL carry over.

Upstream interop is **not** a design goal: StructurizrX can always *read*
upstream DSL (the parser is tested against the upstream fixture corpus), but
its own extensions aren't constrained to also be valid upstream DSL. Where
upstream's design is awkward (comma-joined tag strings, positional quoted
arguments), StructurizrX extensions use explicit keywords instead.

This reference is organized as:

- [Core structure](./core-structure.md) — `workspace`/`model`/`views`, the
  element hierarchy, groups
- [Sketch mode](./sketch-mode.md) — files with no `workspace` block
- [Element extras](./element-extras.md) — `status`, `introduced`/`retired`,
  `perspective`, `port`, `tags`/`technology`/`url`
- [Relationships and ports](./relationships.md) — `kind`, named
  relationships, `?` uncertainty, port-attached relationships
- [Workspace-level blocks](./workspace-blocks.md) — `milestones`,
  `perspectives`, `specification` (kind aliases), `styles`
- [Views](./views.md) — hand-authored views, selectors, and the `auto`
  generator family
- [Documentation and decisions](./documentation.md) — `!adrs`/`!decisions`,
  `!include`
- [Multi-file workspaces](./multi-file.md) — splitting large models

For the condensed, single-page version of everything below (the format
StructurizrX ships to LLM agents), see `structurizrx docs` or `GET /llms.txt`
on a running `structurizrx serve` instance. The full design rationale behind
each extension lives in
[`docs/SPEC.md`](https://github.com/pomali/structurizrx/blob/main/docs/SPEC.md).
