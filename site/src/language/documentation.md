# Documentation and decisions

The division of labor: the DSL holds *structure* (elements, ports,
connections); prose lives in markdown, attached via directives. StructurizrX
never tries to express paragraphs in the DSL itself.

## `!adrs` / `!decisions` — architecture decision records

```text
workspace "Shop" {
    !adrs decisions
    model { ... }
}
```

`!adrs <path>` (alias `!decisions`) points at a directory of Markdown files
in **AdrTools/MADR format**: filenames start with a numeric ID
(`0001-use-postgres.md`), the ID becomes the decision's id (leading zeros
stripped: `0001` → `1`); the first line `# 1. Use PostgreSQL` supplies the
title; a line `Date: 2026-07-04` supplies the date; a `## Status` section
supplies the status (Proposed/Accepted/Superseded/etc.). Files are read in
filename-sorted order.

Decisions are served by `structurizrx serve` at
`/workspace/{name}/decisions` (list) and `/workspace/{name}/decisions/{id}`
(single ADR, rendered from Markdown to HTML), and over the JSON API at
`/api/workspace/{name}/decisions[/{id}]`.

Next: [Multi-file workspaces](./multi-file.md) — splitting a large model
across files with `!include`.
