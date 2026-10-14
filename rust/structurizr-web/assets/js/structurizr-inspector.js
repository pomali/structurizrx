/*
 * The element inspector: one panel showing one model element the same way
 * on every page — the diagrams, the universe graph, the cluster analysis and
 * the review walkthrough. Seeing the same object presented the same way in
 * each of them is what makes them read as views of one model rather than
 * separate tools.
 *
 * Data comes from /api/workspace/{name}/review (`structurizr_query::review`):
 * kind, technology, parent, description, findings, relationships both ways,
 * children and the views showing the element. That covers the static model
 * only; a host showing something else (a deployment node, say, or anything
 * in an export-viewer bundle, which has no server) passes `fallback`, and
 * StructurizrInspector.fromWorkspace() builds the same shape from the
 * workspace JSON, minus the findings.
 *
 * Needs structurizr-selection.js for references and "Open in editor".
 */
(function (global) {
    'use strict';

    var cache = {};

    /** The review data for a workspace, by element id. Cached per page:
     *  a live reload reloads the page. */
    function load(slug) {
        if (!cache[slug]) {
            cache[slug] = fetch('/api/workspace/' + encodeURIComponent(slug) + '/review')
                .then(function (r) { return r.ok ? r.json() : { elements: [] }; })
                .catch(function () { return { elements: [] }; })
                .then(function (review) {
                    var byId = {};
                    review.elements.forEach(function (e) { byId[e.id] = e; });
                    var index = new global.StructurizrSelection.ReferenceIndex(
                        review.elements.map(function (e) {
                            return { id: e.id, name: e.name, parentId: e.parentId };
                        }), []);
                    review.elements.forEach(function (e) { e.reference = index.elementReference(e.id); });
                    return { byId: byId, elements: review.elements };
                });
        }
        return cache[slug];
    }

    /* ---- small DOM helpers ---- */

    function el(tag, className, text) {
        var node = document.createElement(tag);
        if (className) node.className = className;
        if (text !== undefined && text !== null) node.textContent = text;
        return node;
    }

    function icon(name) {
        return el('i', 'bi ' + name);
    }

    /** "softwareSystem" → "software system" */
    function kindLabel(kind) {
        return String(kind || '').replace(/([a-z])([A-Z])/g, '$1 $2').toLowerCase();
    }

    function section(title, count, body) {
        var wrap = el('section', 'sx-insp-section');
        var heading = el('h3', 'sx-insp-heading', title);
        if (count !== undefined) heading.appendChild(el('span', 'sx-insp-count', String(count)));
        wrap.appendChild(heading);
        wrap.appendChild(body);
        return wrap;
    }

    /** A link to another element: shown in the inspector by default, or
     *  wherever the host sends it. */
    function elementLink(id, name, ctx) {
        var a = el('a', 'sx-insp-element', name);
        a.href = '#';
        a.addEventListener('click', function (event) {
            event.preventDefault();
            ctx.onElement(id);
        });
        return a;
    }

    /** Relationships listed beyond this are folded behind "Show all". */
    var FOLD_AFTER = 6;

    function relationshipList(relationships, arrow, ctx) {
        var list = el('ul', 'sx-insp-rels');
        relationships.forEach(function (r, i) {
            var item = el('li');
            if (i >= FOLD_AFTER) item.hidden = true;
            var line = el('div', 'sx-insp-rel-line');
            line.appendChild(el('span', 'sx-insp-arrow', arrow));
            line.appendChild(ctx.known(r.otherId)
                ? elementLink(r.otherId, r.otherName, ctx)
                : el('span', 'sx-insp-element', r.otherName));
            item.appendChild(line);
            var meta = [];
            if (r.description) meta.push(r.description);
            if (r.technology) meta.push('[' + r.technology + ']');
            if (r.kind) meta.push('(' + r.kind + ')');
            item.appendChild(el('div', 'sx-insp-rel-meta', meta.length ? meta.join(' ') : 'No description'));
            if (!meta.length) item.lastChild.classList.add('is-missing');
            list.appendChild(item);
        });
        if (relationships.length > FOLD_AFTER) {
            var more = el('li', 'sx-insp-more');
            var button = el('button', 'btn btn-link btn-sm p-0',
                'Show all ' + relationships.length);
            button.type = 'button';
            button.addEventListener('click', function () {
                list.querySelectorAll('li[hidden]').forEach(function (li) { li.hidden = false; });
                more.remove();
            });
            more.appendChild(button);
            list.appendChild(more);
        }
        return list;
    }

    function actionButton(spec) {
        var node = el(spec.href ? 'a' : 'button', 'btn btn-sm btn-outline-secondary');
        if (spec.href) node.href = spec.href; else node.type = 'button';
        if (spec.title) node.title = spec.title;
        node.appendChild(icon(spec.icon));
        node.appendChild(document.createTextNode(' ' + spec.label));
        if (spec.onClick) {
            node.addEventListener('click', function (event) { spec.onClick(event, node); });
        }
        return node;
    }

    function copyReference(reference) {
        return function (event, button) {
            var done = function (label) {
                var original = button.lastChild.textContent;
                button.lastChild.textContent = ' ' + label;
                setTimeout(function () { button.lastChild.textContent = original; }, 1500);
            };
            if (navigator.clipboard) {
                navigator.clipboard.writeText(reference).then(
                    function () { done('Copied'); },
                    function () { done('Could not copy'); });
            } else {
                done('Could not copy');
            }
        };
    }

    /**
     * The inspector's content for one element, as a DOM node.
     *
     * ctx:
     *   slug       workspace slug (server pages) or null (static export)
     *   page       the host page: 'diagrams' | 'graph' | 'clusters' | 'review';
     *              the action that would lead back to it is left out
     *   known(id)  whether `id` can be shown in the inspector
     *   onElement(id)  a neighbour, parent or child was clicked
     *   viewLink(view, element) → {href, onClick?} for an "Appears in" entry
     *   actions    extra actions for this host: [{icon, label, title?, href? | onClick?}]
     *   onClose    shows a close button when given
     *   controls   a node to place under the title (Review's reviewed/flagged)
     */
    function render(element, ctx) {
        var root = el('div', 'sx-insp');

        var head = el('div', 'sx-insp-head');
        var titles = el('div', 'sx-insp-titles');
        titles.appendChild(el('div', 'sx-insp-kind', kindLabel(element.kind)));
        titles.appendChild(el('h2', 'sx-insp-name', element.name));
        var sub = el('div', 'sx-insp-sub');
        var bits = [];
        if (element.technology) bits.push(el('span', null, element.technology));
        if (element.parentId) {
            var parent = el('span', null, 'in ');
            parent.appendChild(ctx.known(element.parentId)
                ? elementLink(element.parentId, element.parentName, ctx)
                : document.createTextNode(element.parentName || ''));
            bits.push(parent);
        }
        if (element.group) bits.push(el('span', null, 'group ' + element.group));
        bits.forEach(function (bit, i) {
            if (i > 0) sub.appendChild(document.createTextNode(' · '));
            sub.appendChild(bit);
        });
        if (bits.length) titles.appendChild(sub);
        head.appendChild(titles);
        if (ctx.onClose) {
            var close = el('button', 'btn-close sx-insp-close');
            close.type = 'button';
            close.title = 'Close (Esc)';
            close.setAttribute('aria-label', 'Close');
            close.addEventListener('click', ctx.onClose);
            head.appendChild(close);
        }
        root.appendChild(head);

        if (ctx.controls) root.appendChild(ctx.controls);

        root.appendChild(element.description
            ? el('p', 'sx-insp-desc', element.description)
            : el('p', 'sx-insp-desc is-missing', 'No description.'));

        root.appendChild(actions(element, ctx));

        if (element.findings && element.findings.length) {
            var findings = el('ul', 'sx-insp-findings');
            element.findings.forEach(function (f) {
                var item = el('li', f.blocking ? 'is-blocking' : '');
                item.appendChild(icon(f.blocking ? 'bi-x-octagon' : 'bi-exclamation-triangle'));
                item.appendChild(el('span', null, f.message));
                item.title = f.code + (f.blocking ? ' — fails validate --strict' : '');
                findings.appendChild(item);
            });
            root.appendChild(section('Findings', element.findings.length, findings));
        } else if (ctx.page === 'review') {
            // Reviewing, "nothing to see" is itself the answer.
            root.appendChild(section('Findings', 0, el('p', 'sx-insp-empty', 'Nothing flagged.')));
        }

        var outgoing = element.outgoing || [], incoming = element.incoming || [];
        if (outgoing.length) root.appendChild(section('Uses', outgoing.length, relationshipList(outgoing, '→', ctx)));
        if (incoming.length) root.appendChild(section('Used by', incoming.length, relationshipList(incoming, '←', ctx)));
        if (!outgoing.length && !incoming.length) {
            root.appendChild(section('Relationships', 0, el('p', 'sx-insp-empty', 'None.')));
        }

        var children = element.childElements || [];
        if (children.length) {
            var list = el('ul', 'sx-insp-plain');
            children.forEach(function (child) {
                var item = el('li');
                item.appendChild(ctx.known(child.id) ? elementLink(child.id, child.name, ctx) : el('span', null, child.name));
                list.appendChild(item);
            });
            root.appendChild(section('Contains', children.length, list));
        }

        var views = element.views || [];
        var viewBody;
        if (views.length) {
            viewBody = el('ul', 'sx-insp-plain');
            views.forEach(function (v) {
                var item = el('li');
                var target = ctx.viewLink(v, element);
                var link = el('a', null, v.name);
                link.href = target.href;
                if (target.onClick) link.addEventListener('click', target.onClick);
                item.appendChild(link);
                item.appendChild(el('span', 'sx-insp-view-kind', ' ' + v.kind));
                viewBody.appendChild(item);
            });
        } else {
            viewBody = el('p', 'sx-insp-empty', 'Not shown in any view.');
        }
        root.appendChild(section('Appears in', views.length, viewBody));

        if (element.tags && element.tags.length) {
            var tags = el('div', 'sx-insp-tags');
            element.tags.forEach(function (t) { tags.appendChild(el('span', 'badge text-bg-light border', t)); });
            root.appendChild(section('Tags', undefined, tags));
        }

        return root;
    }

    function actions(element, ctx) {
        var bar = el('div', 'sx-insp-actions');
        var reference = element.reference;
        var base = ctx.slug ? '/workspace/' + encodeURIComponent(ctx.slug) : null;

        if (base && reference) {
            // Placeholder until the server says where the element is declared.
            var editor = actionButton({ icon: 'bi-code-slash', label: 'Open in editor', title: 'Locating…' });
            editor.disabled = true;
            bar.appendChild(editor);
            global.StructurizrSelection.locate(ctx.slug, [reference]).then(function (located) {
                var item = located && located.items[0];
                var place = item && item.locations && item.locations[0];
                if (!place || !place.absolutePath) { editor.remove(); return; }
                var link = global.StructurizrSelection.editorLink('', place);
                link.className = 'btn btn-sm btn-outline-secondary';
                link.appendChild(icon('bi-code-slash'));
                link.appendChild(document.createTextNode(' Open in editor'));
                editor.replaceWith(link);
            });
        }

        if (reference) {
            bar.appendChild(actionButton({
                icon: 'bi-link-45deg', label: 'Copy reference',
                title: 'Copy "' + reference + '" — structurizrx locate and query understand it',
                onClick: copyReference(reference)
            }));
        }

        if (base && reference && ctx.page !== 'graph') {
            bar.appendChild(actionButton({
                icon: 'bi-diagram-3', label: 'Graph', title: 'Show in the universe graph',
                href: base + '/graph' + global.StructurizrSelection.buildHash(null, [reference])
            }));
        }
        if (base && element.inReview && ctx.page !== 'review') {
            bar.appendChild(actionButton({
                icon: 'bi-list-check', label: 'Review', title: 'Open in the review walkthrough',
                href: base + '/review#e=' + encodeURIComponent(element.id)
            }));
        }

        (ctx.actions || []).forEach(function (spec) { bar.appendChild(actionButton(spec)); });
        return bar;
    }

    /** Fill in what the review API leaves as ids: child names, and whether
     *  the element is part of the review data at all. */
    function enrich(element, byId) {
        element.inReview = true;
        element.childElements = (element.children || []).map(function (id) {
            return { id: id, name: byId[id] ? byId[id].name : id };
        });
        return element;
    }

    /**
     * The inspector's data for `id`, built from the workspace JSON (a
     * structurizr.Workspace) for what the review API does not cover:
     * deployment elements, and every element of a static export. No findings.
     */
    function fromWorkspace(workspace, id, reference) {
        var e = workspace.findElementById(id);
        if (!e) return null;
        var name = function (otherId) {
            var other = workspace.findElementById(otherId);
            return other ? other.name : otherId;
        };
        var parent = e.parentId ? workspace.findElementById(e.parentId) : null;
        var tags = (e.tags || '').split(',').map(function (t) { return t.trim(); }).filter(Boolean);
        var rels = workspace.getRelationships();
        var side = function (mine, theirs) {
            return rels.filter(function (r) { return r[mine] === id; }).map(function (r) {
                return { id: r.id, otherId: r[theirs], otherName: name(r[theirs]),
                         description: r.description, technology: r.technology };
            });
        };
        var children = workspace.getElements().filter(function (c) { return c.parentId === id; });
        var views = workspace.getViews().filter(function (v) {
            return (v.elements || []).some(function (ev) { return ev.id === id; });
        }).map(function (v) {
            return { key: v.key, name: global.structurizr.ui.getTitleForView(v),
                     kind: kindLabel(v.type) };
        });
        return {
            id: id,
            name: e.name,
            kind: e.type ? e.type.charAt(0).toLowerCase() + e.type.slice(1) : 'element',
            description: e.description,
            technology: e.technology,
            group: e.group,
            tags: tags,
            parentId: parent ? parent.id : null,
            parentName: parent ? parent.name : null,
            childElements: children.map(function (c) { return { id: c.id, name: c.name }; }),
            outgoing: side('sourceId', 'destinationId'),
            incoming: side('destinationId', 'sourceId'),
            views: views,
            findings: [],
            reference: reference,
            inReview: false
        };
    }

    /**
     * An inspector panel floating over `host` (which must be positioned).
     *
     * options: everything render()'s ctx takes except known/onClose, plus
     *   fallback(id)  element data when the review API has none for `id`
     *   onClose()     after the user closes the panel
     */
    function Panel(host, options) {
        this.options = options;
        this.node = el('aside', 'sx-inspector');
        this.node.hidden = true;
        this.node.setAttribute('aria-label', 'Element details');
        host.appendChild(this.node);
        this.currentId = null;
        this.request = 0;
    }

    Panel.prototype.show = function (id) {
        var self = this, request = ++this.request, options = this.options;
        this.currentId = id;
        var data = options.slug ? load(options.slug) : Promise.resolve({ byId: {} });
        return data.then(function (data) {
            if (request !== self.request) return;
            var element = data.byId[id] ? enrich(data.byId[id], data.byId)
                : (options.fallback ? options.fallback(id) : null);
            if (!element) { self.hide(); return; }
            var ctx = {
                slug: options.slug,
                page: options.page,
                actions: options.actions ? options.actions(element) : [],
                viewLink: options.viewLink || function (v) {
                    return { href: '/workspace/' + encodeURIComponent(options.slug) + '#' + encodeURIComponent(v.key) };
                },
                known: function (otherId) {
                    return !!data.byId[otherId] || !!(options.fallback && options.fallback(otherId));
                },
                onElement: options.onElement || function (otherId) { self.show(otherId); },
                onClose: function () {
                    self.hide();
                    if (options.onClose) options.onClose();
                }
            };
            self.node.replaceChildren(render(element, ctx));
            self.node.hidden = false;
            self.node.scrollTop = 0;
        });
    };

    Panel.prototype.hide = function () {
        this.request++;
        this.currentId = null;
        this.node.hidden = true;
    };

    Panel.prototype.isOpen = function () {
        return !this.node.hidden;
    };

    global.StructurizrInspector = {
        load: load,
        render: render,
        enrich: enrich,
        fromWorkspace: fromWorkspace,
        kindLabel: kindLabel,
        Panel: Panel
    };
})(window);
