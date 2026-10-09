# structurizrx diff

Compare two versions of a workspace **as models** — elements, relationships,
views and decisions added, removed or changed — not as text.

```sh
structurizrx diff <file> [--from <rev>] [--to <rev>] [--against <file>] [--json] [--fail-on-change]
```

| Flag | Default | Effect |
|---|---|---|
| `--from` | `HEAD` | Git revision to compare from |
| `--to` | the file on disk | Git revision to compare to |
| `--against` | — | Compare `file` (on disk) against another workspace file, instead of git history |
| `--json` | | Emit `{from, to, diff}` instead of a text report |
| `--fail-on-change` | | Exit 1 when anything changed (for CI gates) |

`--against` and `--from`/`--to` are mutually exclusive. Without `--against`,
`file` must be tracked by git; otherwise `diff` fails and suggests
`--against`.

## Keyed by name, not id

Everything is matched by an element's **canonical path** (ancestor names plus
its own name) and a relationship's two end paths — never by id. The DSL
assigns ids in parse order, so the same element gets a different id in two
revisions as soon as anything is declared earlier in the file. The
consequence: a rename reads as a removal plus an addition, since from the
model's point of view that's indistinguishable from a replacement. Likely
pairs are scored (unchanged description, then parent, then technology) and
only reported under "Likely renames" when one candidate wins outright — a
container split into three suggests nothing rather than the wrong thing.

```sh
structurizrx diff shop-v1.dsl --against shop-v2.dsl
```
```text
shop-v1.dsl -> shop-v2.dsl
elements +1 −1, relationships +1 −1, views =, decisions =

Elements:
  − Shop/API (container)
  + Shop/Gateway (container)

Relationships:
  − Shop/Web App -> Shop/API "calls"
  + Shop/Web App -> Shop/Gateway "calls"

Likely renames:
  Shop/API -> Shop/Gateway (renamed)
```

## `!include`d files

Multi-file workspaces are handled: a revision's `!include`s are read from
*that same revision*, so a comparison never splices one commit's entry file
with another's parts (or today's parts from disk).

## CI use

Fail a check step when a workspace changed since the last commit:

```sh
structurizrx diff ws.dsl --fail-on-change
```

Or capture the JSON for a bot comment:

```sh
structurizrx diff ws.dsl --from origin/main --to HEAD --json > diff.json
```

With no changes:

```sh
structurizrx diff ws.dsl
```
```text
no changes (HEAD -> working)
```
