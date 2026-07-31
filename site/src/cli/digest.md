# structurizrx digest

Print a compact plain-text summary of the model and view set, sized to paste
into an LLM's context window.

```sh
structurizrx digest <file>
```

Generated (`auto`) views are materialized first, so the digest reflects the
effective view set — the same thing `render`/`serve` would produce, not just
what's literally written in the `views` block.

The digest lists elements one per line with qualified ids, relationship
triples with their `kind`, ports, perspectives, and milestones. Target size
for an enterprise-sized model is a few KB — small enough that an agent can
hold the whole architecture in context alongside its actual task, without a
separate retrieval step.

```sh
structurizrx digest ws.dsl
```
```text
workspace: Shop

person Customer
system Shop
  container Shop/Web App [TypeScript]
  container Shop/API [status:implemented] ports: Customer REST API(HTTPS/JSON)
  container Shop/Database

rel Customer -> Shop/Web App "shops on"
rel Shop/Web App -> Shop/API.Customer REST API "calls"
rel Shop/API -> Shop/Database "reads and writes" [sync]

view auto-landscape landscape (2 elements, 0 rels)
view auto-context-shop systemContext of Shop (1 elements, 0 rels)
view auto-container-shop container of Shop (4 elements, 3 rels)
```

Markers like `[status:implemented]` and `[sync]` (relationship `kind`) only
appear when the model sets them; milestones and perspectives, when present,
are summarized in the header alongside `workspace:`/`description:`.
