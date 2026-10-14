# Viewer UX plan

Started 2026-10-14 from a usability review of the web viewer (`structurizrx
serve`) on `site/demo/twitter.dsl`, read through Don Norman's (conceptual
model, signifiers, feedback, the gulfs of execution and evaluation) and Jakob
Nielsen's (the ten heuristics, discount usability testing) lenses.

## The idea the UI should make obvious

The model is the truth; diagrams, graph, clusters, review and diff are lenses
onto it. Every change below is judged by whether it makes that easier to see:
the same element looks the same in every lens, every lens is one click from
every other, and anything can be found by name.

## Who it is for, and what they are doing

| Activity | Who | What they need first |
|---|---|---|
| Understand a system | New team member, stakeholder | What is this, what are the main parts, how do they connect? |
| Find something | Engineer mid-task | Where is X, what talks to it, where is it declared? |
| Review a change | PR reviewer, architect | What did this change do to the architecture? |
| Assess health | Architect, tech lead | Is the structure we declare the structure we have? |
| Communicate | Anyone presenting | A clean diagram or document. |

A new control should serve one of these; if it serves none, it should not be
in the main UI.

## Phase 1 — quick fixes (done, `c400137`)

- Land on the first hand-authored view in C4 order, not the first generated one.
- Sidebar: own views first in C4 order, generated after; generated titles lose the prefix their group header repeats.
- View badge in words ("System landscape", "Focus"), not `SystemLandscape`.
- Review and Clusters in plain language: kinds as words, real plurals, "Include nested relationships" for "Roll up".
- Clusters: members declared elsewhere are information, not errors, so the one real finding stands out.
- `serve <file>` opens the workspace directly.

## Phase 2 — structure

### 2.1 Shared app bar (done, `4f0365d`)

One bar on every page: workspace name, tabs (Diagrams · Graph · Clusters ·
Review · Compare · Decisions), a More menu, the theme toggle. Page controls in
a toolbar row beneath. One theme setting for the whole viewer. Rendered by
`structurizr-web/src/chrome.rs`.

### 2.2 Element inspector (done, `654f00b`)

One panel for one element, wherever it is met: diagram selection, graph node,
cluster name, review pane. Kind, technology, parent, description, findings,
uses / used by, children, views; Open in editor, Copy reference, Graph,
Review. `assets/js/structurizr-inspector.js`.

### 2.3 Find anything: ⌘K (done)

Closes the gulf of execution: today, answering "what talks to the Event Bus?"
needs knowing that focus views or the graph exist.

- **Open**: ⌘K / Ctrl+K on every page, and a "Search" button in the app bar
  that shows the shortcut (recognition over recall). On the workspace page,
  `space` (the old quick navigation) opens it too.
- **Finds**: views (by the title the sidebar shows), model elements (name,
  then path, technology, description), decisions, and commands (go to a tab,
  print all views, toggle dark mode, documentation).
- **Ranking**: whole-name and prefix matches first, then word starts, then
  substrings; name beats path beats description. Results grouped by type,
  a few per group, ↑/↓/Enter/Esc.
- **Acting on an element** depends on the page, defaulting to the most
  useful place to see it:
  - Diagrams: select it if the current view draws it, else open the best view
    that does (own views before generated, in C4 order) with it selected —
    the inspector opens either way.
  - Graph: select the node. Review: jump to it. Clusters: open the inspector.
  - Elsewhere: the Diagrams tab, as above.
- **Data**: one server-built catalog, `GET /api/workspace/{name}/catalog`
  (elements with paths and their views in preference order, views with the
  titles the sidebar uses, decisions). `export-viewer` writes the same thing
  as `catalog.json`, so the palette works in a static export too.

### 2.4 Collapsible sidebar (done)

The view list has a toggle (button, or `[`). On a wide window it collapses in
place and the choice is remembered per browser; below 1000px it starts
closed and opens as a drawer over the diagram, closed again by picking a
view, Esc or a click on the diagram. Searching views is ⌘K's job.

Not covered: the review page's 24rem element list and the graph's controls
panel stay as they are on narrow windows.

### 2.5 View names (done, the light version)

Long titles are cut short with an ellipsis in the sidebar and on the view's
name, with the full text on hover. Views without a title get short names in
the generated views' style — "Social Network Platform — Production", not
"Deployment View: Social Network Platform - Production" — from
`getDefaultViewName`, mirrored in `src/catalog.rs`.

Not done, on purpose: the parser reads a view's third argument as its title,
where upstream Structurizr reads it as the description, so upstream-style
dynamic views still show a sentence as their name. Changing that changes
what `parse`/`emit` mean, and is a separate decision.

### 2.6 A calmer diagram page (done)

The workspace page's toolbar row is gone. The view's name floats at the
diagram's top left with a menu of that view's actions and display settings;
zoom floats at the bottom left and a dynamic view's steps at the bottom
centre. Dark mode moved from the app bar into the ⋯ menu on every page.

## Phase 3 — what makes it more than a viewer

- **Workspace home (done)**: `/workspace/{name}/overview`, the Overview tab and
  where `serve` and the index now land. Description and counts, the first
  view large, health (each finding type links into Review pre-filtered —
  Review keeps `only` and `q` in its hash now, and searches finding codes),
  recent changes (each commit links to its comparison) and every view as a
  thumbnail, generated ones folded away. Dynamic and deployment views show an
  icon: the server's SVG renderer can't draw them yet.
- **Findings-first Clusters**: lead with sentences and actions ("Legal
  Request Tool probably belongs in Data Platform — 4 of its 5 dependencies
  are there [show] [open in editor]"); metrics behind a disclosure.
- **Diff on the diagram**: a compare mode on any view — added green, removed
  ghosted, changed outlined — the picture a reviewer puts in a PR.
- **Readable large views (done, except semantic zoom)**: below 30% zoom the
  relationship labels and element descriptions are hidden (back on hover or
  selection; exports unaffected); a minimap appears once the diagram no
  longer fits and moves the view on click or drag; `z` and the inspector's
  *Zoom to* zoom to the selection, capped at 60% and centred clear of the
  inspector. Not done: semantic zoom (zooming into a system opens its
  container view) — it would change what zooming means, and double-click
  already drills in.
- **Outlined elements (done)**: a second look, chosen in the view menu —
  each element's colour as its border on a near-plain box.

## Validation

Five users, an hour each, before phase 3 is ordered:

1. "You've just joined. In two minutes, explain what this system does."
2. "Find what depends on the Event Bus."
3. "Something changed since the last commit. What?"
4. "Is anything modelled in the wrong place?"
5. "Get a diagram of the containers into a slide."

## Known issues found along the way

- `serve site/examples` finds 1 of the 7 workspaces in the directory.
- The inspector floats over the diagram and can cover elements on the right.
- Review shows finding codes only as tooltips now (the report still has them).
- The theme lives in a cookie; whether it survives in an `export-viewer`
  bundle opened from `file://` is unverified.
