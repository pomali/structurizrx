/*
 * The workspace home, /workspace/{name}/overview: what the system is, whether
 * the model is in good shape, what changed lately, and every view — the page
 * someone new to the workspace should land on.
 *
 * Everything comes from APIs other pages already use: the find palette's
 * catalog (names, counts, views), the review read model (findings), the git
 * history behind Compare, and the cluster analysis (conformance). Each panel
 * renders as soon as its own data arrives, so a slow one holds up nothing.
 */
(function () {
    'use strict';

    var slug = window.OVERVIEW_WORKSPACE_SLUG;
    var base = '/workspace/' + encodeURIComponent(slug);
    var api = '/api' + base;

    function el(tag, className, text) {
        var node = document.createElement(tag);
        if (className) node.className = className;
        if (text !== undefined && text !== null) node.textContent = text;
        return node;
    }

    function icon(name) {
        return el('i', 'bi ' + name);
    }

    function getJson(url) {
        return fetch(url).then(function (r) {
            if (!r.ok) {
                var error = new Error(r.status + ' ' + r.statusText);
                error.status = r.status;
                throw error;
            }
            return r.json();
        });
    }

    function plural(n, one, many) {
        return n + ' ' + (n === 1 ? one : (many || one + 's'));
    }

    function svgUrl(key) {
        return api + '/diagram/' + encodeURIComponent(key) + '/svg';
    }

    function viewUrl(key) {
        return base + '#' + encodeURIComponent(key);
    }

    /* ---- what it is ---- */

    var COUNTS = [
        ['person', 'person', 'people'],
        ['softwareSystem', 'software system'],
        ['container', 'container'],
        ['component', 'component'],
        ['custom', 'custom element']
    ];

    function renderIntro(catalog) {
        document.getElementById('ov-name').textContent = catalog.name;
        document.title = catalog.name + ' – Overview';

        var description = document.getElementById('ov-description');
        if (catalog.description) {
            description.textContent = catalog.description;
        } else {
            description.textContent = 'This workspace has no description.';
            description.classList.add('ov-muted');
        }

        var counts = document.getElementById('ov-counts');
        COUNTS.forEach(function (c) {
            var n = catalog.elements.filter(function (e) { return e.kind === c[0]; }).length;
            if (n) counts.appendChild(el('li', null, plural(n, c[1], c[2])));
        });
        counts.appendChild(el('li', null, plural(catalog.relationshipCount, 'relationship')));
        counts.appendChild(el('li', null, plural(catalog.views.length, 'view')));
        if (catalog.decisions.length) {
            var item = el('li');
            var link = el('a', null, plural(catalog.decisions.length, 'decision'));
            link.href = base + '/decisions';
            item.appendChild(link);
            counts.appendChild(item);
        }

        // The first view the author made (the catalog is in sidebar order):
        // the picture the workspace opens with.
        var first = catalog.views[0];
        if (first) {
            document.getElementById('ov-open').href = viewUrl(first.key);
            var hero = document.getElementById('ov-hero-view');
            hero.href = viewUrl(first.key);
            hero.replaceChild(thumbnail(first, VIEW_KINDS[first.kind] || ['bi-easel'], true), hero.querySelector('.ov-thumb'));
            hero.querySelector('.ov-hero-caption').textContent = first.title;
            hero.hidden = false;
        }
    }

    /* ---- views ---- */

    // The same icons as the view name on the diagrams page.
    var VIEW_KINDS = {
        SystemLandscape: ['bi-globe2', 'Landscape'], SystemContext: ['bi-bullseye', 'System context'],
        Container: ['bi-boxes', 'Containers'], Component: ['bi-puzzle', 'Components'],
        Dynamic: ['bi-play-circle', 'Dynamic'], Deployment: ['bi-hdd-network', 'Deployment'],
        Filtered: ['bi-funnel', 'Filtered'], Image: ['bi-image', 'Image'], Custom: ['bi-easel', 'Custom']
    };

    /** A diagram thumbnail from the server's SVG renderer, or the view's
     *  icon where that renderer can't draw the view. */
    function thumbnail(view, kind, eager) {
        var thumb = el('span', 'ov-thumb');
        var img = el('img');
        img.loading = eager ? 'eager' : 'lazy';
        img.alt = '';
        img.addEventListener('error', function () {
            thumb.classList.add('is-empty');
            thumb.replaceChildren(icon(kind[0]));
        });
        img.src = svgUrl(view.key);
        thumb.appendChild(img);
        return thumb;
    }

    function viewCard(view) {
        var kind = view.generated ? ['bi-stars', 'Generated'] : (VIEW_KINDS[view.kind] || ['bi-easel', view.kind]);
        var card = el('a', 'ov-card');
        card.href = viewUrl(view.key);
        card.appendChild(thumbnail(view, kind, false));
        card.appendChild(el('span', 'ov-card-title', view.title));
        var meta = el('span', 'ov-card-kind');
        meta.appendChild(icon(kind[0]));
        meta.appendChild(document.createTextNode(' ' + kind[1]));
        card.appendChild(meta);
        if (view.description) card.appendChild(el('span', 'ov-card-desc', view.description));
        card.title = view.title + (view.description ? ' — ' + view.description : '');
        return card;
    }

    function cardGrid(views) {
        var grid = el('div', 'ov-grid');
        views.forEach(function (v) { grid.appendChild(viewCard(v)); });
        return grid;
    }

    function renderViews(catalog) {
        var container = document.getElementById('ov-views');
        var own = catalog.views.filter(function (v) { return !v.generated; });
        var generated = catalog.views.filter(function (v) { return v.generated; });

        // One grid in sidebar order (C4 levels in turn); each card names its
        // type, so headings per type would only spread the grid out.
        if (own.length) container.appendChild(cardGrid(own));

        if (generated.length) {
            // Generated views are many and derived; folded away unless there
            // is nothing else, so the author's own views lead.
            var fold = el('details', 'ov-generated');
            if (!own.length) fold.open = true;
            var summary = el('summary', 'ov-group-title', 'Generated views ');
            summary.appendChild(el('span', 'ov-muted', '(' + generated.length + ')'));
            fold.appendChild(summary);
            fold.appendChild(cardGrid(generated));
            container.appendChild(fold);
        }

        if (!catalog.views.length) {
            container.appendChild(el('p', 'ov-muted', 'This workspace has no views.'));
        }
    }

    /* ---- health ---- */

    // Review finding codes (structurizr_query::review) as sentences about
    // n elements.
    var FINDINGS = {
        'missing-description': ['has no description', 'have no description'],
        'missing-technology': ['has no technology', 'have no technology'],
        'not-in-any-view': ['is in no view', 'are in no view'],
        'no-relationships': ['has no relationships', 'have no relationships'],
        'duplicate-name': ['shares its name with another element', 'share a name with another element'],
        'relationship-undescribed': ['has a relationship without a description', 'have relationships without a description']
    };

    function healthRow(iconName, tone, text, href) {
        var item = el('li', 'ov-health-row ' + tone);
        var link = el('a');
        link.href = href;
        link.appendChild(icon(iconName));
        link.appendChild(el('span', null, text));
        link.appendChild(icon('bi-chevron-right ov-chevron'));
        item.appendChild(link);
        return item;
    }

    function renderHealth(review, clusters) {
        var list = document.getElementById('ov-health');
        list.replaceChildren();

        var elements = review ? review.elements : [];
        var blocking = elements.filter(function (e) {
            return e.findings.some(function (f) { return f.blocking; });
        }).length;
        if (blocking) {
            list.appendChild(healthRow('bi-x-octagon', 'is-bad',
                plural(blocking, 'element') + (blocking === 1 ? ' fails' : ' fail') + ' validate --strict',
                base + '/review#only=blocking'));
        }

        // Count elements per finding code, most common first.
        var byCode = {};
        elements.forEach(function (e) {
            var seen = {};
            e.findings.forEach(function (f) {
                if (f.blocking || seen[f.code]) return;
                seen[f.code] = true;
                byCode[f.code] = (byCode[f.code] || 0) + 1;
            });
        });
        Object.keys(byCode).sort(function (a, b) { return byCode[b] - byCode[a]; }).forEach(function (code) {
            var n = byCode[code];
            var phrase = FINDINGS[code] ? FINDINGS[code][n === 1 ? 0 : 1] : '— ' + code;
            list.appendChild(healthRow('bi-exclamation-triangle', 'is-warn',
                plural(n, 'element') + ' ' + phrase,
                base + '/review#only=findings&q=' + encodeURIComponent(code)));
        });

        var conformance = clusters ? clusters.conformance.length : 0;
        if (conformance) {
            list.appendChild(healthRow('bi-diagram-3', 'is-info',
                plural(conformance, 'element') + ' ' + (conformance === 1 ? 'depends' : 'depend') +
                ' mostly on another group than ' + (conformance === 1 ? 'its own' : 'their own'),
                base + '/clusters'));
        }

        if (!list.children.length) {
            var ok = el('li', 'ov-health-row is-good');
            var span = el('span');
            span.appendChild(icon('bi-check-circle'));
            span.appendChild(el('span', null, review ? 'No findings. Every element is described, related and shown.' : 'Could not check the model.'));
            ok.appendChild(span);
            list.appendChild(ok);
        }
    }

    /* ---- recent changes ---- */

    var relative = window.Intl && Intl.RelativeTimeFormat ? new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' }) : null;

    function ago(iso) {
        var then = new Date(iso);
        if (!relative || isNaN(then)) return iso.slice(0, 10);
        var seconds = (then - new Date()) / 1000;
        var steps = [['year', 31536000], ['month', 2592000], ['week', 604800], ['day', 86400], ['hour', 3600], ['minute', 60]];
        for (var i = 0; i < steps.length; i++) {
            if (Math.abs(seconds) >= steps[i][1]) return relative.format(Math.round(seconds / steps[i][1]), steps[i][0]);
        }
        return relative.format(0, 'minute');
    }

    var SHOWN_CHANGES = 6;

    function renderChanges(history, error) {
        var list = document.getElementById('ov-changes');
        list.replaceChildren();
        if (!history) {
            list.appendChild(el('li', 'ov-muted', error && error.status === 409
                ? 'This workspace isn’t tracked by git, so there is no history to show.'
                : 'Could not read the history.'));
            return;
        }
        var revisions = history.revisions;
        if (history.dirty && revisions.length) {
            list.appendChild(changeRow('Uncommitted changes', 'working copy', null,
                base + '/diff?from=' + encodeURIComponent(revisions[0].sha) + '&to=working', true));
        }
        revisions.slice(0, SHOWN_CHANGES).forEach(function (rev, i) {
            // Each commit against the one before it in this file's history;
            // the first commit has nothing before it to compare with.
            var older = revisions[i + 1];
            var href = older ? base + '/diff?from=' + encodeURIComponent(older.sha) + '&to=' + encodeURIComponent(rev.sha) : null;
            list.appendChild(changeRow(rev.subject, rev.short, rev, href, false));
        });
        if (!revisions.length) list.appendChild(el('li', 'ov-muted', 'No commits yet.'));

        var more = el('li', 'ov-more');
        var link = el('a', null, 'Compare any two versions');
        link.href = base + '/diff';
        more.appendChild(link);
        list.appendChild(more);
    }

    function changeRow(subject, ref, rev, href, pending) {
        var item = el('li', 'ov-change' + (pending ? ' is-pending' : ''));
        var body = el(href ? 'a' : 'div', 'ov-change-body');
        if (href) {
            body.href = href;
            body.title = 'What this changed in the model';
        }
        body.appendChild(el('span', 'ov-change-subject', subject));
        var meta = el('span', 'ov-change-meta');
        meta.appendChild(el('code', null, ref));
        if (rev) {
            meta.appendChild(document.createTextNode(' · ' + rev.author + ' · '));
            var time = el('time', null, ago(rev.date));
            time.dateTime = rev.date;
            time.title = new Date(rev.date).toLocaleString();
            meta.appendChild(time);
        }
        body.appendChild(meta);
        item.appendChild(body);
        return item;
    }

    /* ---- load ---- */

    getJson(api + '/catalog').then(function (catalog) {
        renderIntro(catalog);
        renderViews(catalog);
    }).catch(function () {
        document.getElementById('ov-name').textContent = slug;
        document.getElementById('ov-views').appendChild(el('p', 'ov-muted', 'Could not load this workspace.'));
    });

    var review = getJson(api + '/review').catch(function () { return null; });
    var clusters = getJson(api + '/clusters?level=container').catch(function () { return null; });
    Promise.all([review, clusters]).then(function (results) { renderHealth(results[0], results[1]); });

    getJson(api + '/revisions').then(
        function (history) { renderChanges(history); },
        function (error) { renderChanges(null, error); });

    // Live reload: this page is all derived data, so reload it whole.
    (function connect() {
        var ws = new WebSocket((location.protocol === 'https:' ? 'wss' : 'ws') + '://' + location.host + '/ws');
        ws.onmessage = function (e) {
            if (JSON.parse(e.data).type === 'reload') location.reload();
        };
        ws.onclose = function () { setTimeout(connect, 2000); };
    })();
})();
