// BenchUp UI prototypes: isometric sketches of workshop tools (96 × 64 viewBox).
// Every tool is modelled with a few simple solids in 3D and projected with true
// isometric axes (30°), so the whole set reads as one family of technical drawings:
// a thin ink line, a heavier outer silhouette, three tints per material for the three
// visible faces, and a few draughtsman's cues (overshooting corners, centre lines,
// hidden edges, hatching) that disappear at small sizes. Colours come from CSS
// variables so the art follows the theme. Use toolArt(id, width) to get an <svg>.
// The previous flat side-view set is kept, unreferenced, in tool-art-flat.js.
(function () {
  "use strict";

  // ---------- Vector helpers ----------
  const COS30 = Math.cos(Math.PI / 6);
  const VIEW = [1, 1, 1]; // the viewer looks down the (-1, -1, -1) direction
  const proj = (p) => [(p[0] - p[1]) * COS30, (p[0] + p[1]) / 2 - p[2]];
  const add = (a, b) => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
  const sub = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
  const mul = (a, k) => [a[0] * k, a[1] * k, a[2] * k];
  const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
  const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
  const len = (a) => Math.hypot(a[0], a[1], a[2]);
  const unit = (a) => mul(a, 1 / (len(a) || 1));
  const rad = (a) => (a * Math.PI) / 180;
  const centroid = (pts) => mul(pts.reduce(add, [0, 0, 0]), 1 / pts.length);
  const AXIS = { x: [1, 0, 0], y: [0, 1, 0], z: [0, 0, 1] };

  function newell(pts) {
    const n = [0, 0, 0];
    for (let i = 0; i < pts.length; i++) {
      const a = pts[i], b = pts[(i + 1) % pts.length];
      n[0] += (a[1] - b[1]) * (a[2] + b[2]);
      n[1] += (a[2] - b[2]) * (a[0] + b[0]);
      n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    return n;
  }
  // Top faces take the lightest tint, faces turned left the middle one, right the darkest.
  const face = (n) => (n[2] >= n[0] && n[2] >= n[1] ? "f1" : n[1] >= n[0] ? "f2" : "f3");

  // ---------- 2D profiles (u, v) ----------
  const arc = (cx, cy, r, a0, a1, n = 8) =>
    Array.from({ length: n + 1 }, (_, i) => {
      const a = rad(a0 + ((a1 - a0) * i) / n);
      return [cx + r * Math.cos(a), cy + r * Math.sin(a)];
    });
  const circle = (cx, cy, r, n = 24) => arc(cx, cy, r, 0, 360, n).slice(0, n);
  // Rounded rectangle; r is one radius or [bottom-right, top-right, top-left, bottom-left].
  function rr(x0, y0, x1, y1, r, n = 4) {
    const [a, b, c, d] = Array.isArray(r) ? r : [r, r, r, r];
    const corner = (cx, cy, rad_, a0) => (rad_ ? arc(cx, cy, rad_, a0, a0 + 90, n) : [[cx, cy]]);
    return [
      ...corner(x1 - a, y0 + a, a, -90),
      ...corner(x1 - b, y1 - b, b, 0),
      ...corner(x0 + c, y1 - c, c, 90),
      ...corner(x0 + d, y0 + d, d, 180),
    ];
  }
  // Toothed disc: alternating outer and gullet radii.
  function toothed(cx, cy, r, teeth, depth) {
    const pts = [];
    for (let i = 0; i < teeth; i++) {
      const a = (i / teeth) * Math.PI * 2, b = ((i + 0.7) / teeth) * Math.PI * 2;
      pts.push([cx + r * Math.cos(a), cy + r * Math.sin(a)], [cx + (r - depth) * Math.cos(b), cy + (r - depth) * Math.sin(b)]);
    }
    return pts;
  }
  const PLANE = { xz: (u, v, w) => [u, w, v], yz: (u, v, w) => [w, u, v], xy: (u, v, w) => [u, v, w] };
  const on = (prof, plane, w) => prof.map(([u, v]) => PLANE[plane](u, v, w));

  // Convex polygon clipping (Sutherland–Hodgman) in screen space, used for holes.
  function clip(subject, clipper) {
    let area = 0;
    for (let i = 0; i < clipper.length; i++) {
      const a = clipper[i], b = clipper[(i + 1) % clipper.length];
      area += a[0] * b[1] - b[0] * a[1];
    }
    const s = area >= 0 ? 1 : -1;
    const inside = (p, a, b) => s * ((b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])) >= 0;
    const meet = (p, q, a, b) => {
      const d = (p[0] - q[0]) * (a[1] - b[1]) - (p[1] - q[1]) * (a[0] - b[0]);
      const t = ((p[0] - a[0]) * (a[1] - b[1]) - (p[1] - a[1]) * (a[0] - b[0])) / d;
      return [p[0] + t * (q[0] - p[0]), p[1] + t * (q[1] - p[1])];
    };
    let out = subject;
    for (let i = 0; i < clipper.length && out.length; i++) {
      const a = clipper[i], b = clipper[(i + 1) % clipper.length];
      const input = out;
      out = [];
      for (let j = 0; j < input.length; j++) {
        const p = input[j], q = input[(j + 1) % input.length];
        const pin = inside(p, a, b), qin = inside(q, a, b);
        if (pin) out.push(p);
        if (pin !== qin) out.push(meet(p, q, a, b));
      }
    }
    return out;
  }

  // ---------- Sketch builder ----------
  // Shapes are projected as they are added and fitted into the viewBox at render time,
  // so every drawing fills a similar area whatever units it was modelled in.
  function Sketch() {
    this.items = [];
    this.fit = [84, 50];
  }
  Sketch.prototype = {
    poly2(pts, cls, sil = true) {
      if (pts.length > 2) this.items.push({ t: "p", pts, cls, sil });
    },
    poly(pts, m, f, extra) {
      this.poly2(pts.map(proj), `${m} ${f}${extra ? " " + extra : ""}`, !(extra && /\bx\b/.test(extra)));
    },
    // Lines: one polyline or a list of polylines, in 3D.
    line(paths, cls = "ln") {
      if (!paths.length) return;
      if (!Array.isArray(paths[0][0])) paths = [paths];
      this.items.push({ t: "l", paths: paths.map((pl) => pl.map(proj)), cls });
    },
    cl(a, b) {
      this.line([a, b], "x cl");
    },
    // Flat polygon, tinted by the way it faces.
    F(pts, m, o = {}) {
      let n = newell(pts);
      if (dot(n, VIEW) < 0) n = mul(n, -1);
      this.poly(pts, m, o.f || face(n), o.cls);
    },
    // Axis-aligned box. o.h hatches the darkest face, o.os overshoots the silhouette
    // corners, o.hd dashes the three hidden edges.
    B(x, y, z, dx, dy, dz, m, o = {}) {
      const p = (a, b, c) => [x + a * dx, y + b * dy, z + c * dz];
      this.poly([p(1, 0, 0), p(1, 1, 0), p(1, 1, 1), p(1, 0, 1)], m, "f3");
      if (o.h) this.hatch([p(1, 0, 0), p(1, 1, 0), p(1, 1, 1), p(1, 0, 1)]);
      this.poly([p(0, 1, 0), p(1, 1, 0), p(1, 1, 1), p(0, 1, 1)], m, "f2");
      this.poly([p(0, 0, 1), p(1, 0, 1), p(1, 1, 1), p(0, 1, 1)], m, "f1");
      if (o.hd) this.line([[p(0, 0, 0), p(1, 0, 0)], [p(0, 0, 0), p(0, 1, 0)], [p(0, 0, 0), p(0, 0, 1)]], "x hd");
      if (o.os) {
        const hex = [p(0, 0, 1), p(1, 0, 1), p(1, 0, 0), p(1, 1, 0), p(0, 1, 0), p(0, 1, 1)];
        hex.forEach((a, i) => this.over(a, hex[(i + 1) % 6]));
        this.over(p(1, 1, 1), p(1, 1, 0));
      }
    },
    // A solid between two matching rings of points: prisms, cylinders, frusta.
    loft(back, front, m, o = {}) {
      let ax = sub(centroid(front), centroid(back));
      if (dot(ax, VIEW) < 0) {
        [back, front] = [front, back];
        ax = mul(ax, -1);
      }
      const n = back.length;
      const wind = dot(newell(back), ax) >= 0 ? 1 : -1;
      const quads = [];
      for (let i = 0; i < n; i++) {
        const j = (i + 1) % n;
        const a = back[i], b = back[j], c = front[j], e = front[i];
        const out = mul(cross(sub(b, a), ax), wind);
        let nq = cross(sub(b, a), sub(e, a));
        if (len(nq) < 1e-9) nq = cross(sub(c, e), sub(e, a));
        if (len(nq) < 1e-9) nq = out;
        if (dot(nq, out) < 0) nq = mul(nq, -1);
        const vis = len(sub(b, a)) + len(sub(c, e)) > 1e-6 && dot(unit(nq), VIEW) > 1e-4;
        quads.push({ pts: [a, b, c, e], n: nq, vis, depth: dot(add(add(a, b), add(c, e)), VIEW) });
      }
      quads.filter((q) => q.vis).sort((p, q) => p.depth - q.depth).forEach((q) => this.poly(q.pts, m, face(q.n), "ns"));
      const paths = [];
      if (o.edges !== false) {
        let run = null;
        const start = quads.findIndex((q) => !q.vis);
        for (let k = 0; k <= n; k++) {
          const i = ((start < 0 ? 0 : start) + k) % n;
          if (quads[i].vis && k < n) {
            if (!run) paths.push((run = [back[i]]));
            run.push(back[(i + 1) % n]);
          } else run = null;
        }
      }
      if (o.lat !== false) {
        const sharp = Math.cos(rad(o.sharp || 32));
        for (let i = 0; i < n; i++) {
          const qa = quads[(i - 1 + n) % n], qb = quads[i];
          let draw = qa.vis !== qb.vis;
          if (!draw && qa.vis) {
            const da = sub(back[i], back[(i - 1 + n) % n]), db = sub(back[(i + 1) % n], back[i]);
            draw = len(da) > 1e-6 && len(db) > 1e-6 && dot(unit(da), unit(db)) < sharp;
          }
          if (draw) paths.push([back[i], front[i]]);
        }
      }
      if (paths.length) this.line(paths, "ln");
      if (o.cap !== false) this.F(front, m);
      return front;
    },
    X(prof, plane, w0, w1, m, o) {
      return this.loft(on(prof, plane, w0), on(prof, plane, w1), m, o);
    },
    ring(c, dir, r, n = 24, phase = 0) {
      const u = unit(dir);
      const e1 = unit(cross(u, Math.abs(u[2]) < 0.9 ? [0, 0, 1] : [1, 0, 0]));
      const e2 = cross(u, e1);
      return Array.from({ length: n }, (_, k) => {
        const t = rad(phase) + (k / n) * Math.PI * 2;
        return add(c, add(mul(e1, r * Math.cos(t)), mul(e2, r * Math.sin(t))));
      });
    },
    // Cylinder (or frustum with o.r2) between two points.
    rod(a, b, r, m, o = {}) {
      const dir = sub(b, a), n = o.n || 24;
      return this.loft(this.ring(a, dir, r, n, o.phase), this.ring(b, dir, o.r2 == null ? r : o.r2, n, o.phase), m, o);
    },
    Cy(axis, base, r, length, m, o) {
      return this.rod(base, add(base, mul(AXIS[axis], length)), r, m, o);
    },
    // Solid of revolution: rings [[t, r], ...] along a positive axis, drawn smooth.
    lathe(axis, base, rings, m, o = {}) {
      const dir = AXIS[axis], n = o.n || 24;
      // Two rings at the same t make a flat step: cap the segment below, outline the one above.
      for (let k = 0; k < rings.length - 1; k++) {
        const [t0, r0] = rings[k], [t1, r1] = rings[k + 1];
        if (t1 - t0 < 0.01) continue;
        const last = k === rings.length - 2;
        const flatNext = !last && rings[k + 2][0] - t1 < 0.01;
        const flatPrev = k > 0 && t0 - rings[k - 1][0] < 0.01;
        this.loft(this.ring(add(base, mul(dir, t0)), dir, r0, n), this.ring(add(base, mul(dir, t1)), dir, r1, n), m, {
          edges: !!(k === 0 || flatPrev || o.rings),
          cap: !!(last || flatNext),
        });
      }
    },
    // Hatching on a convex face, generated in screen space at render time.
    hatch(pts, step = 2.1) {
      this.items.push({ t: "h", pts: pts.map(proj), step });
    },
    // Overshoot: short extensions past both ends of an edge, like a quick pencil sketch.
    over(a, b) {
      this.items.push({ t: "o", a: proj(a), b: proj(b) });
    },
    // A through hole: the far opening shows the background, the rest is the inner wall.
    hole(top, depth, m) {
      const t = top.map(proj), bot = top.map((p) => proj(add(p, depth)));
      this.poly2(t, `${m} f3`, false);
      this.poly2(clip(bot, t), "h f1", false);
      this.items.push({ t: "l", paths: [[...t, t[0]]], cls: "ln" });
    },

    render() {
      let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
      const grow = (p) => {
        x0 = Math.min(x0, p[0]); y0 = Math.min(y0, p[1]);
        x1 = Math.max(x1, p[0]); y1 = Math.max(y1, p[1]);
      };
      for (const it of this.items) {
        if (it.t === "p" && it.sil) it.pts.forEach(grow);
        if (it.t === "l" && !/\bx\b/.test(it.cls)) it.paths.forEach((pl) => pl.forEach(grow));
      }
      const [fw, fh] = this.fit;
      const s = Math.min(fw / (x1 - x0), fh / (y1 - y0));
      const ox = 48 - ((x0 + x1) / 2) * s, oy = 32 - ((y0 + y1) / 2) * s;
      const T = (p) => `${(ox + p[0] * s).toFixed(1)} ${(oy + p[1] * s).toFixed(1)}`;
      const S = (p) => [ox + p[0] * s, oy + p[1] * s];
      const closed = (pts) => `M${pts.map(T).join("L")}Z`;
      const sil = this.items.filter((it) => it.t === "p" && it.sil).map((it) => closed(it.pts)).join("");
      let out = `<path class="sil" d="${sil}"/>`;
      for (const it of this.items) {
        if (it.t === "p") out += `<path class="${it.cls} fl" d="${closed(it.pts)}"/>`;
        else if (it.t === "l") out += `<path class="${it.cls}" d="${it.paths.map((pl) => `M${pl.map(T).join("L")}`).join("")}"/>`;
        else if (it.t === "o") {
          const a = S(it.a), b = S(it.b);
          const L = Math.hypot(b[0] - a[0], b[1] - a[1]) || 1;
          const e = [((b[0] - a[0]) / L) * 2.4, ((b[1] - a[1]) / L) * 2.4];
          const f = (p) => `${p[0].toFixed(1)} ${p[1].toFixed(1)}`;
          out += `<path class="x os" d="M${f(a)}L${f([a[0] - e[0], a[1] - e[1]])}M${f(b)}L${f([b[0] + e[0], b[1] + e[1]])}"/>`;
        } else if (it.t === "h") {
          const pts = it.pts.map(S), dir = [Math.cos(rad(-58)), Math.sin(rad(-58))], nrm = [-dir[1], dir[0]];
          const pr = (p, v) => p[0] * v[0] + p[1] * v[1];
          const cs = pts.map((p) => pr(p, nrm));
          let d = "";
          for (let c = Math.min(...cs) + it.step * 0.7; c < Math.max(...cs) - it.step * 0.3; c += it.step) {
            const hits = [];
            pts.forEach((p, i) => {
              const q = pts[(i + 1) % pts.length], cp = pr(p, nrm), cq = pr(q, nrm);
              if ((cp - c) * (cq - c) < 0) {
                const t = (c - cp) / (cq - cp);
                hits.push([p[0] + t * (q[0] - p[0]), p[1] + t * (q[1] - p[1])]);
              }
            });
            if (hits.length < 2) continue;
            hits.sort((p, q) => pr(p, dir) - pr(q, dir));
            const a = hits[0], b = hits[hits.length - 1], k = 0.8;
            d += `M${(a[0] + dir[0] * k).toFixed(1)} ${(a[1] + dir[1] * k).toFixed(1)}L${(b[0] - dir[0] * k).toFixed(1)} ${(b[1] - dir[1] * k).toFixed(1)}`;
          }
          if (d) out += `<path class="x ht" d="${d}"/>`;
        }
      }
      return out;
    },
  };

  // ---------- Shared parts ----------
  // Saw blade outline in the xz plane with teeth along z = 0.
  function sawBlade(x0, x1, hTip, hHeel, step = 3, depth = 2.6) {
    const pts = [[x0, hTip], [x1, hHeel], [x1, 0]];
    for (let x = x1; x > x0 + 0.1; x -= step) pts.push([x - step * 0.45, -depth], [Math.max(x - step, x0), 0]);
    return pts;
  }
  function benchPlane(d, L) {
    const k = L / 92, at = (v) => v * k;
    const i0 = at(37);
    d.X([[0, 0], [L, 0], [L, 6], [at(62), 6], [at(56), 11], [at(34), 11], [at(28), 6], [0, 6]], "xz", 0, 22, "d");
    d.hatch([[0, 22, 0], [at(28), 22, 0], [at(28), 22, 6], [0, 22, 6]], 2.4);
    // Iron and lever cap rise out of the frog between the cheeks.
    d.X([[i0 + 4, 11], [i0 + 10, 11], [i0 + 21, 25], [i0 + 16, 28]], "xz", 3, 19, "s");
    d.X([[i0 + 8, 11], [i0 + 13, 11], [i0 + 20, 20], [i0 + 16, 22]], "xz", 5, 17, "s");
    d.Cy("y", [i0 + 22, 9.5, 16], 4.4, 3, "g");
    const t0 = L - 27;
    d.X([[t0, 6], [t0 + 10, 6], [t0 + 17, 25], [t0 + 17, 30], [t0 + 12, 32], [t0 + 8, 27]], "xz", 8, 14, "w");
    d.lathe("z", [at(15), 11, 6], [[0, 4.4], [3, 5.6], [9, 5.4], [12, 3.8], [13.4, 1.6]], "w");
    d.cl([at(15), 11, 2], [at(15), 11, 24]);
  }
  function chisel(d, y0, bw, x1) {
    const yc = y0 + bw / 2;
    d.X([[0, y0], [x1 - 6, y0], [x1, yc - 2.4], [x1, yc + 2.4], [x1 - 6, y0 + bw], [0, y0 + bw]], "xy", 3, 5.2, "s");
    d.line([[5, y0, 5.2], [5, y0 + bw, 5.2]], "ln e2");
    d.Cy("x", [x1, yc, 4.2], 3.2, 7, "s");
    d.rod([x1 + 7, yc, 4.2], [x1 + 42, yc, 4.2], 4.6, "w", { n: 8, r2: 5.8, phase: 22.5 });
  }
  // Bits stand shank-down so the cutting end faces the viewer.
  const onTop = (x, y, r, a, z) => [x + r * Math.cos(rad(a)), y + r * Math.sin(rad(a)), z];
  function bit(d, x, y, rBody, hBody, rShank, hShank) {
    const top = hShank + hBody;
    d.Cy("z", [x, y, 0], rShank, hShank + 1, "s", { n: 16 });
    d.Cy("z", [x, y, hShank], rBody, hBody, "b");
    d.F([[x, y, hShank + 1], [x + rBody + 0.7, y, hShank + 1], [x + rBody + 0.7, y, top + 0.8], [x, y, top + 0.8]], "s");
    d.F([[x, y, top + 0.05], onTop(x, y, rBody, 95, top + 0.05), onTop(x, y, rBody, 150, top + 0.05)], "d");
    d.line([onTop(x, y, rBody, 0, top + 0.05), onTop(x, y, rBody, 180, top + 0.05)], "ln e2");
    d.cl([x, y, -5], [x, y, top + 6]);
  }
  function forstnerBit(d, x, y, R, H, rs, hs) {
    const top = hs + H;
    d.Cy("z", [x, y, 0], rs, hs + 1, "s", { n: 16 });
    d.Cy("z", [x, y, hs], R, H, "s", { n: 28 });
    const zig = [];
    for (let a = 0; a <= 360; a += 10) zig.push(onTop(x, y, a % 20 ? R - 1.3 : R - 0.3, a, top + 0.05));
    d.line(zig, "ln e2");
    for (const a0 of [15, 195]) {
      d.F([[x, y, top + 0.05], onTop(x, y, R - 1.6, a0, top + 0.05), onTop(x, y, R - 1.6, a0 + 32, top + 0.05)], "d");
      d.line([[x, y, top + 0.1], onTop(x, y, R - 1.6, a0, top + 0.1)], "ln");
    }
    d.rod([x, y, top], [x, y, top + 3.4], 1.4, "s", { r2: 0.05, n: 10 });
    d.cl([x, y, -5], [x, y, top + 8]);
  }

  // ---------- The tools ----------
  const ART = {
    // ---------- Measuring & marking ----------
    tape(d) {
      d.X(rr(0, 0, 40, 40, [3, 12, 12, 12]), "xz", 0, 20, "b");
      d.F(on(circle(20, 21, 9.5), "xz", 20.05), "s");
      d.F(on(circle(20, 21, 2.6, 12), "xz", 20.1), "d");
      d.cl([20, 20.1, 7], [20, 20.1, 35]);
      d.cl([6, 20.1, 21], [34, 20.1, 21]);
      d.B(15, 6, 39.5, 11, 8, 3, "d");
      d.B(37, 4, 0, 48, 12, 1.2, "s");
      const t = [];
      for (let x = 42; x <= 82; x += 4) t.push([[x, 4, 1.2], [x, x % 8 === 2 ? 10 : 7.5, 1.2]]);
      d.line(t, "ln e2");
      d.B(85, 3, -4, 1.6, 14, 5.2, "g");
    },
    square(d) {
      d.X([[0, 0], [30, 0], [30, 12], [0, 12]], "xz", 0, 1.4, "s");
      d.X([[14, 14], [30, 14], [30, -16], [22, -16], [14, -8]], "xz", -5, 6.5, "d");
      d.F(on(rr(17, 3, 27, 8.5, 2.5), "xz", 6.55), "p");
      d.F(on(circle(23.5, 5.7, 1.4, 10), "xz", 6.6), "g");
      d.Cy("y", [22, 6.5, -7], 2.8, 1.8, "g");
      d.X([[30, 0], [92, 0], [92, 12], [30, 12]], "xz", 0, 1.4, "s");
      const t = [];
      for (let x = 34; x <= 90; x += 3) t.push([[x, 1.4, 12], [x, 1.4, (x - 34) % 12 === 0 ? 8 : 10]]);
      d.line(t, "ln e2");
    },
    mgauge(d) {
      d.line([[4, 8, 0], [4, 8, -4]]);
      d.B(0, 0, 0, 48, 9, 9, "w");
      d.B(48, -12, -12, 10, 33, 33, "w", { os: true });
      d.Cy("z", [53, 4.5, 21], 3.4, 4.5, "g");
      d.B(58, 0, 0, 30, 9, 9, "w");
      d.hatch([[88, 0, 0], [88, 9, 0], [88, 9, 9], [88, 0, 9]], 1.6);
    },
    mortgauge(d) {
      d.line([[3, 8, 0], [3, 8, -4]]);
      d.line([[7, 8, 0], [7, 8, -4]]);
      d.B(0, 0, 0, 48, 9, 9, "w");
      d.B(0, 2.5, 9, 48, 4, 0.6, "g");
      d.B(48, -12, -12, 10, 33, 33, "w", { os: true });
      d.F([[58.05, -12, -12], [58.05, -12, 21], [58.05, -9, 21], [58.05, -9, -12]], "g");
      d.Cy("z", [53, 4.5, 21], 3.4, 4.5, "g");
      d.B(58, 0, 0, 28, 9, 9, "w");
      d.B(58, 2.5, 9, 28, 4, 0.6, "g");
      d.Cy("x", [86, 4.5, 4.5], 6, 2.6, "g", { n: 20 });
      d.cl([82, 4.5, 4.5], [94, 4.5, 4.5]);
    },
    knife(d) {
      d.X([[33, 1.5], [33, 8.5], [14, 8.5], [2, 1.5]], "xy", 2.4, 4.2, "s");
      d.line([[14, 8.5, 4.2], [8, 4.2, 4.2]], "ln e2");
      d.X(rr(0, 0, 10, 7, 3), "yz", 32, 86, "w");
      d.F(on(circle(46, 5, 1.6, 10), "xy", 7.05), "g");
      d.F(on(circle(70, 5, 1.6, 10), "xy", 7.05), "g");
      d.cl([0, 5, 3.5], [92, 5, 3.5]);
    },
    labels(d) {
      d.B(14, -48, 0, 54, 40, 1.2, "p");
      const L = [];
      for (let r = 0; r < 4; r++)
        for (let c = 0; c < 2; c++) {
          const x = 18 + r * 12.5, y = -44 + c * 18;
          L.push([[x, y, 1.25], [x + 10, y, 1.25], [x + 10, y + 15, 1.25], [x, y + 15, 1.25], [x, y, 1.25]]);
        }
      d.line(L, "ln e2");
      d.F([[20, -40, 1.3], [26, -40, 1.3], [23, -34, 1.3]], "a");
      d.Cy("z", [2, 6, 0], 17, 13, "g", { n: 28 });
      d.F(on(circle(2, 6, 10, 24), "xy", 13.05), "p");
      d.hole(on(circle(2, 6, 8.2, 24), "xy", 13.1), [0, 0, -13], "p");
    },

    // ---------- Hand tools ----------
    crosscut(d) {
      d.X(sawBlade(0, 64, 18, 30), "xz", 0, 0.7, "s");
      d.X(rr(58, -3, 90, 37, 11), "xz", -3, 3.8, "w");
      d.hole(on(rr(66, 7, 80, 28, 6), "xz", 3.85), [0, -6.8, 0], "w");
      d.F(on(circle(62.5, 30, 1.6, 10), "xz", 3.9), "g");
      d.F(on(circle(62.5, 4, 1.6, 10), "xz", 3.9), "g");
    },
    tenonsaw(d) {
      d.X(sawBlade(0, 62, 24, 24, 2.4, 2), "xz", 0, 0.7, "s");
      d.B(-1, -1.5, 23, 64, 3.7, 5, "g");
      d.X(rr(58, -4, 88, 34, [6, 12, 6, 10]), "xz", -3, 3.8, "w");
      d.hole(on(rr(66, 3, 80, 26, 6), "xz", 3.85), [0, -6.8, 0], "w");
      d.F(on(circle(62.5, 25, 1.7, 10), "xz", 3.9), "g");
      d.F(on(circle(62.5, 6, 1.7, 10), "xz", 3.9), "g");
    },
    chisels(d) {
      chisel(d, -26, 20, 44);
      chisel(d, 0, 10, 44);
    },
    mortchisel(d) {
      d.X([[0, 0], [46, 0], [46, 14], [11, 14]], "xz", 0, 6, "s");
      d.line([[11, 6, 14], [0, 6, 0]], "ln");
      d.B(46, -1.5, -1.5, 6, 9, 17, "s");
      d.X(rr(-3.5, -4, 9.5, 18.5, 4), "yz", 52, 86, "w");
      d.X(rr(-4, -4.5, 10, 19, 4.5), "yz", 86, 89.5, "s");
      d.cl([-4, 3, 7], [96, 3, 7]);
    },
    mallet(d) {
      d.fit = [70, 52];
      d.B(19, 7, 0, 8, 8, 30, "w");
      d.X([[2, 30], [44, 30], [46, 56], [0, 56]], "xz", 0, 22, "w");
      d.hatch([[44, 22, 30], [44, 0, 30], [46, 0, 56], [46, 22, 56]], 1.6);
      d.line([[19, 7, 56.05], [27, 7, 56.05], [27, 15, 56.05], [19, 15, 56.05], [19, 7, 56.05]], "ln e2");
      d.line([[19, 11, 56.05], [27, 11, 56.05]], "ln e2");
    },
    blockplane(d) {
      d.X([[0, 0], [64, 0], [64, 5], [56, 11], [8, 11], [0, 5]], "xz", 0, 18, "d");
      d.hatch([[0, 18, 0], [64, 18, 0], [64, 18, 5], [0, 18, 5]], 2.4);
      d.X([[14, 11], [50, 11], [53, 15], [49, 19], [18, 14]], "xz", 2.5, 15.5, "s");
      d.Cy("x", [53, 9, 15], 3.4, 3, "g", { n: 16 });
      d.lathe("z", [7, 9, 8], [[0, 3.6], [3, 3.8], [5, 2.4]], "g", { n: 16 });
    },
    jackplane(d) {
      benchPlane(d, 92);
    },
    jointerplane(d) {
      benchPlane(d, 134);
    },
    shoulderplane(d) {
      // Narrow, tall body with a thumb scoop at the front; the iron runs full width,
      // flush with both sides, so it shows through the escapement on the side.
      const W = 9;
      d.X([[0, 0], [62, 0], [62, 14], [59, 19.5], [54, 22], [46, 22], [40, 15], [30, 15], [27, 19], ...arc(17, 22, 9.5, -15, -165, 8), [4, 17], [1, 12], [0, 6]], "xz", 0, W, "d");
      d.hatch([[48, W, 0], [62, W, 0], [62, W, 13], [48, W, 13]], 2.2);
      d.F(on([[23.5, 0.4], [33.5, 0.4], [45, 15.2], [37, 15.2]], "xz", W + 0.05), "h", { f: "f1" });
      d.X([[27, 0], [31.5, 0], [48, 25], [43.5, 26]], "xz", 0, W, "s");
      d.X([[36, 15], [46, 15], [52, 22.5], [47.5, 27], [40, 21]], "xz", 1.5, W - 1.5, "g");
      d.lathe("z", [48, W / 2, 23.5], [[0, 3.4], [3, 3.6], [5, 2.4]], "g", { n: 16 });
      d.cl([48, W / 2, 19], [48, W / 2, 33]);
    },
    routerplane(d) {
      d.X([[20.6, 0], [23.4, 0], [23.4, -6], [29, -6], [29, -8.5], [20.6, -8.5]], "yz", 34.6, 37.4, "s");
      d.X(rr(0, 0, 72, 26, 6), "xy", 0, 5, "d");
      d.lathe("z", [11, 13, 5], [[0, 5.4], [5, 6.6], [9, 6.2], [11, 4.2], [12, 1.6]], "w");
      d.Cy("z", [36, 22, 5], 2.4, 22, "s");
      d.Cy("z", [36, 22, 16], 4.6, 3, "g", { n: 16 });
      d.lathe("z", [61, 13, 5], [[0, 5.4], [5, 6.6], [9, 6.2], [11, 4.2], [12, 1.6]], "w");
      d.cl([36, 22, -12], [36, 22, 31]);
    },
    ploughplane(d) {
      d.B(0, -32, -6, 54, 5, 22, "w");
      d.Cy("y", [12, -27, 10], 2, 27, "s", { n: 12 });
      d.Cy("y", [42, -27, 10], 2, 27, "s", { n: 12 });
      d.Cy("y", [12, -25, 10], 3.4, 2.4, "g", { n: 12 });
      d.Cy("y", [42, -25, 10], 3.4, 2.4, "g", { n: 12 });
      d.B(-2, 3.5, -5, 58, 2.5, 5, "s");
      d.X([[20, 22], [26, 22], [36, 36], [31, 38]], "xz", 3, 7, "s");
      d.B(0, 0, 0, 54, 10, 24, "w");
      d.X([[38, 24], [50, 24], [57, 38], [55, 43], [49, 42], [46, 36]], "xz", 3, 7, "w");
      d.B(4, 3, 24, 7, 4, 4, "g");
    },
    scraper(d) {
      d.B(-12, -26, -6, 78, 50, 6, "w", { os: true });
      // A thin steel card, flexed by the thumbs and leaning into the cut.
      const lean = (x, y, z) => [x, y - z * 0.32, z * 0.95];
      const yb = (x) => 4 - 3 * Math.sin((Math.PI * (x - 4)) / 46);
      const xs = Array.from({ length: 11 }, (_, i) => 4 + i * 4.6);
      const ringAt = (z) => [...xs.map((x) => lean(x, yb(x) + 0.8, z)), ...xs.slice().reverse().map((x) => lean(x, yb(x), z))];
      d.loft(ringAt(0), ringAt(34), "s");
      const pt = (x, z) => lean(x, yb(x) + 0.85, z);
      d.line([[pt(12, 27), pt(18, 19)], [pt(16, 29), pt(20, 24)]], "x e2");
      // Curled shaving rising off the board in front of the edge.
      const y0 = 9, steps = [];
      for (let t = 0; t <= 1.55 * Math.PI + 1e-6; t += 0.16) steps.push(t);
      const at = (t, x) => {
        const r = 7 - (1.6 * t) / Math.PI;
        return [x, y0 + r * Math.sin(t), 7 - r * Math.cos(t) + 0.1];
      };
      const xa = (t) => 10 + t * 0.5, xb = (t) => 34 + t * 0.2;
      const quads = [];
      for (let i = 0; i < steps.length - 1; i++) {
        const t0 = steps[i], t1 = steps[i + 1];
        const q = [at(t0, xa(t0)), at(t0, xb(t0)), at(t1, xb(t1)), at(t1, xa(t1))];
        let n = newell(q);
        if (dot(n, VIEW) < 0) n = mul(n, -1);
        quads.push({ q, n, k: dot(centroid(q), VIEW) });
      }
      const flat = [[xa(0), y0 - 5, 0.1], [xb(0), y0 - 5, 0.1], at(0, xb(0)), at(0, xa(0))];
      d.poly(flat, "g", "f1", "ns");
      quads.sort((a, b) => a.k - b.k).forEach(({ q, n }) => d.poly(q, "g", face(n), "ns"));
      d.line([[[xa(0), y0 - 5, 0.1], ...steps.map((t) => at(t, xa(t)))], [[xb(0), y0 - 5, 0.1], ...steps.map((t) => at(t, xb(t)))]], "ln");
      const tEnd = steps[steps.length - 1];
      d.line([at(tEnd, xa(tEnd)), at(tEnd, xb(tEnd))], "ln");
    },
    rasp(d) {
      const sec = (x, w, h) => {
        const pts = [[x, -w / 2, 3]];
        for (let i = 0; i <= 8; i++) pts.push([x, (w / 2) * Math.cos(rad(i * 22.5)), 3 + h * Math.sin(rad(i * 22.5))]);
        return pts.slice(0, -1).concat([[x, -w / 2, 3]]).slice(1);
      };
      d.loft(sec(0, 5, 1.6), sec(56, 13, 4.2), "s");
      const m = [];
      for (let x = 5; x < 54; x += 4.2) {
        const k = x / 56, w = 5 + 8 * k, h = 1.6 + 2.6 * k;
        for (const a of [55, 90, 125]) {
          const y = (w / 2) * Math.cos(rad(a)), z = 3 + h * Math.sin(rad(a)) + 0.1;
          m.push([[x, y, z], [x + 1.6, y, z]]);
        }
      }
      d.line(m, "ln e2");
      d.Cy("x", [56, 0, 5], 4.4, 4, "g", { n: 16 });
      d.lathe("x", [60, 0, 5], [[0, 4.6], [8, 5.8], [24, 6], [31, 4.8], [33, 2.6]], "w");
      d.cl([-4, 0, 5], [96, 0, 5]);
    },

    // ---------- Power tools ----------
    drill(d) {
      d.Cy("x", [-14, 9, 40], 1.2, 17, "s", { n: 8 });
      d.lathe("x", [2, 9, 40], [[0, 4], [4, 5.8], [12, 6.6], [13, 7.6], [16, 7.6]], "s");
      d.X([[44, 32], [60, 32], [64, 10], [47, 10]], "xz", 2, 16, "b");
      d.X(rr(14, 30, 68, 51, 9), "xz", 0, 18, "b");
      d.F(on([[52, 32], [60, 32], [63.5, 12], [57, 12]], "xz", 16.05), "d");
      const v = [];
      for (let x = 54; x <= 62; x += 2.6) v.push([[x, 18.05, 37], [x, 18.05, 45]]);
      d.line(v, "ln e2");
      d.X([[39, 24], [44, 24], [44, 31], [40, 31]], "xz", 6, 12, "d");
      d.B(36, -2, 0, 34, 22, 10, "d", { os: true });
      d.cl([-20, 9, 40], [30, 9, 40]);
    },
    router(d) {
      d.Cy("z", [0, 0, -20], 2, 20, "s", { n: 12 });
      d.Cy("z", [0, 0, 0], 24, 5, "d", { n: 32 });
      d.Cy("z", [-9, 16, 5], 3.2, 2.4, "g", { n: 12 });
      d.lathe("x", [-33, 0, 28], [[0, 3.4], [2, 5], [12, 5], [14, 3.6]], "w");
      d.Cy("z", [-17, 0, 5], 2.4, 27, "s", { n: 12 });
      d.B(-19, -5, 24, 38, 10, 7, "d");
      d.Cy("z", [0, 0, 6], 12, 38, "b", { n: 28 });
      d.Cy("z", [0, 0, 44], 10, 4, "d", { n: 28 });
      d.Cy("z", [17, 0, 5], 2.4, 27, "s", { n: 12 });
      d.lathe("x", [19, 0, 28], [[0, 5], [10, 5], [12, 4.4], [13.4, 2.6]], "w");
      d.cl([0, 0, -26], [0, 0, 56]);
    },
    ros(d) {
      d.Cy("z", [0, 0, 0], 22, 5, "d", { n: 32 });
      d.lathe("z", [0, 0, 5], [[0, 20], [6, 19.6], [13, 17.4], [18, 13.6]], "b", { n: 32 });
      d.lathe("z", [0, 0, 23], [[0, 12.6], [3, 12], [6, 8.6], [7.4, 3]], "d", { n: 28 });
      d.Cy("x", [14, 0, 13], 5, 18, "d", { n: 20 });
      d.hole(d.ring([32.05, 0, 13], [1, 0, 0], 3.4, 20), [-6, 0, 0], "d");
      d.cl([0, 0, -6], [0, 0, 38]);
    },
    circsaw(d) {
      d.Cy("y", [38, 12, 21], 12, 14, "d", { n: 24 });
      d.X(arc(42, 4, 24, 180, 360, 16), "xz", 29.5, 30.5, "s");
      d.B(0, 0, 0, 84, 36, 2, "s", { os: true });
      d.X(arc(42, 2, 27, 0, 180, 16), "xz", 25, 34, "b");
      d.lathe("z", [22, 29.5, 15], [[0, 4], [6, 4.6], [8.4, 3]], "d", { n: 16 });
      d.X(rr(58, 10, 88, 38, [3, 12, 12, 3]), "xz", 27, 32, "d");
      d.hole(on(rr(64, 16, 80, 31, 5), "xz", 32.05), [0, -5, 0], "d");
      d.cl([42, 30.5, -24], [42, 30.5, 34]);
      d.cl([14, 30.5, 4], [70, 30.5, 4]);
    },
    jigsaw(d) {
      const blade = [[9, -4], [9, -25], [6, -25]];
      for (let z = -25; z < -5; z += 3) blade.push([4.2, z + 1.5], [6, z + 3]);
      blade.push([5, -4]);
      d.X(blade, "xz", 7.6, 8.4, "s");
      d.B(-4, -4, -4, 72, 24, 4, "s");
      d.X(rr(0, 0, 64, 34, [4, 12, 10, 4]), "xz", 0, 16, "b");
      d.hole(on(rr(16, 16, 52, 26, 5), "xz", 16.05), [0, -16, 0], "b");
      d.F(on(rr(16, 28, 54, 32.5, 2), "xz", 16.05), "d");
      const v = [];
      for (let x = 46; x <= 58; x += 3) v.push([[x, 16.05, 5], [x, 16.05, 11]]);
      d.line(v, "ln e2");
    },
    domino(d) {
      d.B(2, -4, 0, 12, 32, 34, "d", { os: true });
      d.F(on(circle(8, 22, 3.4, 16), "xz", 28.05), "g");
      d.line([[[4, 28.05, 30], [12, 28.05, 30]], [[4, 28.05, 27], [9, 28.05, 27]], [[4, 28.05, 24], [12, 28.05, 24]]], "ln e2");
      d.X(rr(14, 4, 66, 28, 8), "xz", 0, 24, "b");
      d.B(26, 8, 28, 4, 8, 7, "d");
      d.B(26, 8, 34, 28, 8, 4, "d");
      d.B(50, 8, 28, 4, 8, 7, "d");
      d.Cy("x", [66, 12, 15], 5, 9, "d", { n: 20 });
      d.hole(d.ring([75.05, 12, 15], [1, 0, 0], 3.4, 20), [-6, 0, 0], "d");
    },

    // ---------- Stationary machines ----------
    tablesaw(d) {
      d.B(0, 0, 0, 54, 40, 30, "p", { h: true, hd: true, os: true });
      d.Cy("x", [54, 20, 14], 6, 1.8, "d", { n: 20 });
      d.line([[56, 20, 14], [56, 26, 9]], "ln");
      d.B(54, 4, 20, 1.5, 6, 5, "a");
      d.B(-8, -8, 30, 72, 56, 3, "s");
      d.line([[[-8, 6, 33.05], [64, 6, 33.05]], [[-8, 9, 33.05], [64, 9, 33.05]]], "ln e2");
      d.F(on(rr(12, 15, 40, 23, 2), "xy", 33.05), "d");
      const teeth = [];
      for (let i = 0; i <= 18; i++) {
        const a = rad(180 * (i / 18)), r = i % 2 ? 12 : 13.6;
        teeth.push([26 + r * Math.cos(a), 33 + r * Math.sin(a)]);
      }
      d.X(teeth, "xz", 18.4, 19.6, "s");
      d.cl([26, 19, 26], [26, 19, 50]);
      d.B(66, -10, 24, 3, 64, 6, "s");
      d.B(-8, 32, 33, 70, 5, 6, "b");
      d.B(62, 31, 28, 8, 7, 11, "b");
    },
    mitersaw(d) {
      d.B(0, 0, 0, 64, 40, 5, "s", { os: true, hd: true });
      d.Cy("z", [32, 24, 5], 16, 2, "s", { n: 32 });
      d.B(29, 36, 5, 6, 12, 2, "s");
      d.B(28, 0, 5, 8, 6, 30, "d");
      d.B(2, 8, 7, 26, 3, 10, "d");
      d.B(36, 8, 7, 26, 3, 10, "d");
      d.B(29.5, 2, 30, 5, 14, 5, "d");
      const low = [];
      for (let i = 0; i <= 20; i++) {
        const a = rad(180 + 9 * i), r = i % 2 ? 13.3 : 14.6;
        low.push([24 + r * Math.cos(a), 24 + r * Math.sin(a)]);
      }
      d.X(low, "yz", 31.4, 32.6, "s");
      d.X(arc(24, 24, 16.5, 0, 180, 14), "yz", 28.5, 35.5, "b");
      d.X(rr(31, 35, 46, 49, [2, 6, 6, 2]), "yz", 30, 34, "d");
      d.hole(on(rr(34.5, 38.5, 42.5, 45.5, 2.5), "yz", 34.05), [-4, 0, 0], "d");
      d.cl([20, 24, 24], [44, 24, 24]);
    },
    jointer(d) {
      d.B(8, 4, 0, 52, 22, 26, "p", { h: true, hd: true, os: true });
      d.Cy("x", [60, 15, 13], 5.4, 1.8, "d", { n: 20 });
      d.B(-16, -1, 30, 92, 3, 15, "d");
      d.B(-14, 2, 26, 50, 30, 4, "s");
      d.F([[36, 2, 27], [42, 2, 27], [42, 32, 27], [36, 32, 27]], "d");
      d.Cy("y", [39, 2, 26], 2.6, 30, "s", { n: 12 });
      d.B(42, 2, 26, 46, 30, 4, "s");
      d.X(rr(28, 8, 50, 31, [3, 9, 9, 3]), "xy", 30.3, 32, "b");
    },
    planer(d) {
      d.B(-24, 6, 12, 24, 38, 2, "s");
      d.B(0, 0, 0, 40, 50, 30, "p", { h: true, hd: true });
      d.F([[40.05, 7, 15], [40.05, 43, 15], [40.05, 43, 20], [40.05, 7, 20]], "d");
      const s = [];
      for (let z = 4; z <= 26; z += 2.6) s.push([[40.05, 2, z], [40.05, z % 5.2 < 1 ? 6 : 4.5, z]]);
      d.line(s, "x e2");
      d.B(40, 6, 12, 24, 38, 2, "s");
      d.B(-2, -2, 30, 44, 54, 8, "b", { os: true });
      d.Cy("z", [20, 25, 38], 2.6, 5, "s", { n: 12 });
      d.B(18.6, 8, 43, 2.8, 18.6, 2.6, "s");
      d.Cy("z", [20, 9, 45.6], 2.2, 6, "d", { n: 12 });
    },
    bandsaw(d) {
      d.fit = [80, 54];
      d.B(-2, 2, 0, 46, 24, 10, "d", { hd: true });
      d.X([[2, 10], [40, 10], [40, 38], [16, 38], [16, 58], [40, 58], [40, 86], [30, 96], [12, 96], [2, 86]], "xz", 6, 22, "p");
      const c = (cx, cz, r) => [...on(circle(cx, cz, r, 28), "xz", 22.05), on([[cx + r, cz]], "xz", 22.05)[0]];
      d.line([c(26, 24, 11.5), c(25, 77, 14)], "ln e2");
      d.Cy("z", [22, 14, 96], 2.6, 4, "d", { n: 12 });
      d.B(32.5, 12, 52, 3, 4, 6, "d");
      d.B(16, -4, 44, 38, 34, 2.5, "s");
      d.B(33.5, 13.5, 46.5, 1, 1, 5.5, "s");
      d.cl([34, 14, 40], [34, 14, 62]);
    },
    drillpress(d) {
      d.fit = [80, 54];
      d.B(-16, -16, 0, 44, 32, 4, "d", { hd: true });
      d.Cy("z", [0, 0, 4], 3, 58, "s", { n: 16 });
      d.B(0, -3, 30, 16, 6, 4, "s");
      d.Cy("z", [18, 0, 30], 13, 3, "s", { n: 28 });
      d.Cy("z", [20, 0, 37], 1, 10, "s", { n: 8 });
      d.lathe("z", [20, 0, 46], [[0, 2.4], [6, 4], [9, 4.4]], "s", { n: 16 });
      d.Cy("z", [20, 0, 55], 3, 5, "s", { n: 16 });
      d.Cy("z", [-15, 0, 60], 8, 18, "d", { n: 20 });
      d.B(-8, -12, 60, 36, 24, 20, "b", { os: true });
      d.Cy("y", [14, 12, 68], 2.6, 3, "s", { n: 12 });
      const spokes = [], knobs = [-90, 30, 150];
      for (const a of knobs) spokes.push([[14, 15, 68], [14 + 13 * Math.cos(rad(a)), 15, 68 + 13 * Math.sin(rad(a))]]);
      d.line(spokes, "ln");
      for (const a of knobs) d.F(on(circle(14 + 13 * Math.cos(rad(a)), 68 + 13 * Math.sin(rad(a)), 2, 10), "xz", 15.05), "d");
      d.B(28, -8, 66, 0.8, 5, 5, "a");
      d.cl([20, 0, 30], [20, 0, 64]);
    },
    mortiser(d) {
      d.fit = [80, 54];
      d.B(-14, -16, 0, 48, 32, 6, "d", { hd: true });
      d.B(-2, -14, 6, 36, 3, 12, "s");
      d.Cy("z", [-8, 0, 6], 3.4, 50, "s", { n: 16 });
      d.B(2, -11, 6, 32, 18, 8, "w");
      d.F(on(rr(12, -3, 24, 1, 1), "xy", 14.05), "h", { f: "f1" });
      d.F(on(rr(5, -3, 9, 1, 1), "xy", 14.05), "h", { f: "f1" });
      d.B(10, -2, 20, 4, 4, 18, "s");
      d.B(-14, -10, 38, 30, 20, 16, "b", { os: true });
      d.Cy("z", [0, 0, 54], 8, 15, "d", { n: 20 });
      d.Cy("y", [4, 10, 46], 2.6, 3, "s", { n: 12 });
      d.rod([4, 12, 46], [4, 38, 62], 1.4, "s", { n: 10 });
      d.rod([4, 36, 61], [4, 46, 67], 2.8, "w", { n: 12 });
      d.cl([12, 0, 10], [12, 0, 42]);
    },
    routertable(d) {
      d.B(3, 3, 0, 4, 4, 32, "d");
      d.B(3, 41, 0, 4, 4, 32, "d");
      d.Cy("z", [36, 24, 10], 9, 22, "b", { n: 24 });
      d.B(65, 3, 0, 4, 4, 32, "d");
      d.B(65, 41, 0, 4, 4, 32, "d");
      d.B(0, 0, 32, 72, 48, 3.5, "p", { os: true });
      d.F(on(rr(22, 12, 50, 36, 2), "xy", 35.55), "s");
      d.F(on(circle(36, 24, 3.6, 16), "xy", 35.6), "h", { f: "f1" });
      d.line([[0, 42, 35.55], [72, 42, 35.55]], "ln e2");
      d.B(4, 4, 35.5, 28, 4, 11, "s");
      d.B(40, 4, 35.5, 28, 4, 11, "s");
      d.B(32, 1, 35.5, 8, 3, 9, "d");
      d.Cy("z", [36, 24, 35.5], 2.4, 5, "s", { n: 12 });
      d.cl([36, 24, 6], [36, 24, 46]);
    },

    // ---------- Bits, blades & jigs ----------
    forstner(d) {
      forstnerBit(d, 26, -24, 8, 6, 2.2, 24);
      forstnerBit(d, 0, 0, 13, 8, 3, 28);
    },
    straightbit(d) {
      bit(d, 26, -24, 5, 16, 3, 20);
      bit(d, 0, 0, 7.5, 22, 4, 24);
    },
    roundover(d) {
      d.Cy("z", [0, 0, 0], 4.4, 29, "s", { n: 16 });
      d.Cy("z", [0, 0, 28], 16, 4, "b", { n: 28 });
      const cove = [];
      for (let i = 0; i <= 6; i++) {
        const a = rad(90 - i * 15);
        cove.push([41 - 9 * Math.sin(a), 16 - 9 * Math.cos(a)]);
      }
      d.lathe("z", [0, 0, 0], cove, "b", { n: 28 });
      const edge = cove.map(([z, r]) => [r + 0.7, 0, z]);
      d.F([[0, 0, 28.5], [16.7, 0, 28.5], ...edge, [0, 0, 41]], "s");
      d.Cy("z", [0, 0, 41.2], 5.4, 4, "d", { n: 20 });
      d.cl([0, 0, -5], [0, 0, 51]);
    },
    chamferbit(d) {
      d.Cy("z", [0, 0, 0], 4.4, 29, "s", { n: 16 });
      d.Cy("z", [0, 0, 28], 17, 3, "b", { n: 28 });
      d.Cy("z", [0, 0, 31], 17, 11, "b", { n: 28, r2: 6 });
      d.F([[0, 0, 28.5], [17.7, 0, 28.5], [17.7, 0, 31.4], [6.7, 0, 42], [0, 0, 42]], "s");
      d.Cy("z", [0, 0, 42.2], 5.4, 4, "d", { n: 20 });
      d.cl([0, 0, -5], [0, 0, 52]);
    },
    edgeguide(d) {
      d.F(on(circle(27, -40, 22, 32), "xy", 10), "n", { cls: "x hd" });
      d.Cy("y", [10, -34, 12], 1.8, 50, "s", { n: 12 });
      d.Cy("y", [44, -34, 12], 1.8, 50, "s", { n: 12 });
      d.B(0, 14, 0, 56, 5, 22, "d", { os: true });
      d.B(0, 19, 2, 56, 3, 18, "w");
      d.hatch([[56, 19, 2], [56, 22, 2], [56, 22, 20], [56, 19, 20]], 1.4);
      d.Cy("z", [28, 16.5, 22], 1, 3, "s", { n: 8 });
      d.Cy("z", [28, 16.5, 25], 4.4, 3, "b", { n: 20 });
    },
    tenonjig(d) {
      d.B(0, 0, 0, 56, 30, 4, "d", { hd: true });
      d.B(4, 8, 4, 48, 4, 40, "s", { os: true });
      d.B(20, 12, 4, 12, 9, 54, "w");
      d.B(23, 14, 58, 6, 5, 6, "w");
      d.B(14, 21, 34, 24, 3, 5, "s");
      d.Cy("y", [26, 24, 36.5], 1.3, 8, "s", { n: 10 });
      d.Cy("y", [26, 31, 36.5], 4.2, 3.4, "b", { n: 20 });
      d.Cy("x", [52, 22, 14], 4.4, 3, "b", { n: 20 });
    },
    dado(d) {
      // Exploded along the arbor: outer blade, a separate stack of chippers, outer blade.
      const blade = (x) => d.X(toothed(0, 0, 24, 30, 2.4), "yz", x, x + 1.6, "s");
      const chipper = (x, rot) => {
        const pts = [];
        for (let a = 0; a < 360; a += 4) {
          const w = (((a - rot) % 180) + 180) % 180;
          const r = w < 26 ? 20.5 - w * 0.06 : 12.5;
          pts.push([r * Math.cos(rad(a)), r * Math.sin(rad(a))]);
        }
        d.X(pts, "yz", x, x + 1.4, "b");
      };
      const arbor = (x0, x1) => d.Cy("x", [x0, 0, 0], 3, x1 - x0, "s", { n: 16 });
      arbor(-10, 0);
      blade(0);
      arbor(1.6, 18);
      chipper(18, 10);
      arbor(19.4, 21.5);
      chipper(21.5, 70);
      arbor(22.9, 25);
      chipper(25, 130);
      arbor(26.4, 44);
      blade(44);
      d.Cy("x", [45.6, 0, 0], 5.6, 1.2, "g", { n: 20 });
      arbor(46.8, 52);
      d.cl([-16, 0, 0], [60, 0, 0]);
    },
    shootboard(d) {
      d.B(0, 0, 0, 86, 44, 4, "w", { os: true });
      d.X([[46, 6], [56, 6], [61, -6], [56, -8], [52, -2]], "xy", 8, 14, "w");
      d.Cy("y", [28, -3, 11], 3.6, 9, "w", { n: 16 });
      d.B(18, 6, 4, 46, 10, 14, "d");
      d.F([[30, 6, 18.05], [36, 6, 18.05], [38, 16, 18.05], [32, 16, 18.05]], "s");
      d.B(0, 16, 4, 86, 28, 5, "w");
      d.B(4, 16, 9, 8, 28, 8, "w");
      d.B(12, 22, 9, 40, 14, 4, "p");
      d.hatch([[52, 22, 9], [52, 36, 9], [52, 36, 13], [52, 22, 13]], 1.2);
    },

    // ---------- Clamping & assembly ----------
    barclamps(d) {
      d.B(0, 0, 0, 96, 3, 9, "s");
      d.X([[10, 12], [22, 12], [22, -14], [17, -20], [10, -20]], "xz", -3, 6, "b");
      d.B(22, -2, -18, 2, 7, 9, "d");
      d.Cy("x", [60, 1.5, -12], 1.4, 14, "s", { n: 10 });
      d.Cy("x", [57.5, 1.5, -12], 4, 2.5, "d", { n: 16 });
      d.X([[72, 12], [84, 12], [84, -20], [77, -20], [72, -14]], "xz", -3, 6, "b");
      d.lathe("x", [84, 1.5, -12], [[0, 2.6], [2, 3.8], [16, 4.2], [18, 2.6]], "w", { n: 16 });
    },
    fclamps(d) {
      d.fit = [76, 54];
      d.B(0, 0, 0, 5, 3, 62, "s");
      d.Cy("z", [28, 1.5, 50], 3.6, 3, "d", { n: 16 });
      d.X([[0, 53], [34, 53], [34, 62], [0, 62]], "xz", -1.5, 4.5, "d");
      d.Cy("z", [28, 1.5, 10], 1.3, 36, "s", { n: 10 });
      d.Cy("z", [28, 1.5, 46], 3.6, 2.4, "d", { n: 16 });
      d.lathe("z", [28, 1.5, -4], [[0, 2.6], [2, 4], [13, 4.4], [15, 2.6]], "w", { n: 16 });
      d.X([[-2, 18], [34, 18], [34, 26], [-2, 26]], "xz", -2, 5, "b");
      d.cl([28, 1.5, -8], [28, 1.5, 58]);
    },
    strap(d) {
      // Webbing on the floor between the ratchet buckle and a coiled roll of strap.
      d.B(0, 1, 0, 64, 14, 0.8, "b");
      const base = rr(-7, 3, 30, 15, 3);
      const lever = [[0.5, 5.8], [3.5, 12.2], [-28.5, 27.2], [-31.5, 20.8]];
      d.X(base, "xz", -2.5, -1.3, "s");
      d.X(lever, "xz", -2.5, -1.3, "s");
      d.Cy("y", [-30, -3, 24], 3.4, 22, "d", { n: 16 });
      d.Cy("y", [0, -1.3, 10], 6.2, 17.9, "b", { n: 20 });
      d.X(base, "xz", 16.6, 17.8, "s");
      d.X(lever, "xz", 16.6, 17.8, "s");
      d.X(toothed(0, 10, 7.8, 12, 2), "xz", 17.8, 19, "s");
      d.F(on(circle(0, 10, 2, 12), "xz", 19.05), "d");
      d.Cy("y", [62, 1, 14], 14, 14, "b", { n: 28 });
      const sp = [];
      for (let t = 0; t <= 6 * Math.PI; t += 0.3) {
        const r = 12.8 - (t / (6 * Math.PI)) * 8.4;
        sp.push([62 + r * Math.cos(t), 15.05, 14 + r * Math.sin(t)]);
      }
      d.line(sp, "ln e2");
      d.F(on(circle(62, 14, 3.2, 16), "xz", 15.1), "h", { f: "f1" });
    },
    glue(d) {
      d.fit = [70, 52];
      d.lathe("z", [0, 0, 0], [[0, 12], [34, 12], [38, 11], [42, 6], [44, 5]], "b", { n: 28 });
      const wrap = (z) => Array.from({ length: 11 }, (_, i) => rad(-25 + i * 14)).map((a) => [12.1 * Math.cos(a), 12.1 * Math.sin(a), z]);
      d.F([...wrap(9), ...wrap(27).reverse()], "p", { f: "f1" });
      const ln = (z, a1) => Array.from({ length: 7 }, (_, i) => rad(-5 + i * a1)).map((a) => [12.15 * Math.cos(a), 12.15 * Math.sin(a), z]);
      d.line([ln(20, 15), ln(15, 10)], "ln e2");
      d.lathe("z", [0, 0, 44], [[0, 5], [4, 4.6], [5, 3], [13, 1.2]], "d", { n: 20 });
      d.cl([0, 0, -4], [0, 0, 62]);
    },

    // ---------- Finishing ----------
    rags(d) {
      d.Cy("z", [44, -16, 0], 12, 28, "s", { n: 28 });
      const wrap = (z) => Array.from({ length: 11 }, (_, i) => rad(-30 + i * 15)).map((a) => [44 + 12.1 * Math.cos(a), -16 + 12.1 * Math.sin(a), z]);
      d.F([...wrap(7), ...wrap(21).reverse()], "b", { f: "f2" });
      d.Cy("z", [44, -16, 28], 12.6, 2.2, "d", { n: 28 });
      d.B(0, 0, 0, 42, 30, 5, "p", { os: true });
      d.B(3, 2, 5, 37, 26, 5, "p");
      d.F([[3, 28, 10.05], [20, 28, 10.05], [16, 20, 10.05]], "p");
      d.line([[[8, 28.05, 3], [14, 28.05, 2], [22, 28.05, 3], [30, 28.05, 2]], [[42.05, 6, 2.5], [42.05, 14, 2], [42.05, 22, 3]]], "x e2");
    },
    brushes(d) {
      const brush = (y0, w, x1) => {
        const yc = y0 + w / 2;
        const sect = (x, ww, hh, z0) => on(rr(yc - ww / 2, z0, yc + ww / 2, z0 + hh, 1), "yz", x);
        d.loft(sect(0, w - 2, 3, 3), sect(16, w, 6, 1.5), "d");
        d.line([[[2, yc - w / 4, 6], [15, yc - w / 4, 7.4]], [[2, yc + w / 4, 6], [15, yc + w / 4, 7.4]]], "ln e2");
        d.X(rr(y0 - 0.5, 1, y0 + w + 0.5, 8.2, 2), "yz", 16, 26, "s");
        d.rod([26, yc, 4.6], [x1, yc, 4.6], 2.8, "w", { n: 16, r2: 3.6 });
      };
      brush(-26, 22, 86);
      brush(0, 15, 78);
    },
  };

  // Shared look: ink stroke, silhouette and three tints per material, all from CSS
  // variables so the art follows the theme.
  const CSS = `
  :root { --art-ink:#3b342c; --art-wood:#f1dcb4; --art-steel:#e2e6ea; --art-body:#f0a088; --art-body-2:#d97a60; --art-dark:#a7afb8; --art-brass:#ecd08a; --art-paper:#fdfaf3; --art-accent:#d63b32; --art-hole:var(--bg); --art-shade:#5c4d3f; }
  :root[data-theme="dark"] { --art-ink:#e0d4bd; --art-wood:#6c5840; --art-steel:#555b62; --art-body:#b5634e; --art-body-2:#8f4b3a; --art-dark:#3b3f45; --art-brass:#8c7444; --art-paper:#3b3932; --art-accent:#ff7d61; --art-shade:#000; }
  svg.art { overflow: visible; }
  svg.art path { fill: none; stroke: var(--art-ink); stroke-width: .6; stroke-linejoin: round; stroke-linecap: round; }
  svg.art .sil { fill: var(--art-ink); stroke-width: 1.7; }
  svg.art .w { --t: var(--art-wood); } svg.art .s { --t: var(--art-steel); } svg.art .b { --t: var(--art-body); }
  svg.art .c { --t: var(--art-body-2); } svg.art .d { --t: var(--art-dark); } svg.art .g { --t: var(--art-brass); }
  svg.art .p { --t: var(--art-paper); } svg.art .a { --t: var(--art-accent); } svg.art .h { --t: var(--art-hole); }
  svg.art .f1 { --c: var(--t); }
  svg.art .f2 { --c: color-mix(in srgb, var(--t) 86%, var(--art-shade)); }
  svg.art .f3 { --c: color-mix(in srgb, var(--t) 70%, var(--art-shade)); }
  svg.art .fl { fill: var(--c); }
  svg.art .n.fl { fill: none; }
  svg.art .ns { stroke: var(--c); stroke-width: .35; }
  svg.art .e2 { stroke-width: .45; }
  svg.art .x { stroke-width: .45; opacity: .75; }
  svg.art .cl { stroke-dasharray: 6 1.6 1.2 1.6; opacity: .6; }
  svg.art .hd { stroke-dasharray: 2 1.5; }
  svg.art .ht { opacity: .4; }
  svg.art .os { opacity: .6; }
  svg.art.sm .x { display: none; }
  svg.art.sm path { stroke-width: 1.05; }
  svg.art.sm .sil { stroke-width: 3.2; }
  svg.art.sm .ns { stroke-width: .5; }
  svg.art.sm .e2 { stroke-width: .8; }`;
  const style = document.createElement("style");
  style.textContent = CSS;
  document.head.appendChild(style);

  const cache = {};
  function draw(id) {
    if (!(id in cache)) {
      const d = new Sketch();
      ART[id](d);
      cache[id] = d.render();
    }
    return cache[id];
  }

  window.toolArt = function (id, width = 96, cls = "") {
    const h = Math.round((width * 64) / 96);
    const size = width < 60 ? " sm" : "";
    return `<svg class="art${size} ${cls}" width="${width}" height="${h}" viewBox="0 0 96 64" aria-hidden="true">${ART[id] ? draw(id) : ""}</svg>`;
  };
  window.toolArt.has = (id) => !!ART[id];
})();
