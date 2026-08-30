# structurizr-evals

A deterministic eval harness for the LLM-agent workflow documented in
[`skills/structurizrx/SKILL.md`](../../skills/structurizrx/SKILL.md) and
spec'd in `docs/SPEC.md` §9 ("LLM affordances"): can an agent, given a
modeling task and nothing but the `structurizrx` CLI, converge on a correct
`.dsl` file?

This crate does **not** call any LLM. It is the grading half only: a bank of
tasks (`tasks/*.json`, each a natural-language prompt plus a rubric) and a
grader (`src/lib.rs`) that checks a candidate `.dsl` against its task's
rubric using the same machinery the workflow itself relies on —
`structurizr_model::validation::validate`, `structurizr_query::lint`,
`structurizr_query::query` (selector expressions), and
`structurizr_query::digest`.

## Running the suite

```sh
cd rust
cargo test -p structurizr-evals -- --nocapture
```

Every task under `tasks/` is graded against its committed reference
solution under `candidates/`. The reference solutions are hand-written and
`--strict`-clean, so this is green on `main`; it also regression-tests the
rubric-checking logic itself.

## Grading an agent's attempt

1. Pick a task file under `tasks/`, e.g. `tasks/003-sketch-mode.json`, and
   hand an agent its `prompt` field — nothing else. (For a realistic run,
   give it access to the `structurizrx` CLI and the `structurizrx` Claude
   Code skill, and nothing about this eval suite itself.)
2. Let it run the workflow from `SKILL.md`: `structurizrx docs` once, write
   `.dsl`, loop on `structurizrx validate --strict --json` until clean,
   confirm with `structurizrx digest`.
3. Save its output over the task's `candidate` path (or a scratch copy —
   point `cargo test` at an alternate path by temporarily editing the
   task's `"candidate"` field, then restore it before committing).
4. Run `cargo test -p structurizr-evals -- --nocapture`. The eval signal is
   pass/fail per rubric line, printed with the exact selector or check that
   failed — e.g. `required: selector \`element.layer==Data\` matched
   nothing` — the same "machine-fixable" precision `validate` gives an
   agent, so a failed run is something to act on, not just a score.
5. Diff the agent's `.dsl` against the committed reference solution as a
   qualitative baseline — passing the rubric means "structurally correct,"
   not "identical to the reference."

## Task format (`tasks/*.json`)

```json
{
  "id": "004-deployment-and-groups",
  "title": "...",
  "prompt": "the natural-language task, verbatim, handed to the agent",
  "candidate": "candidates/004-deployment-and-groups.dsl",
  "rubric": {
    "strict_valid": true,
    "required": ["selector expression", "..."],
    "forbidden": ["selector expression", "..."],
    "digest_contains": ["substring", "..."]
  }
}
```

All rubric fields are optional (default: not checked).

- `strict_valid` — `structurizr_model::validation::validate` and
  `structurizr_query::lint` must both come back empty, matching what
  `structurizrx validate --strict` reports.
- `required` — each selector (spec §6.2 grammar) must match at least one
  element or relationship.
- `forbidden` — each selector must match nothing (e.g.
  `element.tag==Placeholder` to catch a sketch an agent never firmed up).
- `digest_contains` — substrings the plain-text `structurizr_query::digest`
  output must contain. Useful for things selectors can't express, like
  deployment-view presence (deployment nodes aren't queryable elements —
  see `src/lib.rs`) or port names.

Note: element identifiers are matched by selectors independent of DSL
variable names — a task's rubric checks the *model* (kind, name, tags,
technology, layer/group, relationship kind), never the identifiers an agent
happened to choose.

## Tasks in this suite

| Task | Workflow feature under test |
| --- | --- |
| `001-system-context` | Baseline: person/softwareSystem/relationship + the validate loop |
| `002-ports-and-kinds` | Ports, relationship `kind`, element `status` |
| `003-sketch-mode` | Sketch → firmed-up model; placeholder/uncertain lint convergence |
| `004-deployment-and-groups` | `group`/layers, `deploymentEnvironment`, deployment views |
| `005-multi-file` | Splitting a workspace across files with `!include` |
