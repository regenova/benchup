// Shop drawing helpers for prototype A: joinery features of the sample bench,
// orthographic and isometric line drawings, dimensions and unit formatting.
// Coordinates on a sheet are millimetres of paper; model coordinates are cm.
(function () {
  "use strict";
  const { PARTS, size } = BenchModel;
  const NS = "http://www.w3.org/2000/svg";
  const INK = "#262b33";
  const DIM = "#5a6170";
  const ACCENT = "#d63b32";

  const el = (name, attrs, parent, text) => {
    const n = document.createElementNS(NS, name);
    for (const k in attrs) n.setAttribute(k, attrs[k]);
    if (text != null) n.textContent = text;
    if (parent) parent.appendChild(n);
    return n;
  };

  // ---------------------------------------------------------------- joinery
  // Every apron and stretcher ends in a 10 mm tenon, 20 mm long, that sits in a
  // mortise of the same size in the leg it meets.
  const contains = (outer, inner) => [0, 1, 2].every((k) => inner.min[k] >= outer.min[k] - 1e-6 && inner.max[k] <= outer.max[k] + 1e-6);
  const JOINTS = [];
  for (const p of PARTS) {
    if (p.kind !== "Aprons" && p.kind !== "Stretchers") continue;
    const s = size(p);
    const along = s.x > s.y ? 0 : 1;
    const across = along === 0 ? 1 : 0;
    const mid = (p.min[across] + p.max[across]) / 2;
    const tall = p.max[2] - p.min[2];
    const z0 = p.min[2] + (tall > 6 ? 1.0 : 0.75);
    const z1 = p.max[2] - (tall > 6 ? 1.5 : 0.75);
    for (const end of [0, 1]) {
      const min = [0, 0, 0], max = [0, 0, 0];
      if (end === 0) { min[along] = p.min[along] - 2; max[along] = p.min[along]; }
      else { min[along] = p.max[along]; max[along] = p.max[along] + 2; }
      min[across] = mid - 0.5; max[across] = mid + 0.5;
      min[2] = z0; max[2] = z1;
      const box = { min, max };
      const leg = PARTS.find((q) => q.kind === "Legs" && contains(q, box));
      JOINTS.push({ member: p.id, leg: leg && leg.id, box });
    }
  }
  const tenonsOf = (id) => JOINTS.filter((j) => j.member === id).map((j) => j.box);
  const mortisesOf = (id) => JOINTS.filter((j) => j.leg === id).map((j) => ({ ...j.box, from: j.member }));

  // ---------------------------------------------------------------- units
  function fmt(mm, units) {
    if (units === "in") return `${(mm / 25.4).toFixed(2)}″`;
    if (units === "frac") {
      const sixteenths = Math.round((mm / 25.4) * 16);
      const whole = Math.floor(sixteenths / 16);
      let rest = sixteenths % 16;
      let den = 16;
      while (rest && rest % 2 === 0) { rest /= 2; den /= 2; }
      if (!rest) return `${whole}″`;
      return whole ? `${whole} ${rest}/${den}″` : `${rest}/${den}″`;
    }
    const r = Math.round(mm * 10) / 10;
    return Number.isInteger(r) ? `${r}` : r.toFixed(1);
  }

  // ---------------------------------------------------------------- dimensions
  // Horizontal dimension between paper x1..x2, measured objects at y=ref, line at y.
  function dimH(g, x1, x2, ref, y, label, opts = {}) {
    if (Math.abs(x2 - x1) < 0.3) return;
    const st = { stroke: DIM, "stroke-width": 0.18, fill: "none" };
    const dir = y < ref ? -1 : 1;
    el("line", { x1, y1: ref + dir * 1, x2: x1, y2: y + dir * 1.5, ...st }, g);
    el("line", { x1: x2, y1: ref + dir * 1, x2, y2: y + dir * 1.5, ...st }, g);
    el("line", { x1, y1: y, x2, y2: y, ...st }, g);
    for (const x of [x1, x2]) el("line", { x1: x - 1, y1: y + 1, x2: x + 1, y2: y - 1, stroke: DIM, "stroke-width": 0.3 }, g);
    const fs = opts.size || 2.6;
    const w = label.length * fs * 0.55 + 1.6;
    const mx = (x1 + x2) / 2;
    // A label that does not fit between the arrows sits beside the line, outside.
    const fitsInside = Math.abs(x2 - x1) > w + 1;
    const ty = fitsInside ? y : y + dir * (fs * 1.2);
    el("rect", { x: mx - w / 2, y: ty - fs * 0.62, width: w, height: fs * 1.15, fill: "#fffdf9" }, g);
    el("text", { x: mx, y: ty + fs * 0.35, "font-size": fs, "text-anchor": "middle", fill: INK, class: "num" }, g, label);
  }
  // Vertical dimension between paper y1..y2, objects at x=ref, line at x.
  function dimV(g, y1, y2, ref, x, label, opts = {}) {
    if (Math.abs(y2 - y1) < 0.3) return;
    const st = { stroke: DIM, "stroke-width": 0.18, fill: "none" };
    const dir = x < ref ? -1 : 1;
    el("line", { x1: ref + dir * 1, y1, x2: x + dir * 1.5, y2: y1, ...st }, g);
    el("line", { x1: ref + dir * 1, y1: y2, x2: x + dir * 1.5, y2, ...st }, g);
    el("line", { x1: x, y1, x2: x, y2, ...st }, g);
    for (const y of [y1, y2]) el("line", { x1: x - 1, y1: y + 1, x2: x + 1, y2: y - 1, stroke: DIM, "stroke-width": 0.3 }, g);
    const fs = opts.size || 2.6;
    const w = label.length * fs * 0.55 + 1.6;
    const my = (y1 + y2) / 2;
    const fitsInside = Math.abs(y2 - y1) > w + 1;
    const tx = fitsInside ? x : x + dir * (fs * 1.2);
    const t = el("g", { transform: `translate(${tx} ${my}) rotate(-90)` }, g);
    el("rect", { x: -w / 2, y: -fs * 0.62, width: w, height: fs * 1.15, fill: "#fffdf9" }, t);
    el("text", { x: 0, y: fs * 0.35, "font-size": fs, "text-anchor": "middle", fill: INK, class: "num" }, t, label);
  }
  // Chain or baseline run of dimensions along one axis.
  function dimRun(g, points, horizontal, ref, at, units, style, step = 5) {
    const sorted = [...new Set(points.map((p) => Math.round(p.paper * 100) / 100))].sort((a, b) => a - b);
    const values = points.slice().sort((a, b) => a.paper - b.paper);
    const uniq = [];
    for (const p of values) if (!uniq.length || Math.abs(uniq[uniq.length - 1].paper - p.paper) > 0.01) uniq.push(p);
    if (uniq.length < 2) return;
    const dir = at < ref ? -1 : 1;
    if (style === "baseline") {
      for (let i = 1; i < uniq.length; i++) {
        const pos = at + dir * (i - 1) * step;
        const label = fmt(Math.abs(uniq[i].value - uniq[0].value), units);
        if (horizontal) dimH(g, uniq[0].paper, uniq[i].paper, ref, pos, label);
        else dimV(g, uniq[0].paper, uniq[i].paper, ref, pos, label);
      }
    } else {
      for (let i = 1; i < uniq.length; i++) {
        const label = fmt(Math.abs(uniq[i].value - uniq[i - 1].value), units);
        if (horizontal) dimH(g, uniq[i - 1].paper, uniq[i].paper, ref, at, label);
        else dimV(g, uniq[i - 1].paper, uniq[i].paper, ref, at, label);
      }
    }
    return sorted;
  }

  // ---------------------------------------------------------------- ortho
  // Axis-aligned boxes seen along `view` ("front": from -y with x to the right,
  // "side": from +x with y to the right, "plan": from +z with y = 0 at the bottom). Hidden edges are drawn dashed on top: where an edge is
  // visible the dashes coincide with its solid line, elsewhere only dashes show.
  const VIEWS = {
    front: { h: 0, v: 2, depth: 1, ds: -1, hs: 1, vs: 1 },
    side: { h: 1, v: 2, depth: 0, ds: 1, hs: 1, vs: 1 },
    plan: { h: 0, v: 1, depth: 2, ds: 1, hs: 1, vs: 1 },
  };
  function viewExtent(boxes, view) {
    const V = VIEWS[view];
    const lo = [Infinity, Infinity], hi = [-Infinity, -Infinity];
    for (const b of boxes) {
      lo[0] = Math.min(lo[0], b.min[V.h]); hi[0] = Math.max(hi[0], b.max[V.h]);
      lo[1] = Math.min(lo[1], b.min[V.v]); hi[1] = Math.max(hi[1], b.max[V.v]);
    }
    return { w: hi[0] - lo[0], h: hi[1] - lo[1], lo, hi };
  }
  function orthoBoxes(g, boxes, view, ox, oy, s, opts = {}) {
    const V = VIEWS[view];
    const ext = viewExtent(boxes, view);
    const X = (c) => (V.hs > 0 ? ox + (c - ext.lo[0]) * s : ox + (ext.hi[0] - c) * s);
    const Y = (c) => (V.vs > 0 ? oy + (ext.hi[1] - c) * s : oy + (c - ext.lo[1]) * s);
    const rect = (b) => {
      const xa = X(b.min[V.h]), xb = X(b.max[V.h]);
      const ya = Y(b.min[V.v]), yb = Y(b.max[V.v]);
      return { x: Math.min(xa, xb), y: Math.min(ya, yb), w: Math.abs(xb - xa), h: Math.abs(yb - ya) };
    };
    // Far boxes first, so nearer ones cover them.
    const sorted = boxes.slice().sort((a, b) => V.ds * ((a.min[V.depth] + a.max[V.depth]) - (b.min[V.depth] + b.max[V.depth])));
    for (const b of sorted) {
      const r = rect(b);
      el("rect", { x: r.x, y: r.y, width: r.w, height: r.h, fill: b.highlight ? "#fbe9e6" : "#fffdf9", stroke: b.highlight ? ACCENT : INK, "stroke-width": 0.35 }, g);
    }
    if (opts.hidden) {
      for (const b of sorted) {
        const r = rect(b);
        el("rect", { x: r.x, y: r.y, width: r.w, height: r.h, fill: "none", stroke: INK, "stroke-width": 0.22, "stroke-dasharray": "1.6 1" }, g);
      }
    }
    return { X, Y, ext, w: ext.w * s, h: ext.h * s };
  }

  // ---------------------------------------------------------------- iso
  function isoBasis(az = 52, elv = 28) {
    const a = (az * Math.PI) / 180, e = (elv * Math.PI) / 180;
    const d = [Math.cos(e) * Math.cos(a), Math.cos(e) * Math.sin(a), Math.sin(e)];
    const rl = Math.hypot(d[1], d[0]);
    const right = [d[1] / rl, -d[0] / rl, 0];
    const up = [right[1] * d[2] - right[2] * d[1], right[2] * d[0] - right[0] * d[2], right[0] * d[1] - right[1] * d[0]];
    return { d, right, up };
  }
  const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
  function faces(b) {
    const [x0, y0, z0] = b.min, [x1, y1, z1] = b.max;
    return [
      { n: [1, 0, 0], p: [[x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1]] },
      { n: [-1, 0, 0], p: [[x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]] },
      { n: [0, 1, 0], p: [[x0, y1, z0], [x0, y1, z1], [x1, y1, z1], [x1, y1, z0]] },
      { n: [0, -1, 0], p: [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]] },
      { n: [0, 0, 1], p: [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]] },
      { n: [0, 0, -1], p: [[x0, y0, z0], [x0, y1, z0], [x1, y1, z0], [x1, y0, z0]] },
    ];
  }
  // Fits `boxes` (with optional per-box offset) into the paper rect and draws
  // them with painter's order. Returns a projector for balloons.
  function isoBoxes(g, boxes, rect, opts = {}) {
    const B = isoBasis(opts.azimuth, opts.elevation);
    const moved = boxes.map((b) => {
      const o = b.offset || [0, 0, 0];
      return { ...b, min: b.min.map((v, k) => v + o[k]), max: b.max.map((v, k) => v + o[k]) };
    });
    const pts = [];
    for (const b of moved) for (const x of [b.min[0], b.max[0]]) for (const y of [b.min[1], b.max[1]]) for (const z of [b.min[2], b.max[2]]) pts.push([dot([x, y, z], B.right), -dot([x, y, z], B.up)]);
    const lo = [Math.min(...pts.map((p) => p[0])), Math.min(...pts.map((p) => p[1]))];
    const hi = [Math.max(...pts.map((p) => p[0])), Math.max(...pts.map((p) => p[1]))];
    const s = Math.min(rect.w / (hi[0] - lo[0]), rect.h / (hi[1] - lo[1])) * (opts.fill || 0.92);
    const ox = rect.x + (rect.w - (hi[0] - lo[0]) * s) / 2 - lo[0] * s;
    const oy = rect.y + (rect.h - (hi[1] - lo[1]) * s) / 2 - lo[1] * s;
    const P = (p) => [ox + dot(p, B.right) * s, oy - dot(p, B.up) * s];
    const d = B.d;
    const behind = (A, C) => [0, 1, 2].some((k) => (d[k] > 1e-6 && A.max[k] <= C.min[k] + 1e-6) || (d[k] < -1e-6 && A.min[k] >= C.max[k] - 1e-6));
    const depth = (b) => dot([0, 1, 2].map((k) => (b.min[k] + b.max[k]) / 2), d);
    const n = moved.length, indeg = Array(n).fill(0), out = moved.map(() => []);
    for (let i = 0; i < n; i++) for (let j = 0; j < n; j++) if (i !== j && behind(moved[i], moved[j]) && !behind(moved[j], moved[i])) { out[i].push(j); indeg[j]++; }
    const order = [], done = Array(n).fill(false);
    while (order.length < n) {
      let pick = -1;
      for (let i = 0; i < n; i++) if (!done[i] && indeg[i] === 0 && (pick < 0 || depth(moved[i]) < depth(moved[pick]))) pick = i;
      if (pick < 0) for (let i = 0; i < n; i++) if (!done[i] && (pick < 0 || depth(moved[i]) < depth(moved[pick]))) pick = i;
      done[pick] = true; order.push(pick);
      for (const j of out[pick]) indeg[j]--;
    }
    const light = [0.35, 0.55, 0.85];
    const ll = Math.hypot(...light);
    for (const i of order) {
      const b = moved[i];
      for (const f of faces(b)) {
        if (dot(f.n, d) <= 1e-6) continue;
        const k = 0.9 + 0.1 * Math.max(0, dot(f.n, light) / ll);
        const base = b.highlight ? [251, 233, 230] : [255, 253, 249];
        const fill = `rgb(${base.map((c) => Math.round(c * k)).join(",")})`;
        el("polygon", { points: f.p.map((p) => P(p).join(",")).join(" "), fill, stroke: b.highlight ? ACCENT : INK, "stroke-width": opts.stroke || 0.3, "stroke-linejoin": "round" }, g);
      }
    }
    return { P, moved };
  }

  function balloon(g, at, to, label) {
    el("line", { x1: at[0], y1: at[1], x2: to[0], y2: to[1], stroke: ACCENT, "stroke-width": 0.25 }, g);
    el("circle", { cx: to[0], cy: to[1], r: 0.7, fill: ACCENT }, g);
    el("circle", { cx: at[0], cy: at[1], r: 3.4, fill: "#fffdf9", stroke: ACCENT, "stroke-width": 0.35 }, g);
    el("text", { x: at[0], y: at[1] + 1.15, "font-size": 3.2, "text-anchor": "middle", fill: ACCENT, "font-weight": 600 }, g, label);
  }

  window.Drawings = { el, fmt, dimH, dimV, dimRun, orthoBoxes, viewExtent, isoBoxes, balloon, JOINTS, tenonsOf, mortisesOf, INK, DIM, ACCENT };
})();
