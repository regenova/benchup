// BenchUp UI prototypes: shared sample model and a tiny orthographic SVG renderer.
// Plain script (no modules) so the prototypes open straight from the file system.
(function () {
  "use strict";

  const P = (id, name, kind, material, a, b) => ({ id, name, kind, material, min: a, max: b });

  // A small hall bench, in centimetres. x = length, y = depth, z = height.
  const PARTS = [
    P("leg-bl", "Leg", "Legs", "oak", [2, 2, 0], [6.5, 6.5, 42.5]),
    P("leg-br", "Leg", "Legs", "oak", [83.5, 2, 0], [88, 6.5, 42.5]),
    P("leg-fl", "Leg", "Legs", "oak", [2, 28.5, 0], [6.5, 33, 42.5]),
    P("leg-fr", "Leg", "Legs", "oak", [83.5, 28.5, 0], [88, 33, 42.5]),
    P("apron-b", "Long apron", "Aprons", "oak", [6.5, 3, 34], [83.5, 5, 42.5]),
    P("apron-f", "Long apron", "Aprons", "oak", [6.5, 30, 34], [83.5, 32, 42.5]),
    P("apron-l", "Short apron", "Aprons", "oak", [3, 6.5, 34], [5, 28.5, 42.5]),
    P("apron-r", "Short apron", "Aprons", "oak", [85, 6.5, 34], [87, 28.5, 42.5]),
    P("str-b", "Stretcher", "Stretchers", "oak", [6.5, 3, 8], [83.5, 5, 12]),
    P("str-f", "Stretcher", "Stretchers", "oak", [6.5, 30, 8], [83.5, 32, 12]),
    P("shelf", "Lower shelf", "Panels", "plywood", [6.5, 5, 12], [83.5, 30, 13.8]),
    P("top", "Top", "Panels", "walnut", [0, 0, 42.5], [90, 35, 45]),
  ];

  const MATERIALS = {
    oak: { name: "White oak", stock: "Hardwood 27 mm" },
    walnut: { name: "Black walnut", stock: "Hardwood 25 mm" },
    plywood: { name: "Birch plywood", stock: "Sheet 18 mm" },
  };

  function size(part) {
    const d = [0, 1, 2].map((k) => part.max[k] - part.min[k]);
    const sorted = d.slice().sort((a, b) => b - a);
    return { x: d[0], y: d[1], z: d[2], length: sorted[0], width: sorted[1], thickness: sorted[2] };
  }

  const mm = (cm) => `${Math.round(cm * 10)}`;
  const fmtDims = (part) => {
    const s = size(part);
    return `${mm(s.length)} × ${mm(s.width)} × ${mm(s.thickness)}`;
  };

  const NS = "http://www.w3.org/2000/svg";
  const el = (name, attrs, parent) => {
    const node = document.createElementNS(NS, name);
    for (const k in attrs) node.setAttribute(k, attrs[k]);
    if (parent) parent.appendChild(node);
    return node;
  };
  const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
  const norm = (v) => {
    const l = Math.hypot(v[0], v[1], v[2]) || 1;
    return [v[0] / l, v[1] / l, v[2] / l];
  };
  const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];

  function boxFaces(min, max) {
    const [x0, y0, z0] = min;
    const [x1, y1, z1] = max;
    return [
      { n: [1, 0, 0], p: [[x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1]] },
      { n: [-1, 0, 0], p: [[x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]] },
      { n: [0, 1, 0], p: [[x0, y1, z0], [x0, y1, z1], [x1, y1, z1], [x1, y1, z0]] },
      { n: [0, -1, 0], p: [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]] },
      { n: [0, 0, 1], p: [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]] },
      { n: [0, 0, -1], p: [[x0, y0, z0], [x0, y1, z0], [x1, y1, z0], [x1, y0, z0]] },
    ];
  }

  function shade(hex, f) {
    const n = parseInt(hex.slice(1), 16);
    const c = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((v) => Math.max(0, Math.min(255, Math.round(v * f))));
    return `rgb(${c[0]},${c[1]},${c[2]})`;
  }

  const DEFAULT_THEME = {
    colors: { oak: "#c9a26b", walnut: "#7a5236", plywood: "#e3cfa4" },
    stroke: "rgba(0,0,0,0.35)",
    strokeWidth: 0.8,
    ambient: 0.62,
    diffuse: 0.42,
    light: [0.35, 0.55, 0.85],
    selectedStroke: "#e0533d",
    selectedWidth: 2,
    selectedTint: null,
    hoverStroke: "rgba(224,83,61,0.55)",
    dimColor: "#e0533d",
    dimText: "#e0533d",
    dimBg: "rgba(255,255,255,0.92)",
    dimFont: "11px ui-monospace, SFMono-Regular, Menlo, monospace",
    grid: null, // { minor, major }
    shadow: "rgba(0,0,0,0.18)",
    flat: null, // when set, every face uses this fill (line drawings)
    selectedFill: null, // e.g. "url(#hatch)"
    dimmed: null, // ids not in this set fade out
  };

  class BenchView {
    constructor(svg, options = {}) {
      this.svg = svg;
      this.parts = options.parts || PARTS;
      this.theme = Object.assign({}, DEFAULT_THEME, options.theme || {});
      this.view = Object.assign({ azimuth: 58, elevation: 26, zoom: 1, pan: [0, 0] }, options.view || {});
      this.fixedDir = options.direction || null; // exact view vector for orthographic sheets
      this.interactive = options.interactive !== false;
      this.selectedId = options.selectedId || null;
      this.hoverId = null;
      this.dims = options.dims !== false;
      this.fill = options.fill || 0.78;
      this.onSelect = options.onSelect || (() => {});
      this.onRender = options.onRender || (() => {});
      this.defs = options.defs || "";
      this.center = [45, 17.5, 22.5];
      this.radius = 55;
      this._bind();
      new ResizeObserver(() => this.render()).observe(svg);
      this.render();
    }

    basis() {
      let d;
      if (this.fixedDir) d = norm(this.fixedDir);
      else {
        const az = (this.view.azimuth * Math.PI) / 180;
        const el = (this.view.elevation * Math.PI) / 180;
        d = [Math.cos(el) * Math.cos(az), Math.cos(el) * Math.sin(az), Math.sin(el)];
      }
      let right = [d[1], -d[0], 0];
      if (Math.hypot(right[0], right[1]) < 1e-6) right = [1, 0, 0];
      right = norm(right);
      const up = norm(cross(right, d));
      return { d, right, up };
    }

    project(p) {
      const { right, up } = this._basis;
      const q = [p[0] - this.center[0], p[1] - this.center[1], p[2] - this.center[2]];
      return [this._cx + dot(q, right) * this._s, this._cy - dot(q, up) * this._s];
    }

    partCenterScreen(id) {
      const part = this.parts.find((p) => p.id === id);
      if (!part) return null;
      return this.project([0, 1, 2].map((k) => (part.min[k] + part.max[k]) / 2));
    }

    partTopScreen(id) {
      const part = this.parts.find((p) => p.id === id);
      if (!part) return null;
      const pts = [];
      for (const x of [part.min[0], part.max[0]])
        for (const y of [part.min[1], part.max[1]])
          for (const z of [part.min[2], part.max[2]]) pts.push(this.project([x, y, z]));
      return {
        x: pts.reduce((a, p) => a + p[0], 0) / pts.length,
        top: Math.min(...pts.map((p) => p[1])),
        bottom: Math.max(...pts.map((p) => p[1])),
        right: Math.max(...pts.map((p) => p[0])),
      };
    }

    setSelected(id) {
      this.selectedId = id;
      this.render();
    }

    setView(v) {
      Object.assign(this.view, v);
      this.render();
    }

    animateTo(target, ms = 450) {
      const from = Object.assign({}, this.view);
      let dAz = ((target.azimuth ?? from.azimuth) - from.azimuth) % 360;
      if (dAz > 180) dAz -= 360;
      if (dAz < -180) dAz += 360;
      const t0 = performance.now();
      const step = (t) => {
        const k = Math.min(1, (t - t0) / ms);
        const e = 1 - Math.pow(1 - k, 3);
        this.view.azimuth = from.azimuth + dAz * e;
        this.view.elevation = from.elevation + ((target.elevation ?? from.elevation) - from.elevation) * e;
        this.view.zoom = from.zoom + ((target.zoom ?? from.zoom) - from.zoom) * e;
        this.render();
        if (k < 1) requestAnimationFrame(step);
      };
      requestAnimationFrame(step);
    }

    _bind() {
      const svg = this.svg;
      let start = null;
      let moved = false;
      svg.addEventListener("pointerdown", (e) => {
        start = { x: e.clientX, y: e.clientY, az: this.view.azimuth, el: this.view.elevation };
        moved = false;
        svg.setPointerCapture(e.pointerId);
      });
      svg.addEventListener("pointermove", (e) => {
        if (start && this.interactive) {
          const dx = e.clientX - start.x;
          const dy = e.clientY - start.y;
          if (!moved && Math.hypot(dx, dy) > 4) moved = true;
          if (moved) {
            this.view.azimuth = start.az - dx * 0.4;
            this.view.elevation = Math.max(4, Math.min(86, start.el + dy * 0.3));
            svg.style.cursor = "grabbing";
            this.render();
            return;
          }
        }
        if (!start) {
          const hit = e.target.closest && e.target.closest("[data-part]");
          const id = hit ? hit.getAttribute("data-part") : null;
          if (id !== this.hoverId) {
            this.hoverId = id;
            svg.style.cursor = id ? "pointer" : this.interactive ? "grab" : "default";
            this.render();
          }
        }
      });
      svg.addEventListener("pointerup", (e) => {
        if (start && !moved) {
          const target = document.elementFromPoint(e.clientX, e.clientY);
          const hit = target && target.closest && target.closest("[data-part]");
          const id = hit ? hit.getAttribute("data-part") : null;
          this.selectedId = id;
          this.render();
          this.onSelect(id);
        }
        start = null;
        svg.style.cursor = this.interactive ? "grab" : "default";
      });
      svg.addEventListener("pointerleave", () => {
        if (!start && this.hoverId) {
          this.hoverId = null;
          this.render();
        }
      });
      if (this.interactive) {
        svg.style.cursor = "grab";
        svg.addEventListener(
          "wheel",
          (e) => {
            e.preventDefault();
            this.view.zoom = Math.max(0.4, Math.min(3, this.view.zoom * Math.exp(-e.deltaY * 0.0015)));
            this.render();
          },
          { passive: false }
        );
      }
    }

    render() {
      const svg = this.svg;
      const rect = svg.getBoundingClientRect();
      const W = Math.max(1, rect.width);
      const H = Math.max(1, rect.height);
      svg.setAttribute("viewBox", `0 0 ${W} ${H}`);
      this._basis = this.basis();
      this._s = (Math.min(W, H) / (2 * this.radius)) * this.fill * (this.view.zoom || 1);
      this._cx = W / 2 + (this.view.pan?.[0] || 0);
      this._cy = H / 2 + (this.view.pan?.[1] || 0);
      const t = this.theme;
      const { d } = this._basis;

      while (svg.firstChild) svg.removeChild(svg.firstChild);
      const defs = el("defs", {}, svg);
      defs.innerHTML =
        `<filter id="bv-blur" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="${8 * this._s / 4}"/></filter>` +
        `<radialGradient id="bv-fade"><stop offset="0" stop-color="#fff" stop-opacity="1"/><stop offset="1" stop-color="#fff" stop-opacity="0"/></radialGradient>` +
        this.defs;

      // Ground: fading grid and a soft contact shadow.
      if (t.grid && d[2] > 0.02) {
        const c = this.project([this.center[0], this.center[1], 0]);
        const mask = el("mask", { id: "bv-grid-mask" }, defs);
        el("ellipse", { cx: c[0], cy: c[1], rx: this._s * 150, ry: this._s * 150 * Math.max(0.15, d[2]) + 1, fill: "url(#bv-fade)" }, mask);
        const g = el("g", { mask: "url(#bv-grid-mask)" }, svg);
        const span = 150;
        for (let v = -span; v <= span; v += 10) {
          const major = v % 50 === 0;
          const attrs = { stroke: major ? t.grid.major : t.grid.minor, "stroke-width": major ? 1 : 0.6 };
          const a = this.project([this.center[0] + v, this.center[1] - span, 0]);
          const b = this.project([this.center[0] + v, this.center[1] + span, 0]);
          el("line", Object.assign({ x1: a[0], y1: a[1], x2: b[0], y2: b[1] }, attrs), g);
          const e1 = this.project([this.center[0] - span, this.center[1] + v, 0]);
          const e2 = this.project([this.center[0] + span, this.center[1] + v, 0]);
          el("line", Object.assign({ x1: e1[0], y1: e1[1], x2: e2[0], y2: e2[1] }, attrs), g);
        }
      }
      if (t.shadow && d[2] > 0.02) {
        const pts = [[-3, -3, 0], [93, -3, 0], [93, 38, 0], [-3, 38, 0]].map((p) => this.project(p).join(",")).join(" ");
        el("polygon", { points: pts, fill: t.shadow, filter: "url(#bv-blur)" }, svg);
      }

      // Painter's order using axis-separation for axis-aligned boxes.
      const parts = this.parts;
      const eps = 1e-6;
      const behind = (A, B) => {
        for (let k = 0; k < 3; k++) {
          if (d[k] > eps && A.max[k] <= B.min[k] + eps) return true;
          if (d[k] < -eps && A.min[k] >= B.max[k] - eps) return true;
        }
        return false;
      };
      const depth = (p) => dot([0, 1, 2].map((k) => (p.min[k] + p.max[k]) / 2), d);
      const indeg = parts.map(() => 0);
      const out = parts.map(() => []);
      for (let i = 0; i < parts.length; i++)
        for (let j = 0; j < parts.length; j++) {
          if (i === j) continue;
          if (behind(parts[i], parts[j]) && !behind(parts[j], parts[i])) {
            out[i].push(j);
            indeg[j]++;
          }
        }
      const order = [];
      const done = parts.map(() => false);
      while (order.length < parts.length) {
        let pick = -1;
        for (let i = 0; i < parts.length; i++)
          if (!done[i] && indeg[i] === 0 && (pick < 0 || depth(parts[i]) < depth(parts[pick]))) pick = i;
        if (pick < 0)
          for (let i = 0; i < parts.length; i++)
            if (!done[i] && (pick < 0 || depth(parts[i]) < depth(parts[pick]))) pick = i;
        done[pick] = true;
        order.push(pick);
        for (const j of out[pick]) indeg[j]--;
      }

      const L = norm(t.light);
      const scene = el("g", {}, svg);
      for (const i of order) {
        const part = parts[i];
        const selected = part.id === this.selectedId;
        const hovered = part.id === this.hoverId && !selected;
        const faded = t.dimmed && !t.dimmed.has(part.id);
        const g = el("g", { "data-part": part.id, opacity: faded ? 0.28 : 1 }, scene);
        for (const f of boxFaces(part.min, part.max)) {
          if (dot(f.n, d) <= 1e-6) continue;
          const pts = f.p.map((p) => this.project(p).join(",")).join(" ");
          const k = t.ambient + t.diffuse * Math.max(0, dot(f.n, L));
          let fill = t.flat ? t.flat : shade((selected && t.selectedTint) || t.colors[part.material] || "#bbbbbb", k);
          if (selected && t.selectedFill) fill = t.selectedFill;
          el(
            "polygon",
            {
              points: pts,
              fill,
              stroke: selected ? t.selectedStroke : hovered ? t.hoverStroke : t.stroke,
              "stroke-width": selected ? t.selectedWidth : hovered ? Math.max(t.strokeWidth, 1.4) : t.strokeWidth,
              "stroke-linejoin": "round",
            },
            g
          );
        }
      }

      if (this.dims && this.selectedId) this._drawDims(svg);
      this.onRender(this);
    }

    _drawDims(svg) {
      const part = this.parts.find((p) => p.id === this.selectedId);
      if (!part) return;
      const t = this.theme;
      const { d } = this._basis;
      const g = el("g", { "pointer-events": "none" }, svg);
      const off = 6;
      const fx = d[0] >= 0 ? part.max[0] : part.min[0];
      const fy = d[1] >= 0 ? part.max[1] : part.min[1];
      const sx = d[0] >= 0 ? 1 : -1;
      const sy = d[1] >= 0 ? 1 : -1;
      const z0 = part.min[2];
      const lines = [];
      if (Math.abs(d[1]) > 0.05 || Math.abs(d[2]) > 0.05)
        lines.push({ a: [part.min[0], fy, z0], b: [part.max[0], fy, z0], o: [0, sy * off, d[2] > 0.05 ? 0 : -off] });
      if (Math.abs(d[0]) > 0.05 || Math.abs(d[2]) > 0.05)
        lines.push({ a: [fx, part.min[1], z0], b: [fx, part.max[1], z0], o: [sx * off, 0, d[2] > 0.05 ? 0 : -off] });
      if (Math.abs(d[2]) < 0.995)
        lines.push({ a: [fx, fy, part.min[2]], b: [fx, fy, part.max[2]], o: Math.abs(d[0]) > Math.abs(d[1]) ? [0, sy * off, 0] : [sx * off, 0, 0] });
      for (const ln of lines) {
        const len = Math.hypot(ln.b[0] - ln.a[0], ln.b[1] - ln.a[1], ln.b[2] - ln.a[2]);
        const A = this.project(ln.a);
        const B = this.project(ln.b);
        const A2 = this.project(ln.a.map((v, k) => v + ln.o[k]));
        const B2 = this.project(ln.b.map((v, k) => v + ln.o[k]));
        if (Math.hypot(B2[0] - A2[0], B2[1] - A2[1]) < 14) continue;
        const stroke = { stroke: t.dimColor, "stroke-width": 1 };
        el("line", Object.assign({ x1: A[0], y1: A[1], x2: A2[0], y2: A2[1], "stroke-dasharray": "2 2" }, stroke), g);
        el("line", Object.assign({ x1: B[0], y1: B[1], x2: B2[0], y2: B2[1], "stroke-dasharray": "2 2" }, stroke), g);
        el("line", Object.assign({ x1: A2[0], y1: A2[1], x2: B2[0], y2: B2[1] }, stroke), g);
        for (const P0 of [A2, B2]) el("line", Object.assign({ x1: P0[0] - 4, y1: P0[1] + 4, x2: P0[0] + 4, y2: P0[1] - 4 }, stroke, { "stroke-width": 1.4 }), g);
        const mx = (A2[0] + B2[0]) / 2;
        const my = (A2[1] + B2[1]) / 2;
        const label = `${mm(len)}`;
        const w = label.length * 7 + 10;
        el("rect", { x: mx - w / 2, y: my - 9, width: w, height: 18, rx: 4, fill: t.dimBg }, g);
        const text = el("text", { x: mx, y: my + 4, "text-anchor": "middle", fill: t.dimText, style: `font:${t.dimFont}` }, g);
        text.textContent = label;
      }
    }
  }

  window.BenchModel = { PARTS, MATERIALS, size, fmtDims, mm, BenchView };
})();
