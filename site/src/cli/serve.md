# structurizrx serve

Serve a workspace, or a directory of workspaces, in a local web browser with
live reload.

```sh
structurizrx serve [path] [--port <n>] [--open]
```

| Argument/Flag | Default | Effect |
|---|---|---|
| `path` | `.` | A `.dsl`/`.json` file, or a directory containing one or more workspaces |
| `--port`, `-p` | `3000` | TCP port to listen on |
| `--open` | off | Open the browser automatically after starting |

Like `render`, `serve` materializes generated (`auto`) views before
rendering. Editing and saving a watched file reloads the browser via a
WebSocket automatically — no manual refresh.

Serving a single file opens that workspace's overview directly; serving a directory
opens the list of workspaces in it.

## Finding your way

Every page has the same bar along the top: the workspace's name, then a tab
for each way of looking at it.

| Tab | What it shows |
|---|---|
| **Overview** | The workspace's home: what it is, what the model holds, its health (each problem links to the elements behind it in Review), recent changes (each links to the comparison) and every view as a thumbnail |
| **Diagrams** | The workspace's views, listed in the sidebar — your own views first, in C4 order, then generated ones. The button at the diagram's top left (or <kbd>[</kbd>) hides the list; on a narrow window it opens over the diagram instead |
| **Graph** | The whole workspace as one force-directed graph |
| **Clusters** | Communities, cycles and coupling, compared with the structure the model declares |
| **Review** | The model element by element, with hygiene findings |
| **Compare** | What changed in the model between two git versions |
| **Decisions** | The workspace's ADRs (only when it has some) |

**Find** (<kbd>⌘K</kbd>, or <kbd>Ctrl</kbd>+<kbd>K</kbd>; <kbd>space</kbd> on
the diagrams page) searches views, elements, decisions and commands from any
page. Picking an element shows it where you are — selected on the diagram,
the graph or in review — or, if the current page can't, opens the view that
shows it best with it selected. The **⋯** menu holds *Print all views*, the
canvas demo, this documentation and *Dark mode*, which applies to every page
at once.

On the diagrams page the view's name floats at the diagram's top left; click
it for that view's actions (open as SVG, copy as Mermaid, print, full screen)
and the display settings below. Zoom sits at the bottom left, and a dynamic
view's step buttons at the bottom centre.

## Routes

| Route | What it is |
|---|---|
| `GET /` | Workspace list |
| `GET /workspace/{name}/overview` | Workspace home: description, counts, health, recent changes, views |
| `GET /workspace/{name}` | The diagrams (`#<view>&sel=<references>` picks the view and selection) |
| `GET /workspace/{name}/diagram/{key}` | A single diagram |
| `GET /workspace/{name}/graph` | Universe graph — the whole workspace as one force-directed graph |
| `GET /workspace/{name}/clusters` | Cluster analysis |
| `GET /workspace/{name}/review` | Element-by-element review |
| `GET /workspace/{name}/diff` | Compare two git versions of the workspace |
| `GET /workspace/{name}/decisions` | ADR list (from `!adrs`) |
| `GET /workspace/{name}/decisions/{id}` | A single ADR |
| `GET /workspace/{name}/print` | Every view as one printable document |
| `GET /workspace/{name}/canvas` | In-browser WASM rendering demo |
| `GET /docs/` | This documentation site |
| `GET /llms.txt` | The DSL cheat sheet, as plain text |

## Selecting and linking

On the workspace, diagram and universe graph pages, a click selects an
element or relationship; shift-click (or ⌘/ctrl-click) adds to or removes
from the selection, and a click on the background — or <kbd>Esc</kbd> —
clears it.

The selection is kept in the page URL, so the address bar always links to
exactly what is selected: `#<view>&sel=<references>` on the workspace page,
`#sel=<references>` on the others. Reloading the page, or the workspace
reloading after an edit, keeps the selection. References name things by path
(`Shop/API`, `Shop/API->Shop/Database`) rather than by id, so a link keeps
working while the DSL changes — and [`locate`](./locate.md) turns one into the
file and line of each selected statement. The viewer does that lookup
itself, too: the status bar (and the graph's details panel) links each
selected item to its declaration, opening it in VS Code.

`locate` from the command line:

```sh
structurizrx locate shop.dsl 'http://localhost:3000/workspace/shop#containers&sel=Shop%2FAPI'
```

## Element details

Selecting one element opens its details in a panel beside the diagram:
its kind, technology and parent, its description, any review findings, what
it uses and what uses it, what it contains, and the views it appears in.
The same panel appears wherever you meet an element — a node in the graph, a
name on the clusters page, the review walkthrough — so an element always
reads the same way.

Following a link in the panel moves to that element (and selects it on the
diagram if it is drawn there); an *Appears in* entry opens that view with the
element selected. *Open in editor* jumps to its declaration in VS Code, and
*Copy reference* copies its path for `locate`, `query` or an agent.
<kbd>Esc</kbd> closes the panel.

## JSON API

The same server exposes a JSON API mirroring the CLI, useful for agents
working against a live server:

| Route | Equivalent to |
|---|---|
| `GET /api/workspaces` | workspace list with counts |
| `GET /api/workspace/{name}` | the full workspace JSON (`export`) |
| `GET /api/workspace/{name}/decisions[/{id}]` | ADR data |
| `GET /api/workspace/{name}/diagram/{key}/svg` | `render --format svg`, one diagram |
| `GET /api/workspace/{name}/diagram/{key}/mermaid` | `render --format mermaid`, one diagram |
| `GET /api/workspace/{name}/graph` | the universe graph as `{nodes, links}` JSON |
| `GET /api/workspace/{name}/catalog` | what Find searches: elements with paths and views, views with titles, decisions |
| `GET /api/workspace/{name}/revisions` | the workspace file's git history |
| `GET /api/workspace/{name}/diff?from=&to=` | the model-level comparison of two versions |
| `GET /api/workspace/{name}/digest` | [`digest`](./digest.md) |
| `GET /api/workspace/{name}/query?expr=...` | [`query`](./query.md) |
| `GET /api/workspace/{name}/locate?sel=...` | [`locate --json`](./locate.md) |

## Universe graph

`GET /workspace/{name}/graph` shows the entire workspace as one
force-directed graph, in the spirit of Obsidian's graph view: every element,
view, ADR and documentation section is a node, and every relationship,
containment, deployment instance and view membership is a link. It is the
one page that shows how everything in a workspace hangs together, rather
than one C4 scope at a time.

| Interaction | Effect |
|---|---|
| hover | Highlights the node and its immediate neighbours, dimming the rest |
| click | Opens the details panel (kind, description, technology, tags, connections) |
| double-click | Focuses the node's neighbourhood — a local graph, depth adjustable 1–5 |
| drag a node | Pins it where you drop it; click it without moving to release |
| drag / scroll | Pan and zoom; `f` zooms to fit, `Esc` clears focus and selection |
| `/` | Jumps to the search box; matches stay bright while everything else dims |

The **Nodes** and **Links** checkboxes filter what the graph contains.
Views, decisions and documentation sections start switched off — they belong
to the workspace, but they crowd out the model on first sight. The **Forces**
sliders (repel, link distance, link force, centre, node size) tune the
layout live.

Nodes are coloured by kind and sized by how many connections they have, so
the hubs of a model are visible before you read a single label.

## Diagram display

Two display settings are in the view-name menu on the diagrams page, and a
third in the **⋯** menu. Each is remembered by your browser, and the
single-diagram and print pages follow them too.

| Setting | Effect |
|---|---|
| **Layout** | How views with `autoLayout` are arranged. *ELK, right-angle lines* (the default) and *ELK, curved lines* use the [Eclipse Layout Kernel](https://eclipse.dev/elk/), which routes relationships around boxes and keeps groups and boundaries together; *dagre* is the layout upstream Structurizr uses. The view's direction and separations apply to all three. Views with stored positions are never re-arranged. |
| **Ignore DSL colours** | Draws with the built-in palette instead of the colours the workspace's styles and themes set. Shapes, icons, sizes and borders still come from the workspace. |
| **Dark mode** (in the **⋯** menu) | Switches every page, and the diagrams, to a dark background. |

With the built-in palette, whether because the workspace defines no styles
or because *Ignore DSL colours* is on, software systems other than the one a
view is about are drawn grey, as in `render`'s SVG output.

## Printing a diagram

*Print this view* in the view-name menu (or `p`) opens a *Print
this view* dialog: choose A4 or A3, and landscape, portrait or the
orientation that matches the diagram's shape. A plain `Ctrl`/`⌘`+`P` works
too, using the last paper size and the matching orientation.

Rather than printing the on-screen pan/zoom viewport (which would crop the
diagram to whatever is visible), the current view is exported to a
standalone SVG, scaled to fit one page, and captioned with the view title,
description, workspace name and date. Diagrams viewed in dark mode are
printed with a light background.

This prints one view. To print every view as a paginated document, use
*Print all views* in the app bar's **⋯** menu (`GET /workspace/{name}/print`).

The paper size is set via CSS (`@page { size: ... }`); in browsers that
don't support it, pick the paper size in the print dialog instead — the
diagram still fills whatever page it lands on.

## Compare versions

`GET /workspace/{name}/diff` compares two versions of the workspace out of
git and reports what changed **in the model**, not in the text of the file:
elements, relationships, views and decisions that were added, removed or
modified, field by field.

Pick the two versions in the header. Besides the commits, both pickers offer
*Working copy* — the file as it is on disk, uncommitted edits included — and
the page opens on the comparison you usually want: the last commit against
your working copy.

The comparison is a link. The chosen pair lives in the query string
(`?from=<sha>&to=<sha|working>`), so a comparison worth discussing can be
pasted into a review or an issue.

| It reports | Example |
|---|---|
| Elements | `+ Internet Banking/Cache (container)` |
| Relationships | `~ API → Database`, `description: "Reads from" → "Writes to"` |
| Views | which elements a view now shows, or no longer shows |
| Decisions | a new ADR, a status change, an edited body |
| Renames and moves | `Internet Banking/API → Internet Banking/API Gateway (probably renamed)` |

### Why not `git diff`

`git diff` answers a different question well. Moving a container between two
software systems is a two-hunk textual change that says nothing about the
architecture, while adding one relationship can be a one-word edit with a
large architectural meaning. Reformatting the file, renaming a DSL
identifier or reordering declarations are large text diffs and no model
change at all — this page reports nothing for them.

Elements are matched by their **path** (`System/Container/Component`), never
by id: the DSL assigns ids in parse order, so the same container has a
different id in two revisions as soon as anything is declared before it. One
consequence is worth knowing: a rename is reported as a removal plus an
addition, because from the model's point of view that is indistinguishable
from a replacement. Likely pairs are listed under *Renames and moves* — and
only when one candidate is unambiguous, so a container split into three
suggests nothing rather than the wrong thing.

Multi-file workspaces are handled: the version list covers commits that
touched any `!include`d file, and each version's includes are read from that
same revision, so you never see one commit's entry file spliced with
another's parts.

A workspace outside a repository, or one that has never been committed, says
so instead of offering the comparison.
