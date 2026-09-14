/*
 * structurizr-selection.js — what is selected in a viewer, as the references
 * a link carries.
 *
 * A selection lives in the page URL (`#<view>&sel=<ref>,<ref>` on the
 * workspace page, `#sel=<ref>,…` on the diagram and graph pages), so the
 * address bar always links to exactly what is selected — a link that can be
 * handed to someone, or to an agent, which turns it into DSL locations with
 * `structurizrx locate`.
 *
 * References name things by canonical path (ancestor names then their own,
 * joined with `/`; deployment elements rooted at their environment), never by
 * id, because the DSL parser assigns ids in parse order and they shift
 * whenever the file is edited. The grammar here must stay in step with
 * structurizr-query/src/reference.rs, which is what resolves them.
 */
(function (global) {
    'use strict';

    function decode(s) {
        try { return decodeURIComponent(s); } catch (e) { return s; }
    }

    /** Split a location hash into its view (a leading part without `=`), its
     *  selection (`sel=`, decoded) and any other parts, kept verbatim. */
    function parseHash(hash) {
        var out = { view: null, selection: [], others: [] };
        (hash || '').replace(/^#/, '').split('&').forEach(function (part, i) {
            if (!part) return;
            if (part.indexOf('sel=') === 0) {
                part.slice(4).split(',').forEach(function (reference) {
                    if (reference) out.selection.push(decode(reference));
                });
            } else if (i === 0 && part.indexOf('=') === -1) {
                out.view = decode(part);
            } else {
                out.others.push(part);
            }
        });
        return out;
    }

    /** The inverse of parseHash. Each reference is encoded on its own, so `,`
     *  only ever separates references. */
    function buildHash(view, selection, others) {
        var parts = [];
        if (view) parts.push(encodeURIComponent(view));
        (others || []).forEach(function (part) { parts.push(part); });
        if (selection && selection.length) {
            parts.push('sel=' + selection.map(encodeURIComponent).join(','));
        }
        return parts.length ? '#' + parts.join('&') : '';
    }

    /** Replace the hash without adding a history entry or firing hashchange. */
    function replaceHash(hash) {
        history.replaceState(history.state, '', location.pathname + location.search + hash);
    }

    /** { path } | { from, to, description } | { view } | { decision } */
    function parseReference(reference) {
        var r = reference.trim();
        var lower = r.toLowerCase();
        if (lower.indexOf('view:') === 0) return { view: r.slice(5).trim() };
        if (lower.indexOf('decision:') === 0) return { decision: r.slice(9).trim() };
        var arrow = r.indexOf('->');
        if (arrow === -1) return { path: r };
        var from = r.slice(0, arrow).trim();
        var rest = r.slice(arrow + 2).trim();
        var to = rest, description = null;
        // `to "description"`: the last quoted string, if the reference ends in one.
        if (rest.charAt(rest.length - 1) === '"') {
            var inner = rest.slice(0, -1);
            var open = inner.lastIndexOf('"');
            if (open !== -1 && inner.slice(0, open).trim()) {
                to = inner.slice(0, open).trim();
                description = inner.slice(open + 1);
            }
        }
        return { from: from, to: to, description: description };
    }

    /**
     * Paths for a set of elements and relationships, in both directions.
     *   elements:      [{ id, name, parentId, environment }]
     *   relationships: [{ id, sourceId, destinationId, description }]
     * Ids are whatever the host uses (model ids, graph node ids); they only
     * have to agree between the two lists.
     */
    function ReferenceIndex(elements, relationships) {
        var self = this;
        this.byId = {};
        this.paths = {};
        this.idsByPath = {};
        this.relationships = relationships;
        this.parallel = {};
        elements.forEach(function (e) { self.byId[e.id] = e; });
        elements.forEach(function (e) {
            var key = self.path(e.id).toLowerCase();
            (self.idsByPath[key] = self.idsByPath[key] || []).push(e.id);
        });
        relationships.forEach(function (r) {
            var key = r.sourceId + '\n' + r.destinationId;
            self.parallel[key] = (self.parallel[key] || 0) + 1;
        });
    }

    ReferenceIndex.prototype.path = function (id) {
        if (this.paths[id] !== undefined) return this.paths[id];
        var element = this.byId[id];
        if (!element) return String(id);
        var parts = [], root = element, seen = {};
        for (var e = element; e && !seen[e.id]; e = this.byId[e.parentId]) {
            seen[e.id] = true;
            parts.unshift(e.name);
            root = e;
        }
        // Only deployment elements carry an environment.
        if (root.environment) parts.unshift(root.environment);
        return (this.paths[id] = parts.join('/'));
    };

    ReferenceIndex.prototype.elementReference = function (id) {
        return this.path(id);
    };

    ReferenceIndex.prototype.relationshipReference = function (r) {
        var reference = this.path(r.sourceId) + '->' + this.path(r.destinationId);
        // Parallel relationships are told apart by description, when that is
        // expressible: the grammar has no escape for a quote.
        var description = (r.description || '').trim();
        if (this.parallel[r.sourceId + '\n' + r.destinationId] > 1 &&
            description && description.indexOf('"') === -1) {
            reference += ' "' + description + '"';
        }
        return reference;
    };

    /** The element and relationship ids a reference names. Views, decisions
     *  and implied relationships resolve to nothing here; hosts that show
     *  views or decisions handle those themselves. */
    ReferenceIndex.prototype.resolve = function (reference) {
        var parsed = parseReference(reference), self = this;
        var out = { elements: [], relationships: [] };
        if (parsed.path !== undefined) {
            out.elements = (this.idsByPath[parsed.path.toLowerCase()] || []).slice();
        } else if (parsed.from !== undefined) {
            var from = parsed.from.toLowerCase(), to = parsed.to.toLowerCase();
            var description = parsed.description === null ? null : parsed.description.trim().toLowerCase();
            out.relationships = this.relationships.filter(function (r) {
                return self.path(r.sourceId).toLowerCase() === from &&
                    self.path(r.destinationId).toLowerCase() === to &&
                    (description === null || (r.description || '').trim().toLowerCase() === description);
            }).map(function (r) { return r.id; });
        }
        return out;
    };

    /**
     * Click-to-select on a structurizr.ui.Diagram: a click selects an element
     * or relationship, shift/⌘/ctrl-click adds or removes one, a click on the
     * background clears. `options.onChange(references)` runs after each change
     * the user makes (not after restore()).
     */
    function DiagramSelection(diagram, options) {
        var self = this;
        this.diagram = diagram;
        this.items = [];   // [{ type: 'element' | 'relationship', id }]
        this.onChange = (options && options.onChange) || function () {};

        diagram.onCellClicked(function (evt, hit) {
            var additive = evt && (evt.shiftKey || evt.metaKey || evt.ctrlKey);
            if (!hit) {
                if (additive || self.items.length === 0) return;
                self.items = [];
            } else {
                var item = hit.elementId !== undefined
                    ? { type: 'element', id: hit.elementId }
                    : { type: 'relationship', id: hit.relationshipId };
                var at = self.indexOf(item);
                if (!additive) {
                    self.items = [item];
                } else if (at === -1) {
                    self.items.push(item);
                } else {
                    self.items.splice(at, 1);
                }
            }
            self.apply();
            self.onChange(self.references());
        });
    }

    DiagramSelection.prototype.indexOf = function (item) {
        for (var i = 0; i < this.items.length; i++) {
            if (this.items[i].type === item.type && this.items[i].id === item.id) return i;
        }
        return -1;
    };

    /** The reference index for the current workspace, rebuilt when the page
     *  swaps in a new one (live reload). */
    DiagramSelection.prototype.index = function () {
        var workspace = global.structurizr.workspace;
        if (this._indexFor !== workspace) {
            this._index = new ReferenceIndex(
                workspace.getElements().map(function (e) {
                    return { id: e.id, name: e.name, parentId: e.parentId, environment: e.environment };
                }),
                workspace.getRelationships().map(function (r) {
                    return { id: r.id, sourceId: r.sourceId, destinationId: r.destinationId, description: r.description };
                }));
            this._indexFor = workspace;
        }
        return this._index;
    };

    DiagramSelection.prototype.references = function () {
        var index = this.index(), workspace = global.structurizr.workspace;
        return this.items.map(function (item) {
            if (item.type === 'element') return index.elementReference(item.id);
            var relationship = workspace.findRelationshipById(item.id);
            return relationship ? index.relationshipReference(relationship) : null;
        }).filter(Boolean);
    };

    /** Select what `references` name on the diagram currently drawn; anything
     *  it doesn't draw is dropped. Call after every changeView. */
    DiagramSelection.prototype.restore = function (references) {
        var index = this.index(), self = this;
        this.items = [];
        (references || []).forEach(function (reference) {
            var ids = index.resolve(reference);
            ids.elements.map(function (id) { return { type: 'element', id: id }; })
                .concat(ids.relationships.map(function (id) { return { type: 'relationship', id: id }; }))
                .forEach(function (item) {
                    if (self.indexOf(item) === -1) self.items.push(item);
                });
        });
        this.apply();
    };

    DiagramSelection.prototype.clear = function () {
        this.items = [];
        this.apply();
    };

    DiagramSelection.prototype.apply = function () {
        function idsOf(type) {
            return this.items.filter(function (i) { return i.type === type; })
                .map(function (i) { return i.id; });
        }
        var shown = this.diagram.setSelection(idsOf.call(this, 'element'), idsOf.call(this, 'relationship'));
        this.items = this.items.filter(function (i) {
            return (i.type === 'element' ? shown.elements : shown.relationships).indexOf(i.id) !== -1;
        });
    };

    /** Where `references` are declared, from the server's locate API:
     *  resolves to `{items, unresolved}` (the shape of `structurizrx locate
     *  --json`), or to null when there is nothing to ask or the request fails. */
    function locate(workspaceSlug, references) {
        if (!references || references.length === 0) return Promise.resolve(null);
        return fetch('/api/workspace/' + encodeURIComponent(workspaceSlug) + '/locate?' +
                buildHash(null, references).slice(1))
            .then(function (r) { return r.ok ? r.json() : null; })
            .catch(function () { return null; });
    }

    /** A link opening a location from the locate API in VS Code. */
    function editorLink(text, place) {
        var path = place.absolutePath.replace(/\\/g, '/');
        var link = document.createElement('a');
        link.href = 'vscode://file' + (path.charAt(0) === '/' ? '' : '/') + encodeURI(path) +
            ':' + place.line + ':' + place.col;
        link.textContent = text;
        link.title = place.file + ':' + place.line + ' — open in VS Code';
        return link;
    }

    global.StructurizrSelection = {
        locate: locate,
        editorLink: editorLink,
        parseHash: parseHash,
        buildHash: buildHash,
        replaceHash: replaceHash,
        parseReference: parseReference,
        ReferenceIndex: ReferenceIndex,
        DiagramSelection: DiagramSelection
    };
})(window);
