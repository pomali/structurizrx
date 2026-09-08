/*
 * Print document builder for /workspace/{name}/print.
 *
 * Prints the *rendered* diagram, not the server-side SVG: the two use
 * different layout engines (the server does its own Sugiyama pass, the viewer
 * uses JointJS + dagre), so only the client-side render matches what the
 * reader sees on screen.
 *
 * One diagram instance lives offscreen in #print-stage. Each view is switched
 * into it in turn and serialised with exportCurrentDiagramToSVG(), which
 * already crops to content, drops interaction chrome and inlines icons as data
 * URLs. The serialised markup is appended to #print-doc as one sheet per view.
 *
 * The document is built before any print is triggered — exporting is
 * callback-async and re-renders once per view, so printing on load would
 * capture a half-built document.
 */
(function () {
    'use strict';

    var slug = window.PRINT_WORKSPACE_SLUG;

    /* Sheet geometry, in mm. */
    var PAPER = {
        a4: { name: 'A4', portrait: [210, 297], landscape: [297, 210] },
        letter: { name: 'Letter', portrait: [216, 279], landscape: [279, 216] }
    };

    var SHEET_MARGIN = 14;
    var MM_TO_PX = 96 / 25.4;

    var state = {
        workspace: null,
        diagram: null,
        figures: [],
        built: false
    };

    function el(id) {
        return document.getElementById(id);
    }

    function status(text) {
        el('print-status').textContent = text;
    }

    function options() {
        return {
            paper: el('opt-paper').value,
            orientation: el('opt-orientation').value,
            legend: el('opt-legend').checked,
            toc: el('opt-toc').checked,
            greyscale: el('opt-greyscale').checked
        };
    }

    /* ---- page geometry ---- */

    /*
     * @page cannot read CSS custom properties, so the paper size and the two
     * named page rules used for per-view orientation are injected as real CSS
     * whenever the paper selection changes.
     */
    function applyPageRules(opts) {
        var paper = PAPER[opts.paper];
        var css =
            '@page { size: ' + paper.name + ' portrait; margin: ' + SHEET_MARGIN + 'mm; }\n' +
            '@page print-portrait { size: ' + paper.name + ' portrait; margin: ' + SHEET_MARGIN + 'mm; }\n' +
            '@page print-landscape { size: ' + paper.name + ' landscape; margin: ' + SHEET_MARGIN + 'mm; }\n' +
            '.print-sheet.orientation-portrait { page: print-portrait; }\n' +
            '.print-sheet.orientation-landscape { page: print-landscape; }\n';
        el('print-page-rules').textContent = css;
    }

    /*
     * Per-sheet CSS variables. The content box is stated explicitly because
     * printing drops the sheet's padding in favour of the @page margin, and
     * both paths have to end up with the same box that fitFigures() measured.
     */
    function sheetStyle(orientation, opts) {
        var dims = PAPER[opts.paper][orientation];

        return (
            '--sheet-width: ' + dims[0] + 'mm;' +
            '--sheet-height: ' + dims[1] + 'mm;' +
            '--sheet-margin: ' + SHEET_MARGIN + 'mm;' +
            '--content-width: ' + (dims[0] - 2 * SHEET_MARGIN) + 'mm;' +
            '--content-height: ' + (dims[1] - 2 * SHEET_MARGIN) + 'mm;'
        );
    }

    function contentBoxPx(orientation, opts) {
        var dims = PAPER[opts.paper][orientation];
        return {
            width: (dims[0] - 2 * SHEET_MARGIN) * MM_TO_PX,
            height: (dims[1] - 2 * SHEET_MARGIN) * MM_TO_PX
        };
    }

    function outerHeight(node) {
        var style = window.getComputedStyle(node);
        return (
            node.getBoundingClientRect().height +
            parseFloat(style.marginTop) +
            parseFloat(style.marginBottom)
        );
    }

    /*
     * Second pass: scale each diagram to the space its own sheet has left once
     * title, description, legend and footer have been laid out. Measuring beats
     * budgeting a fixed allowance, and it is exact for every paper size and
     * orientation.
     */
    function fitFigures(opts) {
        [].forEach.call(document.querySelectorAll('.print-figure'), function (figure) {
            var svg = figure.querySelector('.print-figure-diagram svg');
            if (!svg) {
                return;
            }

            var intrinsicWidth = parseFloat(svg.getAttribute('width'));
            var intrinsicHeight = parseFloat(svg.getAttribute('height'));
            if (!intrinsicWidth || !intrinsicHeight) {
                return;
            }

            var orientation = figure.classList.contains('orientation-landscape') ? 'landscape' : 'portrait';
            var box = contentBoxPx(orientation, opts);

            var used = 0;
            [].forEach.call(figure.children, function (child) {
                if (!child.classList.contains('print-figure-diagram')) {
                    used += outerHeight(child);
                }
            });

            var availableHeight = Math.max(box.height - used, 40);
            var scale = Math.min(box.width / intrinsicWidth, availableHeight / intrinsicHeight);

            svg.style.width = intrinsicWidth * scale + 'px';
            svg.style.height = intrinsicHeight * scale + 'px';
        });
    }

    /*
     * Orientation per view, from the aspect ratio of the *cropped* export
     * rather than the view's nominal page size — the crop is what actually
     * gets printed.
     */
    function orientationFor(figure, opts) {
        if (opts.orientation !== 'auto') {
            return opts.orientation;
        }
        if (!figure.width || !figure.height) {
            return 'portrait';
        }
        return figure.width / figure.height > 1.15 ? 'landscape' : 'portrait';
    }

    /* ---- document assembly ---- */

    function sheet(orientation, opts, className) {
        var section = document.createElement('section');
        section.className = 'print-sheet orientation-' + orientation + (className ? ' ' + className : '');
        section.setAttribute('style', sheetStyle(orientation, opts));
        return section;
    }

    function coverSheet(opts, pageCount) {
        var ws = state.workspace;
        var section = sheet('portrait', opts, 'print-cover');

        var title = document.createElement('h1');
        title.className = 'print-cover-title';
        title.textContent = ws.name || slug;
        section.appendChild(title);

        if (ws.description) {
            var desc = document.createElement('p');
            desc.className = 'print-cover-desc';
            desc.textContent = ws.description;
            section.appendChild(desc);
        }

        var meta = document.createElement('div');
        meta.className = 'print-cover-meta';
        meta.appendChild(metaLine('Views', String(state.figures.length)));
        if (ws.version) {
            meta.appendChild(metaLine('Version', String(ws.version)));
        }
        if (ws.lastModifiedDate) {
            meta.appendChild(metaLine('Last modified', formatDate(ws.lastModifiedDate)));
        }
        meta.appendChild(metaLine('Printed', formatDate(new Date().toISOString())));
        meta.appendChild(metaLine('Pages', String(pageCount)));
        section.appendChild(meta);

        return section;
    }

    function metaLine(label, value) {
        var div = document.createElement('div');
        div.textContent = label + ': ' + value;
        return div;
    }

    function formatDate(iso) {
        var d = new Date(iso);
        return isNaN(d.getTime()) ? iso : d.toISOString().slice(0, 10);
    }

    function tocSheet(opts, firstFigurePage) {
        var section = sheet('portrait', opts, 'print-toc-sheet');

        var heading = document.createElement('h2');
        heading.className = 'print-toc-heading';
        heading.textContent = 'Contents';
        section.appendChild(heading);

        var list = document.createElement('ol');
        list.className = 'print-toc';

        state.figures.forEach(function (figure, index) {
            var li = document.createElement('li');

            var title = document.createElement('span');
            title.className = 'toc-title';
            title.textContent = figure.title;
            li.appendChild(title);

            var type = document.createElement('span');
            type.className = 'toc-type';
            type.textContent = figure.type + ' · #' + figure.key;
            li.appendChild(type);

            var fill = document.createElement('span');
            fill.className = 'toc-fill';
            li.appendChild(fill);

            var page = document.createElement('span');
            page.className = 'toc-page';
            page.textContent = String(firstFigurePage + index);
            li.appendChild(page);

            list.appendChild(li);
        });

        section.appendChild(list);
        return section;
    }

    function figureSheet(figure, opts, pageNumber, pageCount) {
        var orientation = orientationFor(figure, opts);
        var section = sheet(orientation, opts, 'print-figure');

        var title = document.createElement('h2');
        title.className = 'print-figure-title';
        title.textContent = figure.title;
        section.appendChild(title);

        if (figure.description) {
            var desc = document.createElement('p');
            desc.className = 'print-figure-desc';
            desc.textContent = figure.description;
            section.appendChild(desc);
        }

        var body = document.createElement('div');
        body.className = 'print-figure-diagram';
        if (figure.error) {
            body.innerHTML = '';
            var err = document.createElement('p');
            err.className = 'print-figure-error';
            err.textContent = 'Could not render this view: ' + figure.error;
            body.appendChild(err);
        } else {
            body.innerHTML = figure.svg;
        }
        section.appendChild(body);

        if (opts.legend && figure.legendSvg) {
            var legend = document.createElement('div');
            legend.className = 'print-figure-legend';
            legend.innerHTML = figure.legendSvg;
            section.appendChild(legend);
        }

        var footer = document.createElement('footer');
        footer.className = 'print-figure-footer';
        footer.appendChild(span(figure.type + ' · #' + figure.key));
        footer.appendChild(span(footerSource()));
        var fill = document.createElement('span');
        fill.className = 'footer-fill';
        footer.appendChild(fill);
        footer.appendChild(span('Page ' + pageNumber + ' of ' + pageCount, 'footer-page'));
        section.appendChild(footer);

        return section;
    }

    function span(text, className) {
        var s = document.createElement('span');
        if (className) {
            s.className = className;
        }
        s.textContent = text;
        return s;
    }

    function footerSource() {
        var ws = state.workspace;
        var parts = [ws.name || slug];
        if (ws.version) {
            parts.push('v' + ws.version);
        }
        parts.push(formatDate(new Date().toISOString()));
        return parts.join(' · ');
    }

    /*
     * Page numbers are computed rather than generated by the browser: @page
     * margin boxes (@bottom-center { content: counter(page) }) are unsupported
     * in Chrome, Safari and Firefox. This is only accurate while every sheet
     * really is one printed page, which the fit-to-page height cap enforces.
     */
    function rebuild() {
        var opts = options();
        applyPageRules(opts);
        document.body.classList.toggle('print-greyscale', opts.greyscale);

        var doc = el('print-doc');
        doc.innerHTML = '';
        doc.setAttribute('style', '--sheet-width: ' + PAPER[opts.paper].portrait[0] + 'mm;');

        var frontMatter = 1 + (opts.toc ? 1 : 0);
        var pageCount = frontMatter + state.figures.length;

        doc.appendChild(coverSheet(opts, pageCount));
        if (opts.toc) {
            doc.appendChild(tocSheet(opts, frontMatter + 1));
        }

        state.figures.forEach(function (figure, index) {
            doc.appendChild(figureSheet(figure, opts, frontMatter + 1 + index, pageCount));
        });

        fitFigures(opts);

        status(state.figures.length + ' view(s) · ' + pageCount + ' page(s)');
    }

    /* ---- capture ---- */

    function typeOf(view) {
        if (view.type === structurizr.constants.FILTERED_VIEW_TYPE) {
            var base = state.workspace.findViewByKey(view.baseViewKey);
            return base ? typeOf(base) + ' (filtered)' : 'Filtered';
        }
        return view.type;
    }

    /*
     * Capture views one at a time. changeView() re-renders the diagram, so the
     * next view cannot be started until its callback has fired; the yield to
     * setTimeout keeps the progress text repainting between views.
     */
    function captureViews(views, index, done) {
        if (index >= views.length) {
            done();
            return;
        }

        var view = views[index];
        status('Rendering ' + (index + 1) + ' of ' + views.length + ': ' + structurizr.ui.getTitleForView(view));

        state.diagram.changeView(view.key, function () {
            var figure = {
                key: view.key,
                title: structurizr.ui.getTitleForView(view),
                description: view.description,
                type: typeOf(view)
            };

            try {
                var exported = state.diagram.exportCurrentDiagramToSVG({ metadata: true, crop: true });
                figure.svg = exported.markup;
                figure.width = exported.width;
                figure.height = exported.height;
                figure.legendSvg = state.diagram.exportCurrentDiagramKeyToSVG();
            } catch (e) {
                figure.error = e.message;
            }

            state.figures.push(figure);
            setTimeout(function () {
                captureViews(views, index + 1, done);
            }, 0);
        });
    }

    function finish() {
        state.built = true;
        el('print-button').disabled = false;
        ['opt-paper', 'opt-orientation', 'opt-legend', 'opt-toc', 'opt-greyscale'].forEach(function (id) {
            el(id).disabled = false;
            el(id).addEventListener('change', rebuild);
        });

        rebuild();

        if (new URLSearchParams(location.search).get('autoprint') === '1') {
            window.print();
        }
    }

    function fail(message) {
        status('');
        el('print-doc').innerHTML =
            '<section class="print-sheet"><p class="print-figure-error">' + message + '</p></section>';
    }

    function build() {
        status('Loading workspace…');

        fetch('/api/workspace/' + encodeURIComponent(slug))
            .then(function (r) {
                if (!r.ok) {
                    throw new Error('workspace request failed (' + r.status + ')');
                }
                return r.json();
            })
            .then(function (json) {
                state.workspace = structurizr.workspace = new structurizr.Workspace(json);

                /* getViews() is already sorted by software system and C4 view
                   type, which is the order a printed document wants. */
                var views = state.workspace.getViews();
                if (!views.length) {
                    fail('This workspace has no views to print.');
                    return;
                }

                structurizr.ui.themes = [];
                structurizr.ui.loadThemes(function () {
                    state.diagram = new structurizr.ui.Diagram('print-stage', false, function () {
                        /* Print is always light: the exported SVG carries an
                           inline background from the diagram's canvas colour. */
                        state.diagram.setDarkMode(false);
                        state.diagram.setEmbedded(true);

                        var stage = el('print-stage');
                        state.diagram.getPossibleViewportWidth = function () {
                            return stage.clientWidth;
                        };
                        state.diagram.getPossibleViewportHeight = function () {
                            return stage.clientHeight;
                        };

                        captureViews(views, 0, finish);
                    });
                });
            })
            .catch(function (e) {
                fail('Error: ' + e.message);
            });
    }

    el('print-button').addEventListener('click', function () {
        if (state.built) {
            window.print();
        }
    });

    el('rebuild-button').addEventListener('click', function () {
        location.reload();
    });

    /*
     * The document is a snapshot: it is captured once, view by view, and never
     * re-captured on its own. Rebuilding underneath someone who is part-way
     * through a print dialog would be worse than going stale, so an edit to the
     * workspace only offers a rebuild.
     */
    function connectWs() {
        var proto = location.protocol === 'https:' ? 'wss' : 'ws';
        var ws = new WebSocket(proto + '://' + location.host + '/ws');
        ws.onmessage = function (e) {
            if (JSON.parse(e.data).type === 'reload') {
                el('rebuild-button').hidden = false;
                status('Workspace changed — this document is out of date');
            }
        };
        ws.onclose = function () {
            setTimeout(connectWs, 2000);
        };
    }

    connectWs();
    build();
})();
