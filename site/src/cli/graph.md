# structurizrx graph

Print the whole workspace as one graph: every element, view and decision a
node; every relationship, containment, deployment instance and view
membership a link. This is the data behind the viewer's universe graph page
(`/workspace/{name}/graph`).

```sh
structurizrx graph <file> [--format <json|dot>]
```

| Flag | Default | Effect |
|---|---|---|
| `--format` | `json` | `json` (the viewer's feed shape, `{workspaceName, nodes, links}`) or `dot` |

Generated (`auto`) views are materialized first.

```sh
structurizrx graph shop.dsl --format json
```
```json
{
  "workspaceName": "Shop",
  "nodes": [
    { "id": "e:1", "refId": "1", "name": "Customer", "kind": "person", "tags": ["Element", "Person"] },
    { "id": "e:2", "refId": "2", "name": "Shop", "kind": "softwareSystem", "tags": ["Element", "Software System"] },
    { "id": "e:3", "refId": "3", "name": "Web App", "kind": "container", "parentId": "e:2", "description": "Storefront", "technology": "TypeScript", "tags": ["Element", "Container"] }
  ]
}
```

## DOT / Graphviz

```sh
structurizrx graph shop.dsl --format dot
```
```text
digraph universe {
  label="Shop";
  node [fontname="sans-serif"];
  edge [fontname="sans-serif"];
  "e:1" [label="Customer", shape=ellipse];
  "e:2" [label="Shop", shape=box];
  "e:3" [label="Web App", shape=box];
  "e:1" -> "e:3" [label="shops on", style=solid];
  "e:2" -> "e:3" [style=dashed];
}
```

Containment edges are dashed, view/decision membership edges are dotted, and
relationships are solid. Pipe straight into Graphviz:

```sh
structurizrx graph shop.dsl --format dot | dot -Tsvg > universe.svg
```
