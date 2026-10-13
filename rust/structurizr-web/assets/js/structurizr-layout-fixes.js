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
            var engine = structurizr.ui.getLayoutEngine();
            if (engine !== structurizr.ui.LAYOUT_ENGINE_DAGRE && structurizr.elkLayout) {
                return structurizr.elkLayout.layout(graphOrCells, options || {}, engine);
            }

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
            if (options && options.setLinkVertices) {
                routeThroughChannels(cells, isCluster, options.rankDir || 'TB');
            }
            return result;
        };
    })();

    // dagre hands back an edge's waypoints without its end points, and the
    // last waypoint can sit level with the source rather than the target. The
    // final leg then runs diagonally to the target across every box stacked
    // in between. Instead, leave the source straight into the gap after its
    // rank, travel along that gap, and enter the target straight from the gap
    // before its rank, so the legs that change level stay between the columns.
    var CHANNEL_INSET = 24;     // how far into the gap from the column edge
    var LEVEL_TOLERANCE = 4;    // waypoints this close in level count as level

    // Coordinates with u along the rank direction and v across it.
    var AXES = {
        TB: { toUV: function(p) { return { u: p.y, v: p.x }; },  fromUV: function(u, v) { return { x: v, y: u }; } },
        BT: { toUV: function(p) { return { u: -p.y, v: p.x }; }, fromUV: function(u, v) { return { x: v, y: -u }; } },
        LR: { toUV: function(p) { return { u: p.x, v: p.y }; },  fromUV: function(u, v) { return { x: u, y: v }; } },
        RL: { toUV: function(p) { return { u: -p.x, v: p.y }; }, fromUV: function(u, v) { return { x: -u, y: v }; } }
    };

    function routeThroughChannels(cells, isCluster, rankDir) {
        var axes = AXES[rankDir] || AXES.TB;
        var byId = {};
        var boxes = [];
        cells.forEach(function(cell) {
            if (!cell.isElement() || isCluster[cell.id]) return;
            var b = cell.getBBox();
            var a = axes.toUV({ x: b.x, y: b.y });
            var c = axes.toUV({ x: b.x + b.width, y: b.y + b.height });
            var box = {
                u0: Math.min(a.u, c.u), u1: Math.max(a.u, c.u),
                v0: Math.min(a.v, c.v), v1: Math.max(a.v, c.v)
            };
            box.cu = (box.u0 + box.u1) / 2;
            box.cv = (box.v0 + box.v1) / 2;
            byId[cell.id] = box;
            boxes.push(box);
        });

        // dagre centres every node of a rank on the same line, so a rank's
        // column runs from its widest node's near edge to its far edge.
        function column(box) {
            var u0 = box.u0, u1 = box.u1;
            boxes.forEach(function(other) {
                if (Math.abs(other.cu - box.cu) < 2) {
                    u0 = Math.min(u0, other.u0);
                    u1 = Math.max(u1, other.u1);
                }
            });
            return { u0: u0, u1: u1 };
        }

        cells.forEach(function(link) {
            if (!link.isLink()) return;
            var source = byId[(link.get('source') || {}).id];
            var target = byId[(link.get('target') || {}).id];
            if (!source || !target || source === target) return;

            // Only edges running with the rank direction; dagre reverses the
            // others internally and their waypoints are already its detour.
            var exitU = column(source).u1 + CHANNEL_INSET;
            var entryU = column(target).u0 - CHANNEL_INSET;
            if (entryU <= exitU) return;

            var points = (link.get('vertices') || []).map(axes.toUV);
            var route = [];

            var first = points.length ? points[0] : { u: entryU, v: target.cv };
            route.push({ u: exitU, v: source.cv });
            if (Math.abs(first.v - source.cv) > LEVEL_TOLERANCE) {
                route.push({ u: exitU, v: first.v });
            }
            points.forEach(function(p) {
                if (p.u > exitU && p.u < entryU) route.push(p);
            });
            var last = route[route.length - 1];
            if (Math.abs(last.v - target.cv) > LEVEL_TOLERANCE) {
                route.push({ u: entryU, v: last.v });
            }
            route.push({ u: entryU, v: target.cv });

            link.set('vertices', route.map(function(p) { return axes.fromUV(p.u, p.v); }));
        });
    }

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
