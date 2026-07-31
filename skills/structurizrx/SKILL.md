---
name: structurizrx
description: >-
  Author, inspect, validate, query, and render C4 software-architecture models
  with the `structurizrx` CLI (a Structurizr DSL toolchain). Use this whenever
  the task involves describing or diagramming software architecture — C4 models,
  system context / container / component / deployment diagrams, Structurizr
  `.dsl` or workspace `.json` files, or architecture-as-code. Reach for it even
  if "structurizr" is never named but the user wants an architecture diagram, a
  C4 model, or a machine-readable description of components and how they relate.
---

# structurizrx

`structurizrx` reads the Structurizr DSL, a text format for C4 architecture
models plus LLM-native extensions. It turns the DSL into validated models,
compact text digests, query results, and rendered diagrams. Use it instead of
hand-drawing or prose whenever the goal is a real, checkable model.

To list every command, run `structurizrx --help`. Each subcommand has its own
`structurizrx <cmd> --help`. Read those for exact flags. This skill covers only
how to drive the tool well.

If `structurizrx --version` fails, the CLI is not installed — see
[install.md](install.md) for the one-line install per platform.

## Read the DSL from the tool first

The tool documents the DSL and all its extensions: sketch mode, ports,
relationship kinds, milestones, `auto` views, and selector expressions. Before
you write any DSL, run `structurizrx docs`. It prints a short cheat sheet, and
that cheat sheet is the authoritative format reference. This dialect extends
upstream Structurizr and is strict by default, so do not write from memory.

## Workflow

1. `structurizrx docs` — load the DSL reference (once per session).
2. Write or edit the `.dsl` file.
3. `structurizrx validate ws.dsl --json` — confirm it parses. Check `errors`
   and `lint`, fix, and re-run. Add `--strict` to make lint findings fail.
4. `structurizrx digest ws.dsl` — read back a compact summary to confirm the
   model matches intent.
5. Render or serve only when an image is the actual deliverable.

Do not skip validate and digest. A model that renders is not always correct.

## Inspect with text, not pictures

To understand an existing workspace, stay in text. Text is cheaper and more
exact than a rendered image.

- `structurizrx digest ws.dsl` — the whole model (elements, relationships,
  views) as compact plain text sized for an LLM context. Read this before you
  answer questions about a workspace.
- `structurizrx query ws.dsl "<selector>" --json` — find specific elements or
  relationships. `--json` gives a parseable `{elements, relationships}`. The
  selector grammar is in `structurizrx docs`.

Use machine-readable output (`--json` on `validate` and `query`) whenever you
parse the result yourself instead of showing it to a person.

## Sketch a vague idea

When the architecture is still unclear, skip the `workspace` block. A file of
bare arrows is a sketch: unknown names become placeholders, `?` marks
uncertainty, and the tool generates a view. `validate --strict` then lists every
placeholder to firm up. See `structurizrx docs` for the exact rules.

## Notes

- **Strict by default.** Unknown identifiers cause an error, unless the file is
  a sketch. The message names the file and line and suggests a fix.
- **`.dsl` or `.json`.** Every command accepts either, so you can work directly
  on exported JSON.
- **Interop.** The extensions are not guaranteed to work in upstream Structurizr
  tools.
