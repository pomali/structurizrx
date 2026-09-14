/*
 * structurizr-universe-graph.js — force-directed "universe" view of a whole
 * workspace, in the spirit of Obsidian's graph view.
 *
 * Standalone: no external libraries, no build step. The host page fetches
 * /api/workspace/{name}/graph and hands the payload to setData().
 *
 * The simulation is a velocity-Verlet integrator with three forces:
 *   • link springs      — pull connected nodes to a target distance
 *   • local repulsion   — Coulomb-style, evaluated only between nodes in
 *                         neighbouring cells of a spatial hash, so cost stays
 *                         near O(n) instead of O(n²) on large workspaces
 *   • centering         — a weak pull to the origin that keeps disconnected
 *                         components from drifting away
 * Alpha (the global "temperature") decays each tick and is reheated whenever
 * the data, the filters or a dragged node change the layout.
 */
(function (global) {
    'use strict';

    var KIND_COLORS = {
        person:                 '#f0a35e',
        softwareSystem:         '#5aa9f0',
        container:              '#5fc9a0',
        component:              '#b98cf0',
        custom:                 '#9aa5b1',
        deploymentNode:         '#f2c14e',
        infrastructureNode:     '#e58fb8',
        containerInstance:      '#8fb8d8',
        softwareSystemInstance: '#8fb8d8',
        view:                   '#ef7a70',
        decision:               '#e0c34a',
        section:                '#8f97a8'
    };

    var LINK_STYLES = {
        relationship: { dash: [],      width: 1.2, arrow: true  },
        containment:  { dash: [],      width: 1.0, arrow: false },
        instance:     { dash: [4, 3],  width: 1.0, arrow: true  },
        membership:   { dash: [2, 4],  width: 0.8, arrow: false },
        documents:    { dash: [2, 4],  width: 0.8, arrow: false }
    };

    function UniverseGraph(canvas, options) {
        this.canvas = canvas;
        this.ctx = canvas.getContext('2d');
        this.nodes = [];
        this.links = [];
        this.adjacency = {};        // node id → array of neighbour ids
        this.byId = {};
        this.visibleNodes = [];
        this.visibleLinks = [];

        this.settings = Object.assign({
            centerStrength: 0.05,
            repelStrength: 2400,
            linkStrength: 0.35,
            linkDistance: 120,
            nodeSize: 1,
            showLabels: true,
            showArrows: true,
            labelZoomThreshold: 0.55
        }, options && options.settings);

        this.filters = {
            kinds: null,            // null = every kind
            linkClasses: null,
            search: '',
            focusId: null,
            focusDepth: 1
        };

        this.transform = { k: 1, x: 0, y: 0 };
        this.alpha = 1;
        this.hovered = null;
        this.hoveredLink = null;
        this.selectedNodes = {};    // node id → true
        this.selectedLinks = {};    // link id → true
        this.dragging = null;
        // onSelect(nodeOrLink | null, event) on click; a link is recognisable
        // by its `source`. Only links passing isLinkSelectable are offered.
        this.onSelect = (options && options.onSelect) || function () {};
        this.isLinkSelectable = (options && options.isLinkSelectable) || function () { return true; };
        this.onOpen = (options && options.onOpen) || function () {};
        this.onHover = (options && options.onHover) || function () {};

        this._bindEvents();
        this._resize();
        this._loop();
    }

    // ---- data -------------------------------------------------------------

    UniverseGraph.prototype.setData = function (payload) {
        var self = this;
        var previous = this.byId;
        var incoming = payload.nodes || [];
        this.nodes = incoming.map(function (n, i) {
            var old = previous[n.id];
            var angle = (i / Math.max(1, incoming.length)) * Math.PI * 2;
            var radius = 120 + Math.sqrt(i) * 26;
            return Object.assign({}, n, {
                // Keep positions across live reloads so the layout does not
                // jump every time the DSL file is saved.
                x: old ? old.x : Math.cos(angle) * radius,
                y: old ? old.y : Math.sin(angle) * radius,
                vx: 0, vy: 0, degree: 0, pinned: false
            });
        });
        this.byId = {};
        this.nodes.forEach(function (n) { self.byId[n.id] = n; });

        this.links = (payload.links || []).filter(function (l) {
            return self.byId[l.sourceId] && self.byId[l.targetId];
        }).map(function (l) {
            return Object.assign({}, l, {
                source: self.byId[l.sourceId],
                target: self.byId[l.targetId]
            });
        });

        this.linkById = {};
        this.links.forEach(function (l) { self.linkById[l.id] = l; });

        this.adjacency = {};
        this.links.forEach(function (l) {
            l.source.degree++;
            l.target.degree++;
            (self.adjacency[l.sourceId] = self.adjacency[l.sourceId] || []).push(l.targetId);
            (self.adjacency[l.targetId] = self.adjacency[l.targetId] || []).push(l.sourceId);
        });

        this.setSelection(Object.keys(this.selectedNodes), Object.keys(this.selectedLinks));
        this.applyFilters();
        this.reheat(1);
    };

    /** Mark nodes and links as selected (replacing any previous selection);
     *  ids the data doesn't have are ignored. */
    UniverseGraph.prototype.setSelection = function (nodeIds, linkIds) {
        var self = this;
        this.selectedNodes = {};
        this.selectedLinks = {};
        (nodeIds || []).forEach(function (id) { if (self.byId[id]) self.selectedNodes[id] = true; });
        (linkIds || []).forEach(function (id) { if (self.linkById[id]) self.selectedLinks[id] = true; });
    };

    UniverseGraph.prototype.setSetting = function (key, value) {
        this.settings[key] = value;
        this.reheat(0.4);
    };

    UniverseGraph.prototype.setFilter = function (key, value) {
        this.filters[key] = value;
        this.applyFilters();
        this.reheat(0.5);
    };

    /** Ids within `depth` hops of `startId`, inclusive. */
    UniverseGraph.prototype.neighborhood = function (startId, depth) {
        var seen = {}, frontier = [startId], d;
        seen[startId] = true;
        for (d = 0; d < depth; d++) {
            var next = [];
            frontier.forEach(function (id) {
                (this.adjacency[id] || []).forEach(function (nb) {
                    if (!seen[nb]) { seen[nb] = true; next.push(nb); }
                });
            }, this);
            frontier = next;
        }
        return seen;
    };

    UniverseGraph.prototype.applyFilters = function () {
        var f = this.filters;
        var needle = f.search.trim().toLowerCase();
        var inFocus = f.focusId && this.byId[f.focusId]
            ? this.neighborhood(f.focusId, f.focusDepth)
            : null;

        this.visibleNodes = this.nodes.filter(function (n) {
            if (f.kinds && !f.kinds[n.kind]) return false;
            if (inFocus && !inFocus[n.id]) return false;
            return true;
        });
        var visible = {};
        this.visibleNodes.forEach(function (n) { visible[n.id] = true; });

        this.visibleLinks = this.links.filter(function (l) {
            if (f.linkClasses && !f.linkClasses[l.class]) return false;
            return visible[l.sourceId] && visible[l.targetId];
        });

        // Visible-link counts drive spring normalisation in _tick: a hub with
        // dozens of links must not receive dozens of full-strength spring
        // kicks per tick, or the integrator diverges and coordinates explode.
        this.visibleNodes.forEach(function (n) { n.linkCount = 0; });
        this.visibleLinks.forEach(function (l) {
            l.source.linkCount++;
            l.target.linkCount++;
        });

        // Search dims rather than hides, the way Obsidian highlights matches.
        this.visibleNodes.forEach(function (n) {
            n.matched = !needle || (n.name || '').toLowerCase().indexOf(needle) >= 0 ||
                (n.tags || []).join(',').toLowerCase().indexOf(needle) >= 0 ||
                (n.kind || '').toLowerCase().indexOf(needle) >= 0;
        });
        this.hasSearch = !!needle;
    };

    UniverseGraph.prototype.reheat = function (alpha) {
        this.alpha = Math.max(this.alpha, alpha === undefined ? 0.6 : alpha);
    };

    // ---- simulation -------------------------------------------------------

    UniverseGraph.prototype._tick = function () {
        if (this.alpha < 0.005) return;
        var s = this.settings, nodes = this.visibleNodes, i, n;

        // Local repulsion through a spatial hash: only pairs sharing or
        // touching a cell interact, which is what keeps big models smooth.
        var cell = Math.max(40, s.linkDistance * 1.6);
        var grid = {};
        for (i = 0; i < nodes.length; i++) {
            n = nodes[i];
            var key = Math.round(n.x / cell) + ',' + Math.round(n.y / cell);
            (grid[key] = grid[key] || []).push(n);
        }
        for (i = 0; i < nodes.length; i++) {
            n = nodes[i];
            var cx = Math.round(n.x / cell), cy = Math.round(n.y / cell);
            // Iterate cell offsets, not raw cell indices: above 2^53,
            // `gx++` on a cell index is a no-op and the loop never exits.
            for (var ox = -1; ox <= 1; ox++) {
                for (var oy = -1; oy <= 1; oy++) {
                    var bucket = grid[(cx + ox) + ',' + (cy + oy)];
                    if (!bucket) continue;
                    for (var j = 0; j < bucket.length; j++) {
                        var m = bucket[j];
                        if (m === n) continue;
                        var dx = n.x - m.x, dy = n.y - m.y;
                        var d2 = dx * dx + dy * dy;
                        if (d2 < 0.01) { dx = (Math.random() - 0.5); dy = (Math.random() - 0.5); d2 = 0.01; }
                        var f = s.repelStrength / d2;
                        var d = Math.sqrt(d2);
                        n.vx += (dx / d) * f * this.alpha * 0.02;
                        n.vy += (dy / d) * f * this.alpha * 0.02;
                    }
                }
            }
        }

        // Link springs. Containment is drawn tighter than a plain
        // relationship so systems visibly cluster with their internals.
        // Strength is normalised by the endpoints' link counts (as in
        // d3-force): the summed spring stiffness on a node must stay below
        // the integrator's stability limit or hub positions diverge to
        // infinity within a second.
        for (i = 0; i < this.visibleLinks.length; i++) {
            var l = this.visibleLinks[i];
            var target = l.class === 'containment' ? s.linkDistance * 0.6 : s.linkDistance;
            var ldx = l.target.x - l.source.x, ldy = l.target.y - l.source.y;
            var dist = Math.sqrt(ldx * ldx + ldy * ldy) || 0.01;
            var sc = l.source.linkCount || 1, tc = l.target.linkCount || 1;
            var strength = s.linkStrength / Math.min(sc, tc);
            var force = ((dist - target) / dist) * strength * this.alpha * 0.5;
            var bias = sc / (sc + tc);   // the busier endpoint moves less
            l.source.vx += ldx * force * (1 - bias);
            l.source.vy += ldy * force * (1 - bias);
            l.target.vx -= ldx * force * bias;
            l.target.vy -= ldy * force * bias;
        }

        // Centering + integration, with a speed cap as a final backstop so
        // no combination of slider settings can send coordinates towards
        // the float range where the physics (and the spatial hash) break.
        var maxSpeed = s.linkDistance;
        for (i = 0; i < nodes.length; i++) {
            n = nodes[i];
            if (n.pinned) { n.vx = n.vy = 0; continue; }
            n.vx -= n.x * s.centerStrength * this.alpha * 0.1;
            n.vy -= n.y * s.centerStrength * this.alpha * 0.1;
            n.vx *= 0.82;
            n.vy *= 0.82;
            var speed = Math.sqrt(n.vx * n.vx + n.vy * n.vy);
            if (speed > maxSpeed) {
                n.vx *= maxSpeed / speed;
                n.vy *= maxSpeed / speed;
            }
            n.x += n.vx;
            n.y += n.vy;
        }

        this.alpha *= 0.985;
    };

    // ---- geometry ---------------------------------------------------------

    UniverseGraph.prototype.radius = function (n) {
        return (3 + Math.sqrt(n.degree || 0) * 1.7) * this.settings.nodeSize;
    };

    UniverseGraph.prototype.toScreen = function (x, y) {
        return { x: x * this.transform.k + this.transform.x, y: y * this.transform.k + this.transform.y };
    };

    UniverseGraph.prototype.toWorld = function (x, y) {
        return { x: (x - this.transform.x) / this.transform.k, y: (y - this.transform.y) / this.transform.k };
    };

    UniverseGraph.prototype.nodeAt = function (screenX, screenY) {
        var p = this.toWorld(screenX, screenY);
        for (var i = this.visibleNodes.length - 1; i >= 0; i--) {
            var n = this.visibleNodes[i];
            var r = Math.max(this.radius(n), 8 / this.transform.k);
            if ((n.x - p.x) * (n.x - p.x) + (n.y - p.y) * (n.y - p.y) <= r * r) return n;
        }
        return null;
    };

    /** The selectable visible link nearest a screen point, within a few pixels. */
    UniverseGraph.prototype.linkAt = function (screenX, screenY) {
        var p = this.toWorld(screenX, screenY);
        var best = null, bestDistance = 5 / this.transform.k;
        for (var i = 0; i < this.visibleLinks.length; i++) {
            var l = this.visibleLinks[i];
            if (!this.isLinkSelectable(l)) continue;
            var ax = l.source.x, ay = l.source.y;
            var dx = l.target.x - ax, dy = l.target.y - ay;
            var length2 = dx * dx + dy * dy;
            var t = length2 ? Math.max(0, Math.min(1, ((p.x - ax) * dx + (p.y - ay) * dy) / length2)) : 0;
            var ex = p.x - (ax + t * dx), ey = p.y - (ay + t * dy);
            var distance = Math.sqrt(ex * ex + ey * ey);
            if (distance <= bestDistance) { best = l; bestDistance = distance; }
        }
        return best;
    };

    UniverseGraph.prototype.zoomToFit = function (padding) {
        var nodes = this.visibleNodes;
        if (!nodes.length) { this.transform = { k: 1, x: this.width / 2, y: this.height / 2 }; return; }
        var pad = padding === undefined ? 60 : padding;
        var minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
        nodes.forEach(function (n) {
            minX = Math.min(minX, n.x); maxX = Math.max(maxX, n.x);
            minY = Math.min(minY, n.y); maxY = Math.max(maxY, n.y);
        });
        var w = Math.max(maxX - minX, 1), h = Math.max(maxY - minY, 1);
        // Floor the zoom: 1/k scales fonts and line widths in _draw, so a
        // near-zero k would ask the canvas for astronomically large glyphs.
        var k = Math.max(0.02, Math.min((this.width - pad * 2) / w, (this.height - pad * 2) / h, 3));
        this.transform.k = k;
        this.transform.x = this.width / 2 - ((minX + maxX) / 2) * k;
        this.transform.y = this.height / 2 - ((minY + maxY) / 2) * k;
    };

    UniverseGraph.prototype.zoomBy = function (factor, cx, cy) {
        var px = cx === undefined ? this.width / 2 : cx;
        var py = cy === undefined ? this.height / 2 : cy;
        var before = this.toWorld(px, py);
        this.transform.k = Math.min(6, Math.max(0.08, this.transform.k * factor));
        var after = this.toWorld(px, py);
        this.transform.x += (after.x - before.x) * this.transform.k;
        this.transform.y += (after.y - before.y) * this.transform.k;
    };

    // ---- rendering --------------------------------------------------------

    /** Node ids to keep lit: the hovered node's neighbourhood or the hovered
     *  link's ends; otherwise every selected node's neighbourhood and every
     *  selected link's ends. Null when nothing is hovered or selected. */
    UniverseGraph.prototype._highlightSet = function () {
        var self = this, set = {}, any = false;
        function around(id) {
            set[id] = true;
            (self.adjacency[id] || []).forEach(function (nb) { set[nb] = true; });
            any = true;
        }
        function ends(l) {
            set[l.sourceId] = true;
            set[l.targetId] = true;
            any = true;
        }
        if (this.hovered) {
            around(this.hovered.id);
        } else if (this.hoveredLink) {
            ends(this.hoveredLink);
        } else {
            Object.keys(this.selectedNodes).forEach(around);
            Object.keys(this.selectedLinks).forEach(function (id) { ends(self.linkById[id]); });
        }
        return any ? set : null;
    };

    /** Re-read the CSS custom properties that colour the canvas. Called once
     *  at start-up and again whenever the page switches theme — reading them
     *  per frame would force a style recalculation 60 times a second. */
    UniverseGraph.prototype.refreshTheme = function () {
        var css = getComputedStyle(this.canvas);
        this.theme = {
            fg: css.getPropertyValue('--graph-fg').trim() || '#222',
            link: css.getPropertyValue('--graph-link').trim() || 'rgba(130,130,150,.45)',
            bg: css.getPropertyValue('--graph-bg').trim() || '#fff'
        };
    };

    UniverseGraph.prototype._draw = function () {
        var ctx = this.ctx, t = this.transform, self = this;
        if (!this.theme) this.refreshTheme();
        var fg = this.theme.fg, linkColor = this.theme.link;

        ctx.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
        ctx.clearRect(0, 0, this.width, this.height);
        ctx.save();
        ctx.translate(t.x, t.y);
        ctx.scale(t.k, t.k);

        var highlight = this._highlightSet();

        // Links.
        this.visibleLinks.forEach(function (l) {
            var style = LINK_STYLES[l.class] || LINK_STYLES.relationship;
            var chosen = self.selectedLinks[l.id] || l === self.hoveredLink;
            var lit = chosen || !highlight || (highlight[l.sourceId] && highlight[l.targetId]);
            ctx.globalAlpha = chosen ? 1 : lit ? (highlight ? 0.95 : 0.6) : 0.08;
            ctx.strokeStyle = chosen ? fg : lit && highlight ? KIND_COLORS[l.source.kind] || linkColor : linkColor;
            ctx.lineWidth = style.width / t.k * (chosen ? 3 : lit && highlight ? 1.8 : 1);
            ctx.setLineDash(style.dash.map(function (d) { return d / t.k; }));
            ctx.beginPath();
            ctx.moveTo(l.source.x, l.source.y);
            ctx.lineTo(l.target.x, l.target.y);
            ctx.stroke();

            if (style.arrow && self.settings.showArrows && t.k > 0.5 && lit) {
                var dx = l.target.x - l.source.x, dy = l.target.y - l.source.y;
                var d = Math.sqrt(dx * dx + dy * dy) || 1;
                var r = self.radius(l.target) + 2;
                var tipX = l.target.x - (dx / d) * r, tipY = l.target.y - (dy / d) * r;
                var a = Math.atan2(dy, dx), size = 6 / t.k;
                ctx.setLineDash([]);
                ctx.beginPath();
                ctx.moveTo(tipX, tipY);
                ctx.lineTo(tipX - size * Math.cos(a - 0.4), tipY - size * Math.sin(a - 0.4));
                ctx.lineTo(tipX - size * Math.cos(a + 0.4), tipY - size * Math.sin(a + 0.4));
                ctx.closePath();
                ctx.fillStyle = ctx.strokeStyle;
                ctx.fill();
            }
        });
        ctx.setLineDash([]);

        // Nodes.
        this.visibleNodes.forEach(function (n) {
            var r = self.radius(n);
            var lit = (!highlight || highlight[n.id]) && (!self.hasSearch || n.matched);
            ctx.globalAlpha = lit ? 1 : 0.12;
            ctx.fillStyle = KIND_COLORS[n.kind] || '#8a8a8a';
            ctx.beginPath();
            ctx.arc(n.x, n.y, r, 0, Math.PI * 2);
            ctx.fill();
            if (self.selectedNodes[n.id]) {
                ctx.strokeStyle = fg;
                ctx.lineWidth = 2 / t.k;
                ctx.stroke();
            }
        });

        // Labels, revealed as you zoom in. Drawn most-connected first, and a
        // label that would collide with one already placed is dropped, so the
        // dense middle of a big model stays readable.
        if (this.settings.showLabels) {
            ctx.textAlign = 'center';
            ctx.textBaseline = 'top';
            ctx.font = (11 / t.k) + 'px system-ui, sans-serif';
            var placed = [];
            this._byDegree().forEach(function (n) {
                var r = self.radius(n);
                var always = (highlight && highlight[n.id]) || (self.hasSearch && n.matched);
                if (!always && t.k < self.settings.labelZoomThreshold) return;
                if (!always && t.k < 1 && n.degree < 2) return;
                var lit = (!highlight || highlight[n.id]) && (!self.hasSearch || n.matched);
                if (!lit && !always) return;

                var label = n.name.length > 42 ? n.name.slice(0, 40) + '…' : n.name;
                var w = ctx.measureText(label).width;
                var box = {
                    x0: n.x - w / 2, x1: n.x + w / 2,
                    y0: n.y + r + 3 / t.k, y1: n.y + r + (3 + 12) / t.k
                };
                var clash = placed.some(function (p) {
                    return !(box.x1 < p.x0 || box.x0 > p.x1 || box.y1 < p.y0 || box.y0 > p.y1);
                });
                if (clash && !always) return;
                placed.push(box);

                ctx.globalAlpha = lit ? 0.95 : 0.25;
                // A halo in the page background colour keeps text legible
                // where it crosses a link.
                ctx.lineWidth = 3 / t.k;
                ctx.strokeStyle = self.theme.bg;
                ctx.strokeText(label, n.x, box.y0);
                ctx.fillStyle = fg;
                ctx.fillText(label, n.x, box.y0);
            });
        }

        ctx.globalAlpha = 1;
        ctx.restore();
    };

    /** Visible nodes, most connected first (cached; labels are laid out in
     *  this order so hubs keep their label when space runs out). */
    UniverseGraph.prototype._byDegree = function () {
        if (this._sortedFor !== this.visibleNodes) {
            this._sorted = this.visibleNodes.slice().sort(function (a, b) {
                return b.degree - a.degree;
            });
            this._sortedFor = this.visibleNodes;
        }
        return this._sorted;
    };

    UniverseGraph.prototype._loop = function () {
        var self = this;
        function frame() {
            self._tick();
            self._draw();
            requestAnimationFrame(frame);
        }
        requestAnimationFrame(frame);
    };

    // ---- interaction ------------------------------------------------------

    UniverseGraph.prototype._resize = function () {
        var rect = this.canvas.getBoundingClientRect();
        this.dpr = window.devicePixelRatio || 1;
        this.width = rect.width;
        this.height = rect.height;
        this.canvas.width = rect.width * this.dpr;
        this.canvas.height = rect.height * this.dpr;
        if (!this._centered) {
            this.transform.x = this.width / 2;
            this.transform.y = this.height / 2;
            this._centered = true;
        }
    };

    UniverseGraph.prototype._bindEvents = function () {
        var self = this, canvas = this.canvas;
        var panning = false, last = null, moved = false;

        window.addEventListener('resize', function () { self._resize(); });

        canvas.addEventListener('mousedown', function (e) {
            var rect = canvas.getBoundingClientRect();
            var x = e.clientX - rect.left, y = e.clientY - rect.top;
            var hit = self.nodeAt(x, y);
            moved = false;
            if (hit) {
                self.dragging = hit;
                hit.pinned = true;
                self.reheat(0.5);
            } else {
                panning = true;
            }
            last = { x: x, y: y };
        });

        canvas.addEventListener('mousemove', function (e) {
            var rect = canvas.getBoundingClientRect();
            var x = e.clientX - rect.left, y = e.clientY - rect.top;
            if (self.dragging) {
                moved = true;
                var p = self.toWorld(x, y);
                self.dragging.x = p.x;
                self.dragging.y = p.y;
                self.reheat(0.4);
            } else if (panning) {
                moved = true;
                self.transform.x += x - last.x;
                self.transform.y += y - last.y;
            } else {
                var hit = self.nodeAt(x, y);
                var link = hit ? null : self.linkAt(x, y);
                if (hit !== self.hovered || link !== self.hoveredLink) {
                    self.hovered = hit;
                    self.hoveredLink = link;
                    canvas.style.cursor = hit || link ? 'pointer' : 'grab';
                    self.onHover(hit || link, { x: e.clientX, y: e.clientY });
                }
            }
            last = { x: x, y: y };
        });

        window.addEventListener('mouseup', function () {
            if (self.dragging) {
                // A node dropped after a real drag stays where it was put;
                // a click without movement releases it back to the layout.
                if (!moved) self.dragging.pinned = false;
                self.dragging = null;
            }
            panning = false;
        });

        canvas.addEventListener('click', function (e) {
            if (moved) return;
            var rect = canvas.getBoundingClientRect();
            var x = e.clientX - rect.left, y = e.clientY - rect.top;
            var node = self.nodeAt(x, y);
            self.onSelect(node || self.linkAt(x, y), e);
        });

        canvas.addEventListener('dblclick', function (e) {
            var rect = canvas.getBoundingClientRect();
            var hit = self.nodeAt(e.clientX - rect.left, e.clientY - rect.top);
            if (hit) self.onOpen(hit);
        });

        canvas.addEventListener('wheel', function (e) {
            e.preventDefault();
            var unit = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? 100 : 1;
            var dx = e.deltaX * unit, dy = e.deltaY * unit;
            if (e.ctrlKey || e.metaKey) {
                // Ctrl/⌘ + wheel zooms (this is also what a trackpad pinch
                // reports). Proportional to deltaY so a pinch's many small-delta
                // events zoom smoothly; the cap keeps a mouse-wheel notch to ~12%.
                dy = Math.max(-12, Math.min(12, dy));
                var rect = canvas.getBoundingClientRect();
                self.zoomBy(Math.exp(-dy * 0.01), e.clientX - rect.left, e.clientY - rect.top);
            } else {
                // A plain wheel / two-finger scroll pans, in both axes.
                self.transform.x -= dx;
                self.transform.y -= dy;
            }
        }, { passive: false });

        canvas.addEventListener('mouseleave', function () {
            if (self.hovered || self.hoveredLink) {
                self.hovered = null;
                self.hoveredLink = null;
                self.onHover(null);
            }
        });
    };

    UniverseGraph.KIND_COLORS = KIND_COLORS;
    global.UniverseGraph = UniverseGraph;
})(window);
