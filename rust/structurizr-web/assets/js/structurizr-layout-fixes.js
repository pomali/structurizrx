// Layout fixes applied on top of JointJS's dagre adapter, shared by every page
// that renders a diagram through structurizr.ui.Diagram (the workspace viewer,
// the single-diagram page, and the exported static viewer, which inlines the
// viewer template but copies this file alongside it).
//
// Loading this file installs the fixes; it must come after
// jointjs-DirectedGraph and before the first layout runs.
(function() {

    // dagre cannot rank an edge whose endpoint is a cluster — a relationship
    // between two deployment nodes, for instance — and throws part-way through.
    // structurizr-diagram.js swallows that, which leaves every element of the
    // view piled up at its pre-layout position. Hide those edges from the
    // layout (they are still drawn) so the rest of the diagram is placed.
    (function patchDirectedGraphLayout() {
        var layout = joint.layout.DirectedGraph.layout;
        joint.layout.DirectedGraph.layout = function(graphOrCells, options) {
            var cells = graphOrCells.getCells ? graphOrCells.getCells() : graphOrCells;
            var isCluster = {};
            cells.forEach(function(cell) {
                if (cell.isElement() && cell.getEmbeddedCells().length > 0) {
                    isCluster[cell.id] = true;
                }
            });
            var layoutable = cells.filter(function(cell) {
                if (!cell.isLink()) return true;
                var source = cell.get('source'), target = cell.get('target');
                return !(source && isCluster[source.id]) && !(target && isCluster[target.id]);
            });
            var result = layout.call(
                this,
                layoutable.length === cells.length ? graphOrCells : layoutable,
                options
            );
            reflowUnconnectedElements(cells, isCluster);
            return result;
        };
    })();

    // dagre only has edges to work with, so a view whose elements are mostly
    // unrelated (a catalogue of containers, say) comes out as one long single
    // file — hundreds of pixels wide and tens of thousands tall, illegible at
    // any zoom that fits the screen. Re-pack those unconnected elements into a
    // roughly 16:9 grid instead, leaving everything dagre actually ranked
    // where it put it.
    var GRID_MIN_ELEMENTS = 5;   // below this, dagre's single row/column is fine
    var GRID_GAP = 50;           // matches the default nodeSeparation
    var GRID_TARGET_RATIO = 16 / 9;
    var CLUSTER_PADDING = 50;    // structurizr-diagram.js uses this for dagre clusters

    function reflowUnconnectedElements(cells, isCluster) {
        var connected = {};
        cells.forEach(function(cell) {
            if (!cell.isLink()) return;
            var source = cell.get('source'), target = cell.get('target');
            if (source && source.id) connected[source.id] = true;
            if (target && target.id) connected[target.id] = true;
        });

        // Group the loose elements by the boundary they sit in, so each
        // boundary gets its own grid and elements never leave their parent.
        var groups = {};
        var siblings = {};
        cells.forEach(function(cell) {
            if (!cell.isElement() || isCluster[cell.id]) return;
            var parent = cell.get('parent') || '';
            (siblings[parent] = siblings[parent] || []).push(cell);
            if (connected[cell.id]) return;
            (groups[parent] = groups[parent] || []).push(cell);
        });

        var touchedParents = {};
        Object.keys(groups).forEach(function(parent) {
            if (layoutGrid(groups[parent], siblings[parent])) {
                if (parent) touchedParents[parent] = true;
            }
        });

        // Moved children leave their boundary the wrong size — dagre sized it
        // around the column it laid out. Re-fit each affected boundary, and
        // any boundary containing it.
        Object.keys(touchedParents).forEach(function(id) {
            for (var cell = cells.find(function(c) { return c.id === id; });
                 cell;
                 cell = cell.getParentCell && cell.getParentCell()) {
                cell.fitEmbeds({ padding: CLUSTER_PADDING });
            }
        });
    }

    // Places `group` in a grid. `siblings` is every element sharing the same
    // parent, connected ones included; the grid is anchored below them so it
    // cannot land on top of the part of the diagram dagre laid out.
    function layoutGrid(group, siblings) {
        if (group.length < GRID_MIN_ELEMENTS) return false;

        var cellW = 0, cellH = 0;
        group.forEach(function(el) {
            var size = el.size();
            cellW = Math.max(cellW, size.width);
            cellH = Math.max(cellH, size.height);
        });
        cellW += GRID_GAP;
        cellH += GRID_GAP;

        var cols = Math.max(1, Math.ceil(
            Math.sqrt(GRID_TARGET_RATIO * group.length * cellH / cellW)));
        if (cols >= group.length) return false;

        // dagre orders nodes it could not rank by their pre-layout positions,
        // which structurizr-diagram.js seeds randomly — so left to itself the
        // grid comes out in a different, meaningless order on every load.
        // Model element ids are handed out in model order, so sorting by them
        // gives a stable grid that reads in the order the elements were
        // written. (A cell's own id is a JointJS UUID, not the element id.)
        var ordered = group.slice().sort(function(a, b) {
            return elementOrder(a) - elementOrder(b);
        });

        var anchor = gridAnchor(group, siblings);
        ordered.forEach(function(el, i) {
            el.position(
                anchor.x + (i % cols) * cellW,
                anchor.y + Math.floor(i / cols) * cellH
            );
        });
        return true;
    }

    function elementOrder(cell) {
        var element = cell.get('element');
        var id = element ? Number(element.id) : NaN;
        return isNaN(id) ? Infinity : id;
    }

    function gridAnchor(group, siblings) {
        var placed = siblings.filter(function(el) { return group.indexOf(el) === -1; });
        var boxes = (placed.length ? placed : group).map(function(el) {
            return el.getBBox();
        });
        var x = Math.min.apply(null, boxes.map(function(b) { return b.x; }));
        var y = placed.length
            ? Math.max.apply(null, boxes.map(function(b) { return b.y + b.height; })) + GRID_GAP
            : Math.min.apply(null, boxes.map(function(b) { return b.y; }));
        return { x: x, y: y };
    }

})();
