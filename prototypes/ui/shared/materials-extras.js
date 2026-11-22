// Materials stage helpers layered on top of BenchView (shared/model.js) without editing it:
// grain streaks drawn on visible faces and outlines for a multi-part selection.
(function () {
  "use strict";
  const NS = "http://www.w3.org/2000/svg";
  const el = (name, attrs, parent) => {
    const node = document.createElementNS(NS, name);
    for (const k in attrs) node.setAttribute(k, attrs[k]);
    if (parent) parent.appendChild(node);
    return node;
  };
  const NORMALS = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
  const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];

  // Small deterministic noise so streaks stay put while orbiting.
  function rand(seed) {
    let s = 0;
    for (let i = 0; i < seed.length; i++) s = (s * 31 + seed.charCodeAt(i)) >>> 0;
    return () => {
      s = (s * 1664525 + 1013904223) >>> 0;
      return s / 4294967296;
    };
  }

  function darken(hex, f) {
    const n = parseInt(hex.slice(1), 16);
    const c = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((v) => Math.round(v * f));
    return `rgb(${c[0]},${c[1]},${c[2]})`;
  }

  /** Axis index (0 x, 1 y, 2 z) the grain follows for a part and a direction choice. */
  function grainAxis(part, direction) {
    const d = [0, 1, 2].map((k) => part.max[k] - part.min[k]);
    const order = [0, 1, 2].sort((a, b) => d[b] - d[a]);
    return direction === "across" ? order[1] : order[0];
  }

  /**
   * Draws grain on the faces BenchView just rendered. Faces inside each part group
   * appear in boxFaces order (+x, -x, +y, -y, +z, -z), filtered by visibility.
   */
  function drawGrain(view, opts) {
    const svg = view.svg;
    const defs = svg.querySelector("defs");
    const d = view._basis.d;
    let clip = 0;
    for (const g of svg.querySelectorAll("g[data-part]")) {
      const part = view.parts.find((p) => p.id === g.getAttribute("data-part"));
      if (!part || part.material === "unassigned") continue;
      const axis = grainAxis(part, opts.direction(part));
      const color = darken(opts.color(part), 0.72);
      const polys = [...g.querySelectorAll("polygon")];
      const normals = NORMALS.filter((n) => dot(n, d) > 1e-6);
      const center = [0, 1, 2].map((k) => (part.min[k] + part.max[k]) / 2);
      const c0 = view.project(center);
      const unit = [0, 0, 0];
      unit[axis] = 1;
      const c1 = view.project(center.map((v, k) => v + unit[k] * 10));
      const raw = [c1[0] - c0[0], c1[1] - c0[1]];
      const len = Math.hypot(raw[0], raw[1]);
      const u = len > 0 ? [raw[0] / len, raw[1] / len] : [1, 0];
      polys.forEach((poly, i) => {
        const n = normals[i];
        if (!n) return;
        const pts = poly.getAttribute("points").trim().split(/\s+/).map((p) => p.split(",").map(Number));
        const id = `grain-${opts.prefix}-${clip++}`;
        const cp = el("clipPath", { id }, defs);
        el("polygon", { points: poly.getAttribute("points") }, cp);
        // Inside the part's own group, so parts drawn later still cover it.
        const group = el("g", { "clip-path": `url(#${id})`, "pointer-events": "none" }, g);
        const cx = pts.reduce((a, p) => a + p[0], 0) / pts.length;
        const cy = pts.reduce((a, p) => a + p[1], 0) / pts.length;
        const r = rand(part.id + i);
        if (Math.abs(n[axis]) > 0.5 || len < 0.5) {
          // End grain: growth rings around an off-centre pith.
          const px = cx + (r() - 0.5) * 30;
          const py = cy + (r() - 0.5) * 30;
          for (let k = 1; k <= 7; k++) {
            el("ellipse", { cx: px, cy: py, rx: k * 4.2, ry: k * 3.1, fill: "none", stroke: color, "stroke-width": 0.6, opacity: 0.45 }, group);
          }
          return;
        }
        const v = [-u[1], u[0]];
        const along = pts.map((p) => (p[0] - cx) * u[0] + (p[1] - cy) * u[1]);
        const across = pts.map((p) => (p[0] - cx) * v[0] + (p[1] - cy) * v[1]);
        const a0 = Math.min(...along) - 4, a1 = Math.max(...along) + 4;
        const spacing = 4.2;
        for (let t = Math.min(...across); t <= Math.max(...across); t += spacing * (0.6 + r() * 0.9)) {
          const wobble = (r() - 0.5) * 3;
          const sx = cx + v[0] * t + u[0] * a0, sy = cy + v[1] * t + u[1] * a0;
          const ex = cx + v[0] * t + u[0] * a1, ey = cy + v[1] * t + u[1] * a1;
          const mx = (sx + ex) / 2 + v[0] * wobble, my = (sy + ey) / 2 + v[1] * wobble;
          el("path", { d: `M${sx},${sy} Q${mx},${my} ${ex},${ey}`, fill: "none", stroke: color, "stroke-width": 0.6 + r() * 0.5, opacity: 0.3 + r() * 0.25 }, group);
        }
      });
      // Redraw edges above the streaks.
      for (const poly of polys) {
        el("polygon", { points: poly.getAttribute("points"), fill: "none", stroke: poly.getAttribute("stroke"),
          "stroke-width": poly.getAttribute("stroke-width"), "stroke-linejoin": "round", "pointer-events": "none" }, g);
      }
    }
  }

  /** Outlines every selected part, since BenchView highlights only one. */
  function outlineSelection(view, ids, color) {
    for (const g of view.svg.querySelectorAll("g[data-part]")) {
      if (!ids.has(g.getAttribute("data-part"))) continue;
      for (const poly of g.querySelectorAll("polygon")) {
        poly.setAttribute("stroke", color);
        poly.setAttribute("stroke-width", 2);
      }
    }
  }

  window.MaterialsExtras = { drawGrain, outlineSelection, grainAxis };
})();
