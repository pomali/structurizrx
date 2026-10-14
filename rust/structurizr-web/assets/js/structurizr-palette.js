/*
 * Find anything (⌘K / Ctrl+K, or the app bar's Find button): views, model
 * elements, decisions and commands, from any page.
 *
 * It sets itself up from the app bar (src/chrome.rs): `data-workspace` names
 * the workspace whose catalog (/api/workspace/{name}/catalog, see
 * src/catalog.rs) it searches; an export-viewer bundle's bar carries
 * `data-catalog="catalog.json"` instead. A page that can do better than
 * navigating — select an element in place, switch view without a reload —
 * says so with StructurizrPalette.configure({ onView, onElement, ... }).
 */
(function (global) {
    'use strict';

    var isMac = /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);
    var PER_GROUP = 6;

    var state = {
        slug: null,
        catalogUrl: null,
        isStatic: false,
        catalog: null,
        loading: null,
        handlers: {},
        root: null,
        input: null,
        list: null,
        items: [],
        active: 0,
        open: false,
        lastFocus: null
    };

    /* ---- data ---- */

    function load() {
        if (!state.loading) {
            state.loading = fetch(state.catalogUrl)
                .then(function (r) { return r.ok ? r.json() : null; })
                .catch(function () { return null; })
                .then(function (catalog) {
                    state.catalog = catalog || { elements: [], views: [], decisions: [] };
                    return state.catalog;
                });
        }
        return state.loading;
    }

    function base() {
        return '/workspace/' + encodeURIComponent(state.slug);
    }

    var KIND_LABELS = {
        SystemLandscape: 'System landscape', SystemContext: 'System context',
        Container: 'Container view', Component: 'Component view', Dynamic: 'Dynamic',
        Deployment: 'Deployment', Filtered: 'Filtered', Image: 'Image', Custom: 'Custom'
    };

    function words(kind) {
        return String(kind || '').replace(/([a-z])([A-Z])/g, '$1 $2').toLowerCase();
    }

    function commands() {
        var list = [];
        var go = function (label, icon, path, keywords) {
            list.push({
                type: 'command', label: label, icon: icon, keywords: keywords || '',
                run: function () { location.href = base() + path; }
            });
        };
        if (!state.isStatic) {
            go('Diagrams', 'bi-diagram-2', '', 'views workspace');
            go('Universe graph', 'bi-diagram-3', '/graph', 'graph force');
            go('Cluster analysis', 'bi-grid-3x3', '/clusters', 'clusters communities coupling');
            go('Review elements', 'bi-list-check', '/review', 'review findings walkthrough');
            go('Compare versions', 'bi-clock-history', '/diff', 'diff compare git history changes');
            if (state.catalog && state.catalog.decisions.length) {
                go('Decisions', 'bi-journal-text', '/decisions', 'adr decisions');
            }
            go('Print all views', 'bi-printer', '/print', 'print document pdf');
        }
        list.push({
            type: 'command', label: 'Toggle dark mode', icon: 'bi-circle-half', keywords: 'theme dark light',
            run: function () {
                if (global.StructurizrApp) global.StructurizrApp.setDarkMode(!global.StructurizrApp.isDarkMode());
            }
        });
        if (!state.isStatic) {
            list.push({
                type: 'command', label: 'Documentation', icon: 'bi-book', keywords: 'docs help manual',
                run: function () { location.href = '/docs/'; }
            });
        }
        return list;
    }

    /* ---- matching ---- */

    /** How well `query` matches `item`: 0 for no match. The name counts most,
     *  a prefix or word start more than a substring; every word of the query
     *  must appear somewhere. */
    function score(item, query, tokens) {
        var name = item.label.toLowerCase();
        var rest = (item.secondary + ' ' + item.keywords).toLowerCase();
        var hay = name + ' ' + rest;
        for (var i = 0; i < tokens.length; i++) {
            if (hay.indexOf(tokens[i]) === -1) return 0;
        }
        if (name === query) return 1000;
        if (name.indexOf(query) === 0) return 800 - name.length;
        var at = name.indexOf(query);
        if (at > 0 && /[\s\-_/.(]/.test(name.charAt(at - 1))) return 600 - name.length;
        if (at > 0) return 400 - name.length;
        var inName = tokens.filter(function (t) { return name.indexOf(t) !== -1; }).length;
        return 100 + inName * 20 - name.length / 100;
    }

    function candidates() {
        var c = state.catalog, out = [];
        c.views.forEach(function (v) {
            out.push({
                type: 'view', label: v.title, icon: v.generated ? 'bi-stars' : 'bi-easel',
                // Generated views are all landscape-shaped underneath; their
                // title ("focus: …") already says what they are.
                secondary: v.generated ? 'Generated view' : (KIND_LABELS[v.kind] || v.kind),
                keywords: (v.description || '') + ' ' + v.key, data: v
            });
        });
        c.elements.forEach(function (e) {
            var where = e.path.indexOf('/') === -1 ? '' : e.path.slice(0, e.path.lastIndexOf('/'));
            out.push({
                type: 'element', label: e.name, icon: 'bi-box',
                secondary: words(e.kind) + (e.technology ? ' · ' + e.technology : '') + (where ? ' · in ' + where : ''),
                keywords: (e.description || '') + ' ' + e.path, data: e
            });
        });
        c.decisions.forEach(function (d) {
            out.push({
                type: 'decision', label: d.title, icon: 'bi-journal-text',
                secondary: 'ADR ' + d.id + ' · ' + d.status, keywords: 'adr ' + d.id, data: d
            });
        });
        commands().forEach(function (cmd) {
            cmd.secondary = 'Command';
            out.push(cmd);
        });
        return out;
    }

    var GROUPS = [
        ['view', 'Views'], ['element', 'Elements'], ['decision', 'Decisions'], ['command', 'Commands']
    ];

    function search(query) {
        var all = candidates();
        query = query.trim().toLowerCase();
        if (!query) {
            // Nothing typed: where to go, and what to do.
            return all.filter(function (i) { return i.type === 'view'; }).slice(0, 8)
                .concat(all.filter(function (i) { return i.type === 'command'; }));
        }
        var tokens = query.split(/\s+/);
        var scored = all.map(function (item) { return { item: item, score: score(item, query, tokens) }; })
            .filter(function (s) { return s.score > 0; });
        var out = [];
        GROUPS.forEach(function (g) {
            scored.filter(function (s) { return s.item.type === g[0]; })
                .sort(function (a, b) { return b.score - a.score; })
                .slice(0, PER_GROUP)
                .forEach(function (s) { out.push(s.item); });
        });
        return out;
    }

    /* ---- acting ---- */

    function activate(item) {
        close(false);
        var h = state.handlers;
        if (item.type === 'command') {
            item.run();
        } else if (item.type === 'view') {
            if (!(h.onView && h.onView(item.data.key))) {
                location.href = (state.isStatic ? '' : base()) + '#' + encodeURIComponent(item.data.key);
            }
        } else if (item.type === 'element') {
            if (!(h.onElement && h.onElement(item.data))) showElement(item.data);
        } else if (item.type === 'decision') {
            location.href = base() + '/decisions/' + encodeURIComponent(item.data.id);
        }
    }

    /** The default for an element: the best view showing it, with it
     *  selected (the inspector opens on it); review if no view does. */
    function showElement(element) {
        var prefix = state.isStatic ? '' : base();
        if (element.views.length) {
            location.href = prefix + '#' + encodeURIComponent(element.views[0]) +
                '&sel=' + encodeURIComponent(element.path);
        } else if (!state.isStatic) {
            location.href = base() + '/review#e=' + encodeURIComponent(element.id);
        }
    }

    /* ---- UI ---- */

    function el(tag, className, text) {
        var node = document.createElement(tag);
        if (className) node.className = className;
        if (text !== undefined) node.textContent = text;
        return node;
    }

    /** `text` with the first occurrence of `query` in bold. */
    function highlighted(text, query) {
        var span = el('span', 'sx-pal-label');
        var at = query ? text.toLowerCase().indexOf(query) : -1;
        if (at === -1) { span.textContent = text; return span; }
        span.appendChild(document.createTextNode(text.slice(0, at)));
        span.appendChild(el('mark', null, text.slice(at, at + query.length)));
        span.appendChild(document.createTextNode(text.slice(at + query.length)));
        return span;
    }

    function build() {
        var root = el('div', 'sx-palette-backdrop');
        root.hidden = true;
        var dialog = el('div', 'sx-palette');
        dialog.setAttribute('role', 'dialog');
        dialog.setAttribute('aria-label', 'Find');

        var field = el('div', 'sx-pal-field');
        var magnifier = el('i', 'bi bi-search');
        field.appendChild(magnifier);
        var input = el('input');
        input.type = 'text';
        input.placeholder = 'Find a view, element, decision or command…';
        input.setAttribute('aria-label', 'Find');
        input.setAttribute('role', 'combobox');
        input.setAttribute('aria-controls', 'sx-pal-list');
        input.setAttribute('aria-expanded', 'true');
        input.autocomplete = 'off';
        input.spellcheck = false;
        field.appendChild(input);
        dialog.appendChild(field);

        var list = el('ul', 'sx-pal-list');
        list.id = 'sx-pal-list';
        list.setAttribute('role', 'listbox');
        dialog.appendChild(list);

        var foot = el('div', 'sx-pal-foot');
        foot.innerHTML = '<span><kbd>↑</kbd><kbd>↓</kbd> move</span><span><kbd>Enter</kbd> open</span>' +
            '<span><kbd>Esc</kbd> close</span>';
        dialog.appendChild(foot);
        root.appendChild(dialog);
        document.body.appendChild(root);

        root.addEventListener('mousedown', function (e) { if (e.target === root) close(true); });
        input.addEventListener('input', render);
        input.addEventListener('keydown', function (e) {
            if (e.key === 'ArrowDown') { move(1); e.preventDefault(); }
            else if (e.key === 'ArrowUp') { move(-1); e.preventDefault(); }
            else if (e.key === 'Enter') {
                if (state.items[state.active]) activate(state.items[state.active]);
                e.preventDefault();
            } else if (e.key === 'Escape') { close(true); e.preventDefault(); e.stopPropagation(); }
        });

        state.root = root;
        state.input = input;
        state.list = list;
    }

    function render() {
        var query = state.input.value.trim().toLowerCase();
        state.items = search(state.input.value);
        state.active = 0;
        state.list.replaceChildren();
        if (!state.items.length) {
            state.list.appendChild(el('li', 'sx-pal-empty', 'Nothing matches “' + state.input.value.trim() + '”.'));
            return;
        }
        var lastType = null;
        state.items.forEach(function (item, i) {
            if (item.type !== lastType) {
                var heading = el('li', 'sx-pal-group');
                heading.setAttribute('role', 'presentation');
                heading.textContent = GROUPS.filter(function (g) { return g[0] === item.type; })[0][1];
                state.list.appendChild(heading);
                lastType = item.type;
            }
            var row = el('li', 'sx-pal-item');
            row.id = 'sx-pal-item-' + i;
            row.setAttribute('role', 'option');
            row.appendChild(el('i', 'bi ' + item.icon));
            var text = el('div', 'sx-pal-text');
            text.appendChild(highlighted(item.label, query));
            if (item.secondary) text.appendChild(el('span', 'sx-pal-secondary', item.secondary));
            row.appendChild(text);
            row.addEventListener('mousemove', function () { if (state.active !== i) select(i); });
            row.addEventListener('click', function () { activate(item); });
            state.list.appendChild(row);
        });
        select(0);
    }

    function select(i) {
        var rows = state.list.querySelectorAll('.sx-pal-item');
        rows.forEach(function (row, n) { row.classList.toggle('active', n === i); row.setAttribute('aria-selected', n === i); });
        state.active = i;
        if (rows[i]) {
            rows[i].scrollIntoView({ block: 'nearest' });
            state.input.setAttribute('aria-activedescendant', rows[i].id);
        }
    }

    function move(delta) {
        if (!state.items.length) return;
        select((state.active + delta + state.items.length) % state.items.length);
    }

    function open() {
        if (state.open) { state.input.select(); return; }
        if (!state.root) build();
        state.open = true;
        state.lastFocus = document.activeElement;
        state.root.hidden = false;
        state.input.value = '';
        state.list.replaceChildren(el('li', 'sx-pal-empty', 'Loading…'));
        state.input.focus();
        if (state.handlers.onOpen) state.handlers.onOpen();
        load().then(function () { if (state.open) render(); });
    }

    function close(restoreFocus) {
        if (!state.open) return;
        state.open = false;
        state.root.hidden = true;
        if (state.handlers.onClose) state.handlers.onClose();
        if (restoreFocus && state.lastFocus && state.lastFocus.focus) state.lastFocus.focus();
    }

    /* ---- setup ---- */

    function typing(target) {
        if (!target || !target.tagName) return false;
        var tag = target.tagName.toLowerCase();
        return tag === 'input' || tag === 'textarea' || tag === 'select' || target.isContentEditable;
    }

    function init() {
        var bar = document.querySelector('.sx-appbar[data-workspace], .sx-appbar[data-catalog]');
        if (!bar) return;
        state.slug = bar.getAttribute('data-workspace');
        state.isStatic = !state.slug;
        state.catalogUrl = state.slug
            ? '/api/workspace/' + encodeURIComponent(state.slug) + '/catalog'
            : bar.getAttribute('data-catalog');

        var button = bar.querySelector('.sx-find');
        if (button) {
            button.querySelector('.sx-find-key').textContent = isMac ? '⌘K' : 'Ctrl K';
            button.addEventListener('click', open);
        }

        document.addEventListener('keydown', function (e) {
            if ((e.metaKey || e.ctrlKey) && !e.altKey && (e.key === 'k' || e.key === 'K')) {
                e.preventDefault();
                if (state.open) close(true); else open();
            } else if (e.key === ' ' && state.handlers.openOnSpace && !state.open && !typing(e.target) &&
                       !document.querySelector('.modal.show')) {
                e.preventDefault();
                open();
            }
        });
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }

    global.StructurizrPalette = {
        /** onView(key) / onElement(element): return true when handled in
         *  place; onOpen / onClose: around the dialog; openOnSpace: also
         *  open on the space bar. */
        configure: function (handlers) {
            Object.keys(handlers).forEach(function (k) { state.handlers[k] = handlers[k]; });
        },
        open: open,
        close: function () { close(true); },
        isOpen: function () { return state.open; }
    };
})(window);
