/*
 * Print this view: one diagram on one page, for the diagram viewer pages.
 * (Not to be confused with structurizr-print.js, which builds the multi-view
 * print document at /workspace/{name}/print.)
 *
 * Exports the current diagram to a standalone, content-cropped SVG and hands
 * it to the browser's print dialog, sized to A4 or A3 paper. Works with
 * structurizr-print-view.css, which swaps the app chrome for the exported
 * SVG under @media print.
 *
 * Usage:
 *   structurizrPrint.init({
 *       getDiagram: function() { return diagram; },   // required
 *       getCaption: function() { return {title, description, metadata}; },
 *       refit: function() { ... }                     // re-fit after a dark-mode round-trip
 *   });
 *   structurizrPrint.print('A4', 'landscape');        // orientation optional (auto)
 *
 * init() also hooks beforeprint, so a plain Ctrl/Cmd+P prints the diagram
 * with the last used paper size and an orientation matching the diagram.
 */
var structurizrPrint = (function() {

    var PAPER_SIZES = ['A4', 'A3'];
    var DEFAULT_PAPER = 'A4';
    var STORAGE_KEY = 'structurizr/print/paperSize';

    var getDiagram = function() { return undefined; };
    var getCaption = function() { return undefined; };
    var refit = function() {};

    var container = null;
    var pageStyle = null;
    var prepared = false;

    function init(options) {
        options = options || {};
        if (options.getDiagram) getDiagram = options.getDiagram;
        if (options.getCaption) getCaption = options.getCaption;
        if (options.refit) refit = options.refit;

        container = document.createElement('div');
        container.id = 'printContainer';
        document.body.appendChild(container);

        // Holds the dynamic "@page { size: ... }" rule.
        pageStyle = document.createElement('style');
        document.head.appendChild(pageStyle);

        // Ctrl/Cmd+P (or File → Print). print() below also ends up here via
        // window.print(), hence the guard.
        window.addEventListener('beforeprint', function() {
            if (!prepared) prepare(lastPaperSize(), undefined);
        });
        window.addEventListener('afterprint', function() {
            prepared = false;
            document.body.classList.remove('structurizrPrintReady');
            container.innerHTML = '';
        });
    }

    function lastPaperSize() {
        try {
            var stored = localStorage.getItem(STORAGE_KEY);
            if (PAPER_SIZES.indexOf(stored) !== -1) return stored;
        } catch (e) { /* storage unavailable */ }
        return DEFAULT_PAPER;
    }

    function rememberPaperSize(paperSize) {
        try { localStorage.setItem(STORAGE_KEY, paperSize); } catch (e) { /* ignore */ }
    }

    // Export the current view and stage it for printing. Returns false when
    // there is nothing to print (no diagram yet). With orientation undefined,
    // it follows the diagram's aspect ratio.
    function prepare(paperSize, orientation) {
        var diagram = getDiagram();
        if (!diagram) return false;

        // Paper wants a light diagram: round-trip through light mode for the
        // export. setDarkMode() re-renders synchronously but discards the
        // zoom, so the page's refit callback restores it.
        var wasDark = diagram.isDarkMode && diagram.isDarkMode() === true;
        if (wasDark) diagram.setDarkMode(false);
        // metadata:false hides the on-canvas title/description/date block: it
        // sits at the page edge, outside the content area the crop keeps, so
        // it would print clipped. The HTML caption replaces it.
        var exported;
        try {
            exported = diagram.exportCurrentDiagramToSVG({ metadata: false, crop: true });
        } catch (e) {
            // e.g. printing before the first view has rendered — fall back to
            // the browser's default rendering of the page.
            console.error('Print export failed:', e);
            exported = undefined;
        } finally {
            if (wasDark) {
                diagram.setDarkMode(true);
                refit();
            }
        }
        if (!exported || !exported.markup) return false;

        if (orientation !== 'portrait' && orientation !== 'landscape') {
            orientation = exported.width >= exported.height ? 'landscape' : 'portrait';
        }
        pageStyle.textContent = '@page { size: ' + paperSize + ' ' + orientation + '; margin: 10mm; }';

        container.innerHTML = '';
        var caption = getCaption();
        if (caption && (caption.title || caption.description || caption.metadata)) {
            var captionEl = document.createElement('div');
            captionEl.className = 'printCaption';
            [['printCaptionTitle', caption.title],
             ['printCaptionDescription', caption.description],
             ['printCaptionMetadata', caption.metadata]].forEach(function(line) {
                if (!line[1]) return;
                var el = document.createElement('div');
                el.className = line[0];
                el.textContent = line[1];
                captionEl.appendChild(el);
            });
            container.appendChild(captionEl);
        }

        var diagramEl = document.createElement('div');
        diagramEl.className = 'printDiagram';
        diagramEl.innerHTML = exported.markup;
        container.appendChild(diagramEl);

        document.body.classList.add('structurizrPrintReady');
        prepared = true;
        return true;
    }

    // Both arguments are optional: print() alone reuses the last paper size
    // and matches the orientation to the diagram.
    function print(paperSize, orientation) {
        if (PAPER_SIZES.indexOf(paperSize) === -1) paperSize = lastPaperSize();
        if (!prepare(paperSize, orientation)) return;
        rememberPaperSize(paperSize);
        window.print();
    }

    return {
        init: init,
        print: print,
        lastPaperSize: lastPaperSize,
        paperSizes: PAPER_SIZES
    };
})();
