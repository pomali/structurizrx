/*
 * Version comparison for /workspace/{name}/diff.
 *
 * Two revision pickers, filled from /api/workspace/{name}/revisions (the git
 * history of the workspace file), and the model-level comparison of the two
 * from /api/workspace/{name}/diff. The server does the diffing: it is a model
 * comparison keyed on element paths, not a text diff, so the page only has to
 * present it.
 *
 * The chosen pair lives in the query string (?from=&to=) rather than in the
 * page's own state, so a comparison someone found is a link they can paste
 * into a review — the same reasoning as the review page's hash state.
 */
(function () {
    'use strict';

    var slug = window.DIFF_WORKSPACE_SLUG;

    /* The one revision that is not a git revision: the file as it is on disk,
       uncommitted edits included. Must match git::WORKING on the server. */
    var WORKING = 'working';

    var state = {
        history: null,
        diff: null,
        from: null,
        to: null
    };

    function el(id) {
        return document.getElementById(id);
    }

    function api(path) {
        return '/api/workspace/' + encodeURIComponent(slug) + path;
    }

    /* ---- URL state ---- */

    function readParams() {
        var params = new URLSearchParams(location.search);
        state.from = params.get('from');
        state.to = params.get('to');
    }

    function writeParams() {
        var params = new URLSearchParams();
        params.set('from', state.from);
        params.set('to', state.to);
        history.replaceState(null, '', location.pathname + '?' + params.toString());
    }

    /* ---- revision pickers ---- */

    function revisionLabel(rev) {
        var date = rev.date ? rev.date.slice(0, 10) : '';
        return rev.short + '  ' + date + '  ' + rev.subject;
    }

    function fillPickers() {
        var revisions = state.history.revisions;

        [['rev-from', 'from'], ['rev-to', 'to']].forEach(function (pair) {
            var select = el(pair[0]);
            select.innerHTML = '';

            var working = document.createElement('option');
            working.value = WORKING;
            working.textContent = state.history.dirty
                ? 'Working copy (uncommitted changes)'
                : 'Working copy';
            select.appendChild(working);

            revisions.forEach(function (rev) {
                var option = document.createElement('option');
                option.value = rev.sha;
                option.textContent = revisionLabel(rev);
                option.title = rev.subject + ' — ' + rev.author;
                select.appendChild(option);
            });

            /* A link may carry an abbreviated sha; match it against the full
               ones so it selects the commit rather than adding a second entry
               for it. */
            var wanted = state[pair[1]];
            var match = Array.prototype.filter.call(select.options, function (o) {
                return o.value === wanted || o.value.indexOf(wanted) === 0;
            })[0];
            if (match) {
                select.value = match.value;
                /* Adopt the full sha: the comparison is then pinned to one
                   commit even if the abbreviation later becomes ambiguous. */
                state[pair[1]] = match.value;
                return;
            }

            /* A revision named rather than spelled out (HEAD~2), or a commit
               older than the listed ones: keep it selectable rather than
               silently comparing something else. */
            var extra = document.createElement('option');
            extra.value = wanted;
            extra.textContent = wanted + ' (not in the list above)';
            select.insertBefore(extra, select.firstChild);
            select.value = wanted;
        });
    }

    /* Default pair: the last commit against the working copy, which is the
       question being asked most of the time ("what have I changed?"). With a
       clean tree that comparison is empty and the previous commit is the
       useful one instead. */
    function defaultPair() {
        var revisions = state.history.revisions;
        if (state.history.dirty || revisions.length < 2) {
            return [revisions.length ? revisions[0].sha : 'HEAD', WORKING];
        }
        return [revisions[1].sha, revisions[0].sha];
    }

    /* ---- rendering ---- */

    function badge(change) {
        var span = document.createElement('span');
        span.className = 'change-badge change-' + change;
        span.textContent = change === 'added' ? '+' : (change === 'removed' ? '−' : '~');
        span.title = change;
        return span;
    }

    function countsRow(label, counts) {
        var parts = [];
        if (counts.added) parts.push(['added', '+' + counts.added]);
        if (counts.removed) parts.push(['removed', '−' + counts.removed]);
        if (counts.modified) parts.push(['modified', '~' + counts.modified]);

        var div = document.createElement('div');
        div.className = 'summary-card' + (parts.length ? '' : ' summary-card-quiet');

        var name = document.createElement('div');
        name.className = 'summary-label';
        name.textContent = label;
        div.appendChild(name);

        var value = document.createElement('div');
        value.className = 'summary-value';
        if (parts.length) {
            parts.forEach(function (p) {
                var chip = document.createElement('span');
                chip.className = 'change-' + p[0];
                chip.textContent = p[1];
                value.appendChild(chip);
            });
        } else {
            value.textContent = 'unchanged';
        }
        div.appendChild(value);
        return div;
    }

    function renderSummary() {
        var summary = state.diff.diff.summary;
        var host = el('diff-summary');
        host.innerHTML = '';

        host.appendChild(countsRow('Elements', summary.elements));
        host.appendChild(countsRow('Relationships', summary.relationships));
        host.appendChild(countsRow('Views', summary.views));
        host.appendChild(countsRow('Decisions', summary.decisions));

        var size = document.createElement('div');
        size.className = 'summary-card summary-card-quiet';
        size.innerHTML = '<div class="summary-label">Model size</div>' +
            '<div class="summary-value">' +
            summary.elementCountBefore + ' → ' + summary.elementCountAfter + ' elements, ' +
            summary.relationshipCountBefore + ' → ' + summary.relationshipCountAfter +
            ' relationships</div>';
        host.appendChild(size);
    }

    function fieldTable(fields) {
        if (!fields.length) {
            return null;
        }
        var table = document.createElement('table');
        table.className = 'field-table';
        fields.forEach(function (f) {
            var tr = document.createElement('tr');

            var name = document.createElement('th');
            name.textContent = f.field;
            tr.appendChild(name);

            var before = document.createElement('td');
            before.className = 'field-before';
            before.textContent = f.before === null || f.before === undefined ? '—' : f.before;
            tr.appendChild(before);

            var arrow = document.createElement('td');
            arrow.className = 'field-arrow';
            arrow.textContent = '→';
            tr.appendChild(arrow);

            var after = document.createElement('td');
            after.className = 'field-after';
            after.textContent = f.after === null || f.after === undefined ? '—' : f.after;
            tr.appendChild(after);

            table.appendChild(tr);
        });
        return table;
    }

    function entry(change, title, subtitle, fields, extras) {
        var li = document.createElement('li');
        li.className = 'diff-entry diff-' + change;

        var head = document.createElement('div');
        head.className = 'diff-entry-head';
        head.appendChild(badge(change));

        var name = document.createElement('span');
        name.className = 'diff-entry-title';
        name.textContent = title;
        head.appendChild(name);

        if (subtitle) {
            var sub = document.createElement('span');
            sub.className = 'diff-entry-subtitle';
            sub.textContent = subtitle;
            head.appendChild(sub);
        }
        li.appendChild(head);

        var table = fieldTable(fields || []);
        if (table) {
            li.appendChild(table);
        }
        (extras || []).forEach(function (node) {
            li.appendChild(node);
        });
        return li;
    }

    function pathList(label, paths, cls) {
        if (!paths || !paths.length) {
            return null;
        }
        var div = document.createElement('div');
        div.className = 'path-list ' + cls;
        div.textContent = label + ': ' + paths.join(', ');
        return div;
    }

    function section(title, entries) {
        if (!entries.length) {
            return null;
        }
        var wrapper = document.createElement('section');
        wrapper.className = 'diff-section';

        var heading = document.createElement('h2');
        heading.textContent = title;
        var count = document.createElement('span');
        count.className = 'section-count';
        count.textContent = entries.length;
        heading.appendChild(count);
        wrapper.appendChild(heading);

        var list = document.createElement('ul');
        list.className = 'diff-list';
        entries.forEach(function (e) { list.appendChild(e); });
        wrapper.appendChild(list);
        return wrapper;
    }

    function visible(change, haystack) {
        if (!el('show-' + change).checked) {
            return false;
        }
        var query = el('filter-query').value.trim().toLowerCase();
        return !query || haystack.toLowerCase().indexOf(query) !== -1;
    }

    function fieldText(fields) {
        return (fields || []).map(function (f) {
            return f.field + ' ' + (f.before || '') + ' ' + (f.after || '');
        }).join(' ');
    }

    function render() {
        var d = state.diff.diff;
        var content = el('diff-content');
        content.innerHTML = '';
        var shown = 0;

        var sections = [];

        var elements = d.elements
            .filter(function (c) {
                return visible(c.change, c.path + ' ' + c.kind + ' ' + fieldText(c.fields));
            })
            .map(function (c) {
                return entry(c.change, c.path, c.kind, c.fields);
            });
        sections.push(section('Elements', elements));

        var relationships = d.relationships
            .filter(function (c) {
                return visible(c.change, c.source + ' ' + c.destination + ' ' +
                    (c.description || '') + ' ' + fieldText(c.fields));
            })
            .map(function (c) {
                return entry(c.change, c.source + ' → ' + c.destination,
                    c.description || '', c.fields);
            });
        sections.push(section('Relationships', relationships));

        var views = d.views
            .filter(function (c) {
                return visible(c.change, c.key + ' ' + c.name + ' ' + c.kind + ' ' +
                    (c.elementsAdded || []).join(' ') + ' ' +
                    (c.elementsRemoved || []).join(' ') + ' ' + fieldText(c.fields));
            })
            .map(function (c) {
                var extras = [
                    pathList('Now shows', c.elementsAdded, 'change-added'),
                    pathList('No longer shows', c.elementsRemoved, 'change-removed')
                ].filter(Boolean);
                return entry(c.change, c.name, c.kind + ' · ' + c.key, c.fields, extras);
            });
        sections.push(section('Views', views));

        var decisions = d.decisions
            .filter(function (c) {
                return visible(c.change, c.id + ' ' + c.title + ' ' + fieldText(c.fields));
            })
            .map(function (c) {
                return entry(c.change, c.id + '. ' + c.title, c.status, c.fields);
            });
        sections.push(section('Decisions', decisions));

        if (d.workspace.length && visible('modified', 'workspace ' + fieldText(d.workspace))) {
            sections.push(section('Workspace', [entry('modified', 'Workspace', '', d.workspace)]));
        }

        /* Rename hints go first: without them a reader sees a removal and an
           addition and has to work out for themselves that they are the same
           element under a new name. */
        if (d.renames.length) {
            var hints = d.renames.map(function (h) {
                var li = document.createElement('li');
                li.className = 'diff-entry diff-hint';
                li.textContent = h.from + ' → ' + h.to;
                var why = document.createElement('span');
                why.className = 'diff-entry-subtitle';
                why.textContent = 'probably ' + h.reason;
                li.appendChild(why);
                return li;
            });
            var renames = section('Renames and moves', hints);
            renames.insertBefore(note(
                'Paths are how elements are matched, so these appear below as a removal ' +
                'and an addition too.'), renames.lastChild);
            content.appendChild(renames);
        }

        sections.filter(Boolean).forEach(function (s) {
            shown += s.querySelectorAll('.diff-entry').length;
            content.appendChild(s);
        });

        if (!shown) {
            var empty = document.createElement('p');
            empty.className = 'diff-placeholder';
            empty.textContent = totalChanges(d.summary) === 0
                ? 'These two versions describe the same architecture. The text of the file may still differ — formatting, comments and identifier names are not part of the model.'
                : 'No change matches the current filter.';
            content.appendChild(empty);
        }

        el('filter-count').textContent = shown + (shown === 1 ? ' change' : ' changes');
    }

    /* Every counted change, across all four categories. Zero means the two
       versions are the same model, which is a different message from "nothing
       matches the filter". */
    function totalChanges(summary) {
        return ['elements', 'relationships', 'views', 'decisions'].reduce(function (n, key) {
            var c = summary[key];
            return n + c.added + c.removed + c.modified;
        }, 0);
    }

    function note(text) {
        var p = document.createElement('p');
        p.className = 'diff-note';
        p.textContent = text;
        return p;
    }

    /* Show a message instead of a comparison. The pickers and filters are
       hidden with it: with nothing to compare they are controls that cannot do
       anything. */
    function fail(message) {
        document.body.classList.add('diff-unavailable');
        var content = el('diff-content');
        content.innerHTML = '';
        var p = document.createElement('p');
        p.className = 'diff-placeholder diff-error';
        p.textContent = message;
        content.appendChild(p);
        el('diff-summary').innerHTML = '';
        el('filter-count').textContent = '';
    }

    /* ---- loading ---- */

    function loadDiff() {
        document.body.classList.remove('diff-unavailable');
        el('diff-status').textContent = 'Comparing…';
        writeParams();

        var url = api('/diff?from=' + encodeURIComponent(state.from) +
            '&to=' + encodeURIComponent(state.to));

        fetch(url)
            .then(function (r) {
                if (!r.ok) {
                    return r.text().then(function (text) { throw new Error(text); });
                }
                return r.json();
            })
            .then(function (data) {
                state.diff = data;
                el('diff-status').textContent = '';
                renderSummary();
                render();
            })
            .catch(function (e) {
                el('diff-status').textContent = '';
                fail(e.message || String(e));
            });
    }

    function start() {
        readParams();

        fetch(api('/revisions'))
            .then(function (r) {
                if (!r.ok) {
                    return r.text().then(function (text) { throw new Error(text); });
                }
                return r.json();
            })
            .then(function (history) {
                state.history = history;
                if (!history.revisions.length) {
                    fail('This workspace file has no commits yet, so there is nothing to ' +
                         'compare it with.');
                    return;
                }
                if (!state.from || !state.to) {
                    var pair = defaultPair();
                    state.from = state.from || pair[0];
                    state.to = state.to || pair[1];
                }
                fillPickers();
                loadDiff();
            })
            .catch(function (e) {
                fail(e.message || String(e));
            });
    }

    /* ---- events ---- */

    document.addEventListener('DOMContentLoaded', function () {
        el('rev-from').addEventListener('change', function () {
            state.from = this.value;
            loadDiff();
        });
        el('rev-to').addEventListener('change', function () {
            state.to = this.value;
            loadDiff();
        });
        el('swap-revisions').addEventListener('click', function () {
            var from = state.from;
            state.from = state.to;
            state.to = from;
            fillPickers();
            loadDiff();
        });

        ['filter-query', 'show-added', 'show-removed', 'show-modified'].forEach(function (id) {
            el(id).addEventListener('input', function () {
                if (state.diff) {
                    render();
                }
            });
        });

        el('copy-link').addEventListener('click', function (event) {
            event.preventDefault();
            var self = this;
            navigator.clipboard.writeText(location.href).then(function () {
                self.classList.add('btn-success');
                self.classList.remove('btn-outline-secondary');
                setTimeout(function () {
                    self.classList.remove('btn-success');
                    self.classList.add('btn-outline-secondary');
                }, 1200);
            });
        });

        start();
    });
}());
