/*
 * Cluster analysis for /workspace/{name}/clusters.
 *
 * All computation happens on the server (structurizr_query::cluster); this
 * renders the four readings of the result: the summary, the communities against
 * what the model declares, a dependency-structure matrix reordered by community
 * and a metrics table.
 *
 * The controls are reflected in the query string so a particular analysis — a
 * level, a tag filter — is a link rather than a set of clicks to repeat.
 */
(function () {
    'use strict';

    var slug = window.CLUSTERS_WORKSPACE_SLUG;

    var state = {
        analysis: null,
        sort: { column: 'betweenness', descending: true }
    };

    function el(id) {
        return document.getElementById(id);
    }

    function options() {
        return {
            level: el('opt-level').value,
            include: el('opt-include').value.trim(),
            exclude: el('opt-exclude').value.trim(),
            implied: el('opt-implied').checked
        };
    }

    function applyOptionsToUrl(opts) {
        var params = new URLSearchParams();
        params.set('level', opts.level);
        if (opts.include) { params.set('include', opts.include); }
        if (opts.exclude) { params.set('exclude', opts.exclude); }
        if (!opts.implied) { params.set('implied', 'false'); }
        history.replaceState(null, '', location.pathname + '?' + params.toString());
        return params;
    }

    function readOptionsFromUrl() {
        var params = new URLSearchParams(location.search);
        if (params.get('level')) { el('opt-level').value = params.get('level'); }
        if (params.get('include')) { el('opt-include').value = params.get('include'); }
        if (params.get('exclude')) { el('opt-exclude').value = params.get('exclude'); }
        el('opt-implied').checked = params.get('implied') !== 'false';
    }

    /* ---- summary ---- */

    function stat(value, label, severity) {
        var box = document.createElement('div');
        box.className = 'stat' + (severity ? ' ' + severity : '');
        var v = document.createElement('div');
        v.className = 'stat-value';
        v.textContent = value;
        var l = document.createElement('div');
        l.className = 'stat-label';
        l.textContent = label;
        box.appendChild(v);
        box.appendChild(l);
        return box;
    }

    function renderSummary(a) {
        var summary = el('summary');
        summary.innerHTML = '';

        summary.appendChild(stat(a.nodes.length, 'elements'));
        summary.appendChild(stat(a.edges.length, 'dependencies'));
        summary.appendChild(stat(a.communities.length, 'communities'));

        /* Below about 0.3 the partition is weak, and the conformance findings
           built on it deserve correspondingly less weight. */
        summary.appendChild(stat(
            a.modularity.toFixed(3),
            'modularity',
            a.modularity < 0.3 ? 'weak' : ''
        ));

        summary.appendChild(stat(a.components, 'disconnected parts', a.components > 1 ? 'weak' : ''));
        summary.appendChild(stat(a.cycles.length, 'cycles', a.cycles.length ? 'bad' : ''));
        summary.appendChild(stat(a.bridges.length, 'bridges', a.bridges.length ? 'weak' : ''));
        summary.appendChild(stat(
            a.conformance.length,
            'conformance findings',
            a.conformance.length ? 'bad' : ''
        ));
    }

    /* ---- communities ---- */

    function renderCommunities(a) {
        var container = el('communities');
        container.innerHTML = '';

        var byId = {};
        a.nodes.forEach(function (n) { byId[n.id] = n; });

        a.communities.forEach(function (community) {
            var card = document.createElement('div');
            card.className = 'community-card';
            card.style.borderLeftColor = colorFor(community.id);

            var title = document.createElement('h3');
            title.textContent = 'Community ' + (community.id + 1) +
                ' · ' + community.memberIds.length + ' element(s)';
            card.appendChild(title);

            var declared = document.createElement('div');
            declared.className = 'declared';
            declared.textContent = community.dominantDeclared
                ? 'mostly declared as "' + community.dominantDeclared + '" (' +
                  community.agreeing + '/' + community.memberIds.length + ')'
                : 'no dominant declared group';
            card.appendChild(declared);

            var list = document.createElement('ul');
            community.memberIds.forEach(function (id) {
                var node = byId[id];
                var li = document.createElement('li');
                li.textContent = node.name;
                if (node.declared && community.dominantDeclared &&
                    node.declared !== community.dominantDeclared) {
                    li.classList.add('mismatch');
                    li.textContent += ' — declared "' + node.declared + '"';
                }
                list.appendChild(li);
            });
            card.appendChild(list);
            container.appendChild(card);
        });
    }

    function colorFor(index) {
        return 'hsl(' + ((index * 137) % 360) + ' 60% 45%)';
    }

    /* ---- conformance ---- */

    function renderConformance(a) {
        var container = el('conformance');
        container.innerHTML = '';

        if (!a.conformance.length) {
            container.appendChild(note('The declared structure and the detected communities agree.'));
            return;
        }

        a.conformance.forEach(function (f) {
            var row = document.createElement('div');
            row.className = 'finding-row';
            row.textContent = f.message;
            container.appendChild(row);
        });
    }

    function note(text) {
        var p = document.createElement('p');
        p.className = 'empty-note';
        p.textContent = text;
        return p;
    }

    /* ---- structure ---- */

    function renderStructure(a) {
        var container = el('structure');
        container.innerHTML = '';

        container.appendChild(list('Cycles', a.cycles.map(function (c) {
            return c.memberNames.join(' → ') + ' → …';
        }), 'No dependency cycles.'));

        container.appendChild(list('Bridges', a.bridges.map(function (b) {
            return b.sourceName + ' — ' + b.targetName;
        }), 'No single dependency holds the model together.'));

        var byId = {};
        a.nodes.forEach(function (n) { byId[n.id] = n; });
        container.appendChild(list('Articulation points', a.articulationPointIds.map(function (id) {
            return byId[id] ? byId[id].name : id;
        }), 'No single element holds the model together.'));
    }

    function list(title, items, emptyText) {
        var wrapper = document.createElement('div');
        wrapper.className = 'mb-3';

        var heading = document.createElement('div');
        heading.className = 'fw-semibold';
        heading.textContent = title + ' (' + items.length + ')';
        wrapper.appendChild(heading);

        if (!items.length) {
            wrapper.appendChild(note(emptyText));
            return wrapper;
        }

        var ul = document.createElement('ul');
        ul.className = 'mb-0';
        items.forEach(function (item) {
            var li = document.createElement('li');
            li.textContent = item;
            ul.appendChild(li);
        });
        wrapper.appendChild(ul);
        return wrapper;
    }

    /* ---- dependency structure matrix ---- */

    /*
     * Rows and columns are ordered by community, so a clean modularisation
     * shows as filled blocks on the diagonal. Cells outside those blocks are
     * the dependencies that cross a community boundary — the ones worth
     * arguing about.
     */
    function renderMatrix(a) {
        var container = el('dsm-scroll');
        container.innerHTML = '';

        if (!a.nodes.length) {
            container.appendChild(note('Nothing to show at this level.'));
            return;
        }

        var order = a.nodes.slice().sort(function (x, y) {
            return x.community - y.community || x.name.localeCompare(y.name);
        });
        var position = {};
        order.forEach(function (n, i) { position[n.id] = i; });

        var weights = {};
        a.edges.forEach(function (e) {
            weights[e.sourceId + '>' + e.targetId] = e.weight;
        });

        var lastOfCommunity = {};
        order.forEach(function (n, i) { lastOfCommunity[n.community] = i; });

        var table = document.createElement('table');
        table.className = 'dsm';

        var head = document.createElement('tr');
        head.appendChild(document.createElement('th'));
        order.forEach(function (n, i) {
            var th = document.createElement('th');
            th.className = 'col-label' + (lastOfCommunity[n.community] === i ? ' boundary-right' : '');
            var span = document.createElement('span');
            span.textContent = n.name;
            th.appendChild(span);
            head.appendChild(th);
        });
        table.appendChild(head);

        order.forEach(function (row, rowIndex) {
            var tr = document.createElement('tr');

            var label = document.createElement('th');
            label.className = 'row-label';
            label.textContent = row.name;
            label.title = row.name + (row.declared ? ' — declared "' + row.declared + '"' : '');
            tr.appendChild(label);

            order.forEach(function (col, colIndex) {
                var td = document.createElement('td');
                var weight = weights[row.id + '>' + col.id];

                if (rowIndex === colIndex) {
                    td.className = 'diagonal';
                } else if (weight) {
                    td.className = row.community === col.community ? 'intra' : 'inter';
                    td.textContent = weight;
                    td.title = row.name + ' → ' + col.name + ' (' + weight + ')';
                }

                if (lastOfCommunity[col.community] === colIndex) {
                    td.classList.add('boundary-right');
                }
                if (lastOfCommunity[row.community] === rowIndex) {
                    td.classList.add('boundary-bottom');
                }
                tr.appendChild(td);
            });

            table.appendChild(tr);
        });

        container.appendChild(table);
    }

    /* ---- metrics ---- */

    var COLUMNS = [
        { key: 'name', label: 'Element', numeric: false },
        { key: 'declared', label: 'Declared', numeric: false },
        { key: 'community', label: 'Community', numeric: true, format: function (v) { return v + 1; } },
        { key: 'afferent', label: 'Ca', numeric: true, title: 'Afferent coupling: dependencies arriving' },
        { key: 'efferent', label: 'Ce', numeric: true, title: 'Efferent coupling: dependencies leaving' },
        { key: 'instability', label: 'I', numeric: true, title: 'Instability: Ce / (Ca + Ce)', format: fixed3 },
        { key: 'pageRank', label: 'PageRank', numeric: true, format: fixed3 },
        { key: 'betweenness', label: 'Betweenness', numeric: true, format: fixed3 },
        { key: 'isArticulationPoint', label: 'Cut', numeric: false, title: 'Removing this element disconnects the graph', format: function (v) { return v ? '●' : ''; } }
    ];

    function fixed3(value) {
        return value === null || value === undefined ? '—' : Number(value).toFixed(3);
    }

    function renderMetrics(a) {
        var container = el('metrics');
        container.innerHTML = '';

        var table = document.createElement('table');
        table.className = 'metrics';

        var head = document.createElement('tr');
        COLUMNS.forEach(function (column) {
            var th = document.createElement('th');
            th.textContent = column.label +
                (state.sort.column === column.key ? (state.sort.descending ? ' ▾' : ' ▴') : '');
            if (column.numeric) { th.className = 'numeric'; }
            if (column.title) { th.title = column.title; }
            th.addEventListener('click', function () {
                if (state.sort.column === column.key) {
                    state.sort.descending = !state.sort.descending;
                } else {
                    state.sort.column = column.key;
                    state.sort.descending = column.numeric;
                }
                renderMetrics(a);
            });
            head.appendChild(th);
        });
        table.appendChild(head);

        var rows = a.nodes.slice().sort(function (x, y) {
            var left = x[state.sort.column];
            var right = y[state.sort.column];
            if (left === null || left === undefined) { left = -1; }
            if (right === null || right === undefined) { right = -1; }
            var order = typeof left === 'string'
                ? left.localeCompare(right)
                : (left === right ? 0 : (left < right ? -1 : 1));
            return state.sort.descending ? -order : order;
        });

        rows.forEach(function (node) {
            var tr = document.createElement('tr');
            COLUMNS.forEach(function (column) {
                var td = document.createElement('td');
                var value = node[column.key];
                td.textContent = column.format ? column.format(value) : (value === null || value === undefined ? '—' : value);
                if (column.numeric) { td.className = 'numeric'; }
                if (column.key === 'community') {
                    td.style.color = colorFor(node.community);
                    td.style.fontWeight = '600';
                }
                tr.appendChild(td);
            });
            table.appendChild(tr);
        });

        container.appendChild(table);
    }

    /* ---- load ---- */

    function load() {
        var opts = options();
        var params = applyOptionsToUrl(opts);
        el('clusters-status').textContent = 'Analysing…';

        return fetch('/api/workspace/' + encodeURIComponent(slug) + '/clusters?' + params.toString())
            .then(function (r) {
                if (!r.ok) {
                    throw new Error('cluster request failed (' + r.status + ')');
                }
                return r.json();
            })
            .then(function (analysis) {
                state.analysis = analysis;
                el('clusters-status').textContent = '';

                if (!analysis.nodes.length) {
                    el('clusters-body').querySelectorAll('section.panel').forEach(function (s) {
                        s.style.display = 'none';
                    });
                    el('summary').innerHTML = '';
                    el('summary').appendChild(note('No elements match this level and filter.'));
                    return;
                }

                el('clusters-body').querySelectorAll('section.panel').forEach(function (s) {
                    s.style.display = '';
                });

                renderSummary(analysis);
                renderCommunities(analysis);
                renderConformance(analysis);
                renderStructure(analysis);
                renderMatrix(analysis);
                renderMetrics(analysis);
            })
            .catch(function (e) {
                el('clusters-status').textContent = 'Error: ' + e.message;
            });
    }

    ['opt-level', 'opt-include', 'opt-exclude', 'opt-implied'].forEach(function (id) {
        el(id).addEventListener('change', load);
    });

    readOptionsFromUrl();
    load();

    function connectWs() {
        var proto = location.protocol === 'https:' ? 'wss' : 'ws';
        var ws = new WebSocket(proto + '://' + location.host + '/ws');
        ws.onmessage = function (e) {
            if (JSON.parse(e.data).type === 'reload') {
                load();
            }
        };
        ws.onclose = function () { setTimeout(connectWs, 2000); };
    }
    connectWs();
})();
