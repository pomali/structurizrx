# structurizrx lint

Check a workspace's model hygiene: findings that block a build, softer
warnings, and dependency cycles.

```sh
structurizrx lint <file> [--strict] [--json]
```

| Flag | Effect |
|---|---|
| `--strict` | Also fail (exit 1) on warnings and cycles, not just blocking findings |
| `--json` | Emit `{passed, blocking: [...], warnings: [...]}` instead of a text report |

`lint` exits 1 whenever there are blocking findings, and — with `--strict` —
also when there are warnings or cycles. Without `--strict` it exits 1 only
for blocking findings, matching `validate --strict`.

## How this differs from `validate --strict`

`validate --strict` fails only on the four **blocking** findings below — it's
the gate a build should never pass with an unfinished model. `lint` reports
the same blocking set, plus hygiene **warnings** (a missing description isn't
wrong, just unfinished) and **dependency cycles**, which `validate` doesn't
look at all. `lint --strict` is the stricter check: it fails on warnings and
cycles too, so it's suited to a "is this workspace actually done" gate rather
than "is this workspace well-formed."

## Findings

Blocking (also fail `validate --strict`):

| Code | Meaning |
|---|---|
| `placeholder` | Element was auto-created in sketch mode and never properly declared |
| `uncertain` | Element is tagged `Uncertain` |
| `orphan` | Element has no relationships and no children |
| `unbound-port` | Port is declared but never referenced by a relationship |

Warnings (only fail with `--strict`):

| Code | Meaning |
|---|---|
| `missing-description` | Element has no description |
| `missing-technology` | Container or component has no technology |
| `not-in-any-view` | Element is not shown by any view |
| `no-relationships` | Element has no incoming or outgoing relationships and no children |
| `duplicate-name` | Element's name is hard to tell apart from another's |
| `relationship-undescribed` | One or more relationships leaving the element have no description |
| `cycle` | A dependency cycle exists between elements (container level, and component level when the model has components) |

Generated (`auto`) views are materialized first, so `not-in-any-view` is
judged against the effective view set, not just what's literally written in
the `views` block.

## Example

`site/examples/sketch.dsl` (see [Sketch mode](../language/sketch-mode.md)) is
all placeholders, so it fails outright:

```sh
structurizrx lint sketch.dsl
```
```text
Blocking:
  [placeholder] 'customer' is a placeholder auto-created in sketch mode; declare it properly (customer)
  [placeholder] 'shop' is a placeholder auto-created in sketch mode; declare it properly (shop)
  [placeholder] 'billing' is a placeholder auto-created in sketch mode; declare it properly (billing)
  [placeholder] 'erp' is a placeholder auto-created in sketch mode; declare it properly (erp)

Warnings:
  [missing-description] 'customer' has no description (customer)
  [missing-description] 'shop' has no description (shop)
  [missing-description] 'billing' has no description (billing)
  [relationship-undescribed] 1 of 1 relationship(s) leaving 'billing' have no description (billing)
  [missing-description] 'erp' has no description (erp)

4 blocking, 5 warnings
```

A finished workspace prints nothing:

```sh
structurizrx lint ws.dsl
```
```text
✓ no findings
```

```sh
structurizrx lint ws.dsl --json
```
```json
{
  "blocking": [],
  "passed": true,
  "warnings": [
    {
      "blocking": false,
      "code": "missing-description",
      "elementId": "1",
      "message": "'Customer' has no description",
      "name": "Customer"
    }
  ]
}
```
