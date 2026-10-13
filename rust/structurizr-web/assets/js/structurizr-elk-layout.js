// Auto-layout with ELK's layered algorithm (elkjs), as an alternative to
// dagre. structurizr-layout-fixes.js calls structurizr.elkLayout.layout() in
// place of joint.layout.DirectedGraph.layout() when the viewer has picked an
// ELK engine (structurizr.ui.getLayoutEngine()).
//
// Unlike dagre, ELK places nodes and routes edges in one pass: edges go
// around boxes, nested boundaries are laid out as real compound nodes, and
// edge labels get space reserved for them.
//
// elkjs only offers a Promise API, but its in-page worker runs the layout
// synchronously inside its dispatcher and writes the result into the graph
// object it was handed. Calling the dispatcher directly keeps this a drop-in
// replacement for the synchronous dagre call that structurizr-diagram.js
// makes (and then immediately sizes and centres the page around).
(function() {

    var elk = null;

    function dispatcher() {
        if (elk === null) {
            elk = new ELK({ algorithms: ['layered'] });
            elk.worker.worker.dispatcher.dispatch({ data: { id: 'structurizr-register', cmd: 'register', algorithms: ['layered'] } });
        }
        return elk.worker.worker.dispatcher;
    }

    var DIRECTIONS = { TB: 'DOWN', BT: 'UP', LR: 'RIGHT', RL: 'LEFT' };
    var NOT_LAID_OUT = ['structurizr.diagramTitle', 'structurizr.diagramMetadata'];

    // ELK's spline sections come back as cubic Bézier control points: start,
    // then (control, control, end) per segment. Draw them as such; anything
    // else (a link the user has since edited) falls back to rounded corners.
    joint.connectors.elkSpline = function(sourcePoint, targetPoint, route, opt) {
        var points = [sourcePoint].concat(route || [], [targetPoint]);
        if ((points.length - 1) % 3 !== 0) {
            return joint.connectors.rounded(sourcePoint, targetPoint, route, opt);
        }
        var d = 'M ' + points[0].x + ' ' + points[0].y;
        for (var i = 1; i < points.length; i += 3) {
            d += ' C ' + points[i].x + ' ' + points[i].y +
                 ' ' + points[i + 1].x + ' ' + points[i + 1].y +
                 ' ' + points[i + 2].x + ' ' + points[i + 2].y;
        }
        var path = new joint.g.Path(d);
        return (opt && opt.raw) ? path : path.serialize();
    };

    function layoutOptions(options, splines) {
        var edgeSep = options.edgeSep || 50;
        return {
            'elk.algorithm': 'layered',
            'elk.direction': DIRECTIONS[options.rankDir] || 'DOWN',
            'elk.hierarchyHandling': 'INCLUDE_CHILDREN',
            'elk.json.shapeCoords': 'ROOT',
            'elk.json.edgeCoords': 'ROOT',
            'elk.edgeRouting': splines ? 'SPLINES' : 'ORTHOGONAL',
            'elk.layered.spacing.nodeNodeBetweenLayers': String(options.rankSep || 100),
            'elk.spacing.nodeNode': String(options.nodeSep || 50),
            'elk.spacing.componentComponent': String(options.nodeSep || 50),
            'elk.spacing.edgeNode': String(Math.round(edgeSep / 2)),
            'elk.spacing.edgeEdge': String(Math.round(edgeSep / 3)),
            'elk.layered.spacing.edgeNodeBetweenLayers': String(Math.round(edgeSep / 2)),
            'elk.layered.spacing.edgeEdgeBetweenLayers': String(Math.round(edgeSep / 3)),
            'elk.spacing.edgeLabel': '4',
            'elk.edgeLabels.placement': 'CENTER',
            'elk.layered.nodePlacement.strategy': 'NETWORK_SIMPLEX'
            // Not elk.layered.considerModelOrder: with nested boundaries it
            // throws inside ELK (0.11.1). ELK is deterministic without it
            // (fixed random seed), so the same view still lays out the same.
        };
    }

    // structurizr-diagram.js re-fits every boundary around its contents
    // after layout (reposition()), leaving room under them for the
    // boundary's name and metadata. Reserve the same room here, or stacked
    // boundaries overlap once they have been re-fitted.
    var LABEL_MARGIN = 15;  // `margin` in reposition()

    function padding(cell, options) {
        var p = options.clusterPadding || {};
        var bottom = (p.bottom || 50) + LABEL_MARGIN + fontSize(cell, '.structurizrName') + LABEL_MARGIN;
        var metadata = cell.attr('.structurizrMetaData/text');
        if (metadata && metadata.length > 0) {
            bottom += fontSize(cell, '.structurizrMetaData');
        }
        return '[top=' + (p.top || 50) + ',left=' + (p.left || 50) +
            ',bottom=' + bottom + ',right=' + (p.right || 50) + ']';
    }

    function fontSize(cell, selector) {
        return parseInt(cell.attr(selector + '/font-size')) || 0;
    }

    function layout(graphOrCells, options, engine) {
        var cells = graphOrCells.getCells ? graphOrCells.getCells() : graphOrCells;
        var splines = engine === structurizr.ui.LAYOUT_ENGINE_ELK_SPLINES;
        var rootOptions = layoutOptions(options, splines);

        var elements = cells.filter(function(cell) {
            return cell.isElement() && NOT_LAID_OUT.indexOf(cell.get('type')) === -1;
        });
        var nodes = {};
        elements.forEach(function(cell) {
            var size = cell.size();
            nodes[cell.id] = { id: String(cell.id), width: size.width, height: size.height, children: [] };
        });

        var root = { id: 'root', layoutOptions: rootOptions, children: [], edges: [] };
        elements.forEach(function(cell) {
            var parent = nodes[cell.get('parent')];
            (parent ? parent.children : root.children).push(nodes[cell.id]);
        });
        elements.forEach(function(cell) {
            var node = nodes[cell.id];
            if (node.children.length > 0) {
                // ELK sizes compound nodes itself; only padding is ours.
                delete node.width;
                delete node.height;
                node.layoutOptions = Object.assign({}, rootOptions, { 'elk.padding': padding(cell, options) });
            }
        });

        var links = cells.filter(function(cell) { return cell.isLink(); });
        var laidOut = {};
        links.forEach(function(link) {
            var source = (link.get('source') || {}).id;
            var target = (link.get('target') || {}).id;
            if (!nodes[source] || !nodes[target] || source === target) return;
            var edge = { id: String(link.id), sources: [String(source)], targets: [String(target)] };
            var labelSize = link.get('labelSize');
            if (labelSize) {
                edge.labels = [{ id: link.id + '-label', text: ' ', width: labelSize.width, height: labelSize.height }];
            }
            root.edges.push(edge);
            laidOut[link.id] = edge;
        });

        dispatcher().dispatch({ data: { id: 'structurizr-layout', cmd: 'layout', graph: root, layoutOptions: {}, options: {} } });

        // Parents before children, so a boundary's new size never pushes its
        // contents about after they have been placed.
        elements.slice().sort(function(a, b) {
            return depth(a) - depth(b);
        }).forEach(function(cell) {
            var node = nodes[cell.id];
            cell.set('position', { x: node.x, y: node.y });
            if (node.children.length > 0) {
                cell.set('size', { width: node.width, height: node.height });
            }
        });

        links.forEach(function(link) {
            var edge = laidOut[link.id];
            if (!edge || !edge.sections || edge.sections.length === 0) {
                link.set('vertices', []);
                return;
            }
            var section = edge.sections[0];
            var bends = section.bendPoints || [];
            anchor(link, 'source', section.startPoint);
            anchor(link, 'target', section.endPoint);
            link.set('vertices', bends.map(function(p) { return { x: p.x, y: p.y }; }));

            if (link._defaultRouting) {
                if (splines) {
                    link.connector('elkSpline');
                } else {
                    link.connector('rounded', { radius: 12 });
                }
            }

            if (edge.labels && edge.labels[0] && edge.labels[0].x !== undefined) {
                var l = edge.labels[0];
                var ratio = ratioAlong([section.startPoint].concat(bends, [section.endPoint]),
                    { x: l.x + l.width / 2, y: l.y + l.height / 2 });
                var labels = link.get('labels') || [];
                labels.forEach(function(label) {
                    if (label.position) label.position.distance = ratio;
                });
                link.set('labels', labels.slice());
            }
        });
    }

    function depth(cell) {
        var d = 0;
        for (var parent = cell.getParentCell(); parent; parent = parent.getParentCell()) d++;
        return d;
    }

    // Pin the link's end to where ELK routed it, as an offset from the
    // element's corner so it follows the element if it is moved.
    function anchor(link, end, point) {
        var ref = link.get(end);
        var cell = link.graph.getCell(ref.id);
        if (!cell || !point) return;
        var position = cell.position();
        link.set(end, Object.assign({}, ref, {
            anchor: { name: 'topLeft', args: { dx: point.x - position.x, dy: point.y - position.y } }
        }));
    }

    // How far along a polyline (0..1) its closest point to `p` lies.
    function ratioAlong(points, p) {
        var total = 0, best = Infinity, bestAt = 0.5;
        var lengths = [];
        for (var i = 0; i + 1 < points.length; i++) {
            var a = points[i], b = points[i + 1];
            lengths.push(Math.hypot(b.x - a.x, b.y - a.y));
            total += lengths[i];
        }
        if (total === 0) return 0.5;
        var walked = 0;
        for (var j = 0; j + 1 < points.length; j++) {
            var s = points[j], e = points[j + 1], len = lengths[j];
            var t = len === 0 ? 0 : Math.max(0, Math.min(1,
                ((p.x - s.x) * (e.x - s.x) + (p.y - s.y) * (e.y - s.y)) / (len * len)));
            var q = { x: s.x + t * (e.x - s.x), y: s.y + t * (e.y - s.y) };
            var dist = Math.hypot(p.x - q.x, p.y - q.y);
            if (dist < best) {
                best = dist;
                bestAt = (walked + t * len) / total;
            }
            walked += len;
        }
        return Math.max(0.02, Math.min(0.98, bestAt));
    }

    window.structurizr = window.structurizr || {};
    structurizr.elkLayout = { layout: layout };
})();
