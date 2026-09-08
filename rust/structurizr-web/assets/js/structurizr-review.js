/*
 * Element walkthrough for /workspace/{name}/review.
 *
 * Data comes from /api/workspace/{name}/review, which carries the element's
 * neighbourhood, the views that show it and its hygiene findings, all derived
 * from the shared graph index on the server.
 *
 * Review state (which elements have been looked at, which are flagged) lives in
 * the URL hash, not on the server: it makes a part-finished review a link
 * someone can hand to a colleague, and it survives a reload. Element ids are
 * short and stable, so a sparse id list stays comfortably short; only
 * non-default states are recorded.
 */
(function () {
    'use strict';

    var slug = window.REVIEW_WORKSPACE_SLUG;

    var state = {
        elements: [],
        filtered: [],
        selected: null,
        reviewed: new Set(),
        flagged: new Set()
    };

    function el(id) {
        return document.getElementById(id);
    }

    /* ---- URL state ---- */

    function readHash() {
        var params = new URLSearchParams(location.hash.replace(/^#/, ''));
        var list = function (key) {
            var raw = params.get(key);
            return new Set(raw ? raw.split(',').filter(Boolean).map(decodeURIComponent) : []);
        };
        state.reviewed = list('r');
        state.flagged = list('f');
        return params.get('e');
    }

    function writeHash() {
        /* Built by hand rather than with URLSearchParams, which percent-encodes
           the separators and turns a shareable link into r=3%2C5. Ids are
           encoded individually, so an id containing a comma still round-trips. */
        var parts = [];
        var idList = function (key, set) {
            if (set.size) {
                parts.push(key + '=' + Array.from(set).map(encodeURIComponent).join(','));
            }
        };
        idList('r', state.reviewed);
        idList('f', state.flagged);
        if (state.selected) {
            parts.push('e=' + encodeURIComponent(state.selected));
        }
        /* replaceState, not location.hash: a review is one navigation, and
           every keystroke should not become a browser history entry. */
        history.replaceState(null, '', location.pathname + location.search + '#' + parts.join('&'));
    }

    /* ---- filtering ---- */

    function matches(element, query, kind, only) {
        if (kind && element.kind !== kind) {
            return false;
        }
        if (only === 'findings' && !element.findings.length) {
            return false;
        }
        if (only === 'blocking' && !element.findings.some(function (f) { return f.blocking; })) {
            return false;
        }
        if (only === 'unreviewed' && state.reviewed.has(element.id)) {
            return false;
        }
        if (only === 'flagged' && !state.flagged.has(element.id)) {
            return false;
        }
        if (!query) {
            return true;
        }
        var haystack = [
            element.name,
            element.description || '',
            element.technology || '',
            element.group || '',
            (element.tags || []).join(' ')
        ].join(' ').toLowerCase();
        return haystack.indexOf(query) !== -1;
    }

    function sortElements(list, order) {
        var copy = list.slice();
        if (order === 'name') {
            copy.sort(function (a, b) { return a.name.localeCompare(b.name); });
        } else if (order === 'degree') {
            /* Most-connected first: hubs are where a review pays off. */
            copy.sort(function (a, b) { return degree(b) - degree(a); });
        } else if (order === 'orphans') {
            /* Least-connected first: the suspicious end of the model. */
            copy.sort(function (a, b) { return degree(a) - degree(b); });
        } else if (order === 'findings') {
            copy.sort(function (a, b) { return b.findings.length - a.findings.length; });
        }
        return copy;
    }

    function degree(element) {
        return element.incoming.length + element.outgoing.length;
    }

    function applyFilters(keepSelection) {
        var query = el('filter-query').value.trim().toLowerCase();
        var kind = el('filter-kind').value;
        var only = el('filter-only').value;

        state.filtered = sortElements(
            state.elements.filter(function (e) { return matches(e, query, kind, only); }),
            el('filter-sort').value
        );

        renderList();

        if (!keepSelection || !state.filtered.some(function (e) { return e.id === state.selected; })) {
            select(state.filtered.length ? state.filtered[0].id : null);
        }
    }

    /* ---- rendering ---- */

    function renderList() {
        var list = el('review-list');
        list.innerHTML = '';

        state.filtered.forEach(function (element) {
            var li = document.createElement('li');
            li.className = 'review-item';
            li.dataset.id = element.id;
            if (element.id === state.selected) {
                li.classList.add('selected');
            }
            if (state.reviewed.has(element.id)) {
                li.classList.add('is-reviewed');
            }

            var mark = document.createElement('span');
            mark.className = 'item-state';
            mark.textContent = state.flagged.has(element.id)
                ? '⚑'
                : (state.reviewed.has(element.id) ? '✓' : '');
            li.appendChild(mark);

            var name = document.createElement('span');
            name.className = 'item-name';
            name.textContent = element.name;
            li.appendChild(name);

            if (element.findings.length) {
                var count = document.createElement('span');
                count.className = 'badge rounded-pill ' +
                    (element.findings.some(function (f) { return f.blocking; })
                        ? 'text-bg-danger'
                        : 'text-bg-warning');
                count.textContent = String(element.findings.length);
                li.appendChild(count);
            }

            var kind = document.createElement('span');
            kind.className = 'item-kind';
            kind.textContent = element.kind;
            li.appendChild(kind);

            li.addEventListener('click', function () { select(element.id); });
            list.appendChild(li);
        });

        el('review-count').textContent =
            state.filtered.length + ' of ' + state.elements.length + ' element(s)';
    }

    function section(title, node) {
        var wrapper = document.createElement('section');
        wrapper.className = 'detail-section';
        var heading = document.createElement('h2');
        heading.textContent = title;
        wrapper.appendChild(heading);
        wrapper.appendChild(node);
        return wrapper;
    }

    function empty(text) {
        var p = document.createElement('p');
        p.className = 'detail-empty';
        p.textContent = text;
        return p;
    }

    function relationshipTable(rows, arrow) {
        if (!rows.length) {
            return empty('None.');
        }
        var table = document.createElement('table');
        table.className = 'detail-table';
        rows.forEach(function (r) {
            var tr = document.createElement('tr');

            var direction = document.createElement('td');
            direction.className = 'col-direction';
            direction.textContent = arrow;
            tr.appendChild(direction);

            var other = document.createElement('td');
            var link = document.createElement('a');
            link.href = '#';
            link.textContent = r.otherName;
            link.addEventListener('click', function (evt) {
                evt.preventDefault();
                jumpTo(r.otherId);
            });
            other.appendChild(link);
            tr.appendChild(other);

            var meta = document.createElement('td');
            meta.className = 'col-meta';
            var bits = [];
            if (r.description) { bits.push(r.description); }
            if (r.technology) { bits.push('[' + r.technology + ']'); }
            if (r.kind) { bits.push(r.kind); }
            meta.textContent = bits.length ? bits.join(' ') : '— no description —';
            tr.appendChild(meta);

            table.appendChild(tr);
        });
        return table;
    }

    function renderDetail() {
        var detail = el('review-detail');
        detail.innerHTML = '';

        var element = state.elements.find(function (e) { return e.id === state.selected; });
        if (!element) {
            detail.appendChild(empty('No element selected.'));
            return;
        }

        var title = document.createElement('h1');
        title.textContent = element.name;
        detail.appendChild(title);

        var subtitle = document.createElement('div');
        subtitle.className = 'detail-subtitle';
        var bits = [element.kind];
        if (element.technology) { bits.push(element.technology); }
        if (element.parentName) { bits.push('in ' + element.parentName); }
        if (element.group) { bits.push('group: ' + element.group); }
        var d = degree(element);
        bits.push(d + ' relationship(s)');
        subtitle.textContent = bits.join(' · ');
        detail.appendChild(subtitle);

        var controls = document.createElement('div');
        controls.className = 'btn-group btn-group-sm';
        controls.appendChild(toggleButton('reviewed', element));
        controls.appendChild(toggleButton('flagged', element));
        detail.appendChild(controls);

        detail.appendChild(section('Description',
            element.description ? text(element.description) : empty('No description.')));

        if (element.tags.length) {
            detail.appendChild(section('Tags', text(element.tags.join(', '))));
        }

        var findings = document.createElement('div');
        if (element.findings.length) {
            element.findings.forEach(function (f) {
                var row = document.createElement('div');
                row.className = 'finding' + (f.blocking ? ' blocking' : '');
                var code = document.createElement('code');
                code.textContent = f.code;
                row.appendChild(code);
                var message = document.createElement('span');
                message.textContent = f.message;
                row.appendChild(message);
                findings.appendChild(row);
            });
        } else {
            findings.appendChild(empty('Nothing flagged.'));
        }
        detail.appendChild(section('Findings', findings));

        detail.appendChild(section('Incoming', relationshipTable(element.incoming, '←')));
        detail.appendChild(section('Outgoing', relationshipTable(element.outgoing, '→')));

        var views = document.createElement('div');
        if (element.views.length) {
            element.views.forEach(function (v) {
                var link = document.createElement('a');
                link.className = 'd-block';
                link.href = '/workspace/' + encodeURIComponent(slug) + '#' + encodeURIComponent(v.key);
                link.textContent = v.name + ' (' + v.kind + ')';
                views.appendChild(link);
            });
        } else {
            views.appendChild(empty('This element is not shown by any view.'));
        }
        detail.appendChild(section('Appears in', views));

        if (element.children.length) {
            var children = document.createElement('div');
            element.children.forEach(function (id) {
                var child = state.elements.find(function (e) { return e.id === id; });
                var link = document.createElement('a');
                link.className = 'd-block';
                link.href = '#';
                link.textContent = child ? child.name : id;
                link.addEventListener('click', function (evt) {
                    evt.preventDefault();
                    jumpTo(id);
                });
                children.appendChild(link);
            });
            detail.appendChild(section('Contains', children));
        }
    }

    function text(value) {
        var p = document.createElement('p');
        p.textContent = value;
        return p;
    }

    function toggleButton(kind, element) {
        var set = kind === 'reviewed' ? state.reviewed : state.flagged;
        var on = set.has(element.id);
        var button = document.createElement('button');
        button.type = 'button';
        button.className = 'btn btn-sm ' + (on
            ? (kind === 'reviewed' ? 'btn-success' : 'btn-warning')
            : 'btn-outline-secondary');
        button.textContent = (on ? '✓ ' : '') +
            (kind === 'reviewed' ? 'Reviewed (r)' : 'Flagged (f)');
        button.addEventListener('click', function () { toggle(kind, element.id); });
        return button;
    }

    /* ---- actions ---- */

    function select(id) {
        state.selected = id;
        [].forEach.call(document.querySelectorAll('.review-item'), function (node) {
            node.classList.toggle('selected', node.dataset.id === id);
        });
        var current = document.querySelector('.review-item.selected');
        if (current) {
            current.scrollIntoView({ block: 'nearest' });
        }
        renderDetail();
        writeHash();
    }

    /* Jump to an element that the current filter may be hiding — following a
       relationship should never dead-end. */
    function jumpTo(id) {
        if (!state.elements.some(function (e) { return e.id === id; })) {
            return;
        }
        if (!state.filtered.some(function (e) { return e.id === id; })) {
            el('filter-query').value = '';
            el('filter-kind').value = '';
            el('filter-only').value = '';
            applyFilters(true);
        }
        select(id);
    }

    function toggle(kind, id) {
        var set = kind === 'reviewed' ? state.reviewed : state.flagged;
        if (set.has(id)) {
            set.delete(id);
        } else {
            set.add(id);
        }
        renderProgress();
        renderList();
        renderDetail();
        writeHash();
    }

    function move(delta) {
        if (!state.filtered.length) {
            return;
        }
        var index = state.filtered.findIndex(function (e) { return e.id === state.selected; });
        var next = Math.min(Math.max(index + delta, 0), state.filtered.length - 1);
        select(state.filtered[next].id);
    }

    function renderProgress() {
        var total = state.elements.length;
        var done = state.elements.filter(function (e) { return state.reviewed.has(e.id); }).length;
        var flagged = state.elements.filter(function (e) { return state.flagged.has(e.id); }).length;
        var percent = total ? Math.round((done / total) * 100) : 0;
        el('progress-bar').style.width = percent + '%';
        el('progress-label').textContent =
            done + ' / ' + total + ' reviewed' + (flagged ? ' · ' + flagged + ' flagged' : '');
    }

    /* ---- report ---- */

    /*
     * A review is only useful if it can leave the browser. The report lists
     * what was flagged and everything still carrying a finding, so it can be
     * pasted into an issue or a PR.
     */
    function report() {
        var lines = ['# Review — ' + slug, ''];
        lines.push('Reviewed ' + state.reviewed.size + ' of ' + state.elements.length + ' elements.');
        lines.push('');

        var flagged = state.elements.filter(function (e) { return state.flagged.has(e.id); });
        if (flagged.length) {
            lines.push('## Flagged');
            lines.push('');
            flagged.forEach(function (e) {
                lines.push('- **' + e.name + '** (' + e.kind + ')');
                e.findings.forEach(function (f) {
                    lines.push('  - `' + f.code + '` ' + f.message);
                });
            });
            lines.push('');
        }

        var withFindings = state.elements.filter(function (e) { return e.findings.length; });
        if (withFindings.length) {
            lines.push('## Findings');
            lines.push('');
            lines.push('| Element | Kind | Code | Detail |');
            lines.push('| --- | --- | --- | --- |');
            withFindings.forEach(function (e) {
                e.findings.forEach(function (f) {
                    lines.push('| ' + e.name + ' | ' + e.kind + ' | `' + f.code + '` | ' + f.message + ' |');
                });
            });
        }

        return lines.join('\n');
    }

    function downloadReport() {
        var blob = new Blob([report()], { type: 'text/markdown' });
        var url = URL.createObjectURL(blob);
        var a = document.createElement('a');
        a.href = url;
        a.download = slug + '-review.md';
        a.click();
        URL.revokeObjectURL(url);
    }

    /* ---- boot ---- */

    function populateKinds() {
        var kinds = [];
        state.elements.forEach(function (e) {
            if (kinds.indexOf(e.kind) === -1) {
                kinds.push(e.kind);
            }
        });
        var select = el('filter-kind');
        kinds.forEach(function (kind) {
            var option = document.createElement('option');
            option.value = kind;
            option.textContent = kind;
            select.appendChild(option);
        });
    }

    function load(initial) {
        return fetch('/api/workspace/' + encodeURIComponent(slug) + '/review')
            .then(function (r) {
                if (!r.ok) {
                    throw new Error('review request failed (' + r.status + ')');
                }
                return r.json();
            })
            .then(function (data) {
                state.elements = data.elements;
                if (initial) {
                    populateKinds();
                }
                renderProgress();
                applyFilters(true);
            })
            .catch(function (e) {
                el('review-detail').textContent = 'Error: ' + e.message;
            });
    }

    ['filter-query', 'filter-kind', 'filter-only', 'filter-sort'].forEach(function (id) {
        el(id).addEventListener('input', function () { applyFilters(true); });
    });

    el('report-button').addEventListener('click', downloadReport);

    document.addEventListener('keydown', function (evt) {
        /* evt.target is not always an Element — a key event dispatched at the
           document has no matches(). */
        var target = evt.target;
        var typing = target instanceof Element && target.matches('input, select, textarea');
        if (typing || evt.metaKey || evt.ctrlKey || evt.altKey) {
            return;
        }
        if (evt.key === 'j' || evt.key === 'n') {
            move(1);
        } else if (evt.key === 'k' || evt.key === 'p') {
            move(-1);
        } else if (evt.key === 'r' && state.selected) {
            toggle('reviewed', state.selected);
        } else if (evt.key === 'f' && state.selected) {
            toggle('flagged', state.selected);
        } else if (evt.key === '/') {
            evt.preventDefault();
            el('filter-query').focus();
        } else {
            return;
        }
        evt.preventDefault();
    });

    var initiallySelected = readHash();

    load(true).then(function () {
        if (initiallySelected) {
            jumpTo(initiallySelected);
        }
    });

    /* Live reload: the model can change under a review, and the findings shown
       must not describe a file that no longer exists. Review marks are keyed by
       element id, so they survive the refresh. */
    function connectWs() {
        var proto = location.protocol === 'https:' ? 'wss' : 'ws';
        var ws = new WebSocket(proto + '://' + location.host + '/ws');
        ws.onmessage = function (e) {
            if (JSON.parse(e.data).type === 'reload') {
                load(false);
            }
        };
        ws.onclose = function () { setTimeout(connectWs, 2000); };
    }
    connectWs();
})();
