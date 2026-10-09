# Demo: a Twitter-like social network

A large workspace to explore in the interactive viewer: a microblogging
platform with timelines, search, direct messages, ads, trust & safety and a
data platform — 6 people, 9 external systems, 54 containers in 13 groups,
two containers broken down into components, ~170 relationships, a
production deployment and three dynamic scenarios.

**[Open the interactive demo →](demo/twitter/index.html#landscape)**

The model is an educated approximation built from public engineering
write-ups, not an accurate description of any real company's systems.

## What to look at

| View | Shows |
|---|---|
| `containers` | Everything at once — the stress test |
| `platform-core` | The read and write paths without ads, data platform, safety and operations |
| `post-tweet`, `read-home-timeline` | Dynamic views: the write path with fanout, and the ranked read path |
| `home-timeline`, `fanout` | Component views inside the two busiest services |
| `auto-focus-event-bus` | Every producer and consumer of the Kafka bus (`auto focus … { direction in }`) |
| `auto-perspective-security`, `auto-perspective-performance` | Elements carrying a `perspective` note |
| `auto-slice-…publish…subscribe` | Only the asynchronous, event-driven relationships |
| `auto-layer-timelines`, `auto-layer-trust-safety` | One `group` and its neighbours |
| `production-deployment` | Containers placed on data-centre and cloud nodes |

Click an element to select it — the URL updates, so a selection is a link.
Press <kbd>space</kbd> to jump to a view and <kbd>?</kbd> for shortcuts.

The demo is the static viewer that [`export-viewer`](./cli/export.md#static-viewer)
produces, so the server-only tools (universe graph, clusters, review, diff,
print document) are not part of it. Run the workspace locally to get them:

```sh
structurizrx serve site/demo/twitter.dsl --open
```

## Source

<details>
<summary><code>site/demo/twitter.dsl</code></summary>

```text
{{#include ../demo/twitter.dsl}}
```

</details>
