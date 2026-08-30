# Multi-file workspaces

`!include <path>` splits a large model across files, resolved relative to the
including file's own directory:

```text
// root.dsl
workspace "Enterprise" {
    model {
        !include shop.dsl
        !include billing.dsl
    }
    views { auto }
}
```

```text
// shop.dsl
shop = softwareSystem "Shop" {
    web = container "Web App"
}
```

Recommended layout is **one file per bounded context or subsystem**, plus a
root workspace file that just wires them together with `!include` and
declares cross-subsystem relationships. This is also the natural editing
unit for an LLM agent: it can load and edit one subsystem file without
pulling the whole enterprise model into context, then re-`validate` just its
change.

Parse errors inside an included file report **that file's own path and line
number**, not the root file's — so "line 5 of shop.dsl" points exactly where
an agent needs to look, even several `!include` levels deep.

Full runnable example: a root workspace
[`catalog.dsl`](https://github.com/pomali/structurizrx/blob/main/site/examples/catalog.dsl)
that `!include`s two subsystem files from a
[`catalog/`](https://github.com/pomali/structurizrx/tree/main/site/examples/catalog)
subdirectory and wires a cross-subsystem relationship between them. Note
that identifiers declared inside an included file (`ordersApi`, `db`, ...)
are flat, top-level names once included — not accessed as `orders.api` —
so give elements you need to reference from outside their own file a
globally unique identifier.

![System landscape view: Customer, Orders and Customers systems](./images/catalog/auto-landscape.svg)
