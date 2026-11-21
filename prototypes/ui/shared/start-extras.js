// Prototype A first-run screens: project templates, view fitting, units and
// theme-aware viewport colours shared by a-start.html and a-empty.html.
(function () {
  "use strict";

  const P = (id, name, kind, material, a, b) => ({ id, name, kind, material, min: a, max: b });

  // All templates are in centimetres, like BenchModel.PARTS.
  const TEMPLATES = [
    {
      id: "hall-bench",
      name: "Hall bench",
      note: "Mortise & tenon frame",
      level: "Beginner",
      parts: BenchModel.PARTS,
    },
    {
      id: "bookshelf",
      name: "Bookshelf",
      note: "Dadoed fixed shelves",
      level: "Beginner",
      parts: [
        P("bs-l", "Side", "Sides", "oak", [0, 0, 0], [1.8, 28, 160]),
        P("bs-r", "Side", "Sides", "oak", [78.2, 0, 0], [80, 28, 160]),
        P("bs-back", "Back", "Panels", "plywood", [1.8, 0, 4], [78.2, 0.6, 158.2]),
        P("bs-bottom", "Bottom", "Shelves", "oak", [1.8, 1, 4], [78.2, 28, 5.8]),
        P("bs-plinth", "Plinth", "Rails", "oak", [1.8, 26, 0], [78.2, 28, 4]),
        P("bs-s1", "Shelf", "Shelves", "oak", [1.8, 1, 42], [78.2, 28, 43.8]),
        P("bs-s2", "Shelf", "Shelves", "oak", [1.8, 1, 80], [78.2, 28, 81.8]),
        P("bs-s3", "Shelf", "Shelves", "oak", [1.8, 1, 118], [78.2, 28, 119.8]),
        P("bs-top", "Top", "Shelves", "oak", [1.8, 0, 158.2], [78.2, 28, 160]),
      ],
    },
    {
      id: "side-table",
      name: "Side table",
      note: "Walnut top, lower shelf",
      level: "Intermediate",
      parts: [
        P("st-l1", "Leg", "Legs", "oak", [2, 2, 0], [6, 6, 52]),
        P("st-l2", "Leg", "Legs", "oak", [44, 2, 0], [48, 6, 52]),
        P("st-l3", "Leg", "Legs", "oak", [2, 44, 0], [6, 48, 52]),
        P("st-l4", "Leg", "Legs", "oak", [44, 44, 0], [48, 48, 52]),
        P("st-a1", "Apron", "Aprons", "oak", [6, 3, 44], [44, 5, 52]),
        P("st-a2", "Apron", "Aprons", "oak", [6, 45, 44], [44, 47, 52]),
        P("st-a3", "Apron", "Aprons", "oak", [3, 6, 44], [5, 44, 52]),
        P("st-a4", "Apron", "Aprons", "oak", [45, 6, 44], [47, 44, 52]),
        P("st-shelf", "Shelf", "Panels", "plywood", [6, 6, 12], [44, 44, 13.8]),
        P("st-top", "Top", "Panels", "walnut", [0, 0, 52], [50, 50, 55]),
      ],
    },
    {
      id: "workbench",
      name: "Workbench",
      note: "Heavy frame, solid top",
      level: "Intermediate",
      parts: [
        P("wb-l1", "Leg", "Legs", "oak", [5, 5, 0], [12, 12, 85]),
        P("wb-l2", "Leg", "Legs", "oak", [138, 5, 0], [145, 12, 85]),
        P("wb-l3", "Leg", "Legs", "oak", [5, 48, 0], [12, 55, 85]),
        P("wb-l4", "Leg", "Legs", "oak", [138, 48, 0], [145, 55, 85]),
        P("wb-r1", "Rail", "Rails", "oak", [12, 6, 75], [138, 9, 85]),
        P("wb-r2", "Rail", "Rails", "oak", [12, 51, 75], [138, 54, 85]),
        P("wb-s1", "Stretcher", "Rails", "oak", [12, 7, 15], [138, 10, 25]),
        P("wb-s2", "Stretcher", "Rails", "oak", [12, 50, 15], [138, 53, 25]),
        P("wb-s3", "Stretcher", "Rails", "oak", [7, 12, 15], [10, 48, 25]),
        P("wb-s4", "Stretcher", "Rails", "oak", [140, 12, 15], [143, 48, 25]),
        P("wb-shelf", "Shelf", "Panels", "plywood", [12, 10, 25], [138, 50, 27]),
        P("wb-top", "Top", "Panels", "oak", [0, 0, 85], [150, 60, 90]),
      ],
    },
    {
      id: "box",
      name: "Box with lid",
      note: "Finger-jointed corners",
      level: "Beginner",
      parts: [
        P("bx-back", "Side", "Sides", "oak", [0, 0, 0], [30, 1.2, 12]),
        P("bx-left", "End", "Sides", "oak", [0, 1.2, 0], [1.2, 18.8, 12]),
        P("bx-right", "End", "Sides", "oak", [28.8, 1.2, 0], [30, 18.8, 12]),
        P("bx-bottom", "Bottom", "Panels", "plywood", [1.2, 1.2, 0.6], [28.8, 18.8, 1.2]),
        P("bx-front", "Side", "Sides", "oak", [0, 18.8, 0], [30, 20, 12]),
        P("bx-lid", "Lid", "Panels", "walnut", [0, 0, 12.3], [30, 20, 14]),
      ],
    },
  ];

  function bounds(parts) {
    if (!parts.length) return null;
    const min = [Infinity, Infinity, Infinity];
    const max = [-Infinity, -Infinity, -Infinity];
    for (const p of parts)
      for (let k = 0; k < 3; k++) {
        min[k] = Math.min(min[k], p.min[k]);
        max[k] = Math.max(max[k], p.max[k]);
      }
    return { min, max };
  }

  /** Centre the view on its parts and size it to their bounding sphere. */
  function fit(view, fallback) {
    const b = bounds(view.parts);
    if (!b) {
      const f = fallback || { center: [30, 15, 0], radius: 60 };
      view.center = f.center.slice();
      view.radius = f.radius;
    } else {
      view.center = [0, 1, 2].map((k) => (b.min[k] + b.max[k]) / 2);
      const half = Math.hypot(b.max[0] - b.min[0], b.max[1] - b.min[1], b.max[2] - b.min[2]) / 2;
      view.radius = Math.max(half * 1.05, fallback ? fallback.minRadius || 0 : 0);
    }
    view.render();
  }

  // Units preference shared by the start and empty project screens.
  const UNITS_KEY = "benchup.prototype.units";
  const units = {
    get() {
      try {
        return localStorage.getItem(UNITS_KEY) || "mm";
      } catch {
        return "mm";
      }
    },
    set(value) {
      try {
        localStorage.setItem(UNITS_KEY, value);
      } catch {}
    },
    chosen() {
      try {
        return localStorage.getItem(UNITS_KEY) !== null;
      } catch {
        return false;
      }
    },
  };

  /** Format a length given in millimetres in the current unit. */
  function length(mmValue, unit = units.get()) {
    if (unit === "in") return `${(mmValue / 25.4).toFixed(1).replace(/\.0$/, "")}`;
    return `${Math.round(mmValue)}`;
  }

  function overall(parts, unit = units.get()) {
    const b = bounds(parts);
    if (!b) return "";
    const [w, d, h] = [0, 1, 2].map((k) => (b.max[k] - b.min[k]) * 10);
    return `${length(w, unit)} × ${length(d, unit)} × ${length(h, unit)} ${unit === "in" ? "in" : "mm"}`;
  }

  const currentTheme = () => (document.documentElement.dataset.theme === "dark" ? "dark" : "light");

  /** Viewport colours that follow the light or dark theme. */
  function viewportTheme(theme = currentTheme()) {
    const dark = theme === "dark";
    return {
      grid: dark
        ? { minor: "rgba(255,255,255,.05)", major: "rgba(255,255,255,.10)" }
        : { minor: "rgba(40,40,40,.06)", major: "rgba(40,40,40,.12)" },
      shadow: dark ? "rgba(0,0,0,.5)" : "rgba(60,50,40,.22)",
      dimBg: dark ? "rgba(30,32,36,.92)" : "rgba(255,255,255,.92)",
      dimText: dark ? "#ff8a78" : "#b8302a",
      dimColor: dark ? "#ff8a78" : "#d63b32",
      stroke: dark ? "rgba(0,0,0,.55)" : "rgba(60,45,30,0.45)",
    };
  }

  const ICONS = {
    folder: '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
    import: '<path d="M12 3v12M8 11l4 4 4-4"/><path d="M4 15v4a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-4"/>',
    clock: '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
    device: '<rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8M12 16v4"/>',
    sparkle: '<path d="M12 3l1.8 5.2L19 10l-5.2 1.8L12 17l-1.8-5.2L5 10l5.2-1.8z"/>',
    square: '<rect x="8" y="3" width="8" height="18" rx="1"/>',
    sheet: '<path d="M4 8l8-4 8 4-8 4z"/><path d="M4 8v2l8 4 8-4V8"/>',
    dowel: '<rect x="3" y="10" width="18" height="4" rx="2"/>',
  };
  const icon = (name, size = 18) =>
    ICONS[name]
      ? `<svg class="ic" width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">${ICONS[name]}</svg>`
      : window.icon(name, size);

  function toast(message) {
    let el = document.getElementById("start-toast");
    if (!el) {
      el = document.createElement("div");
      el.id = "start-toast";
      el.className = "start-toast";
      document.body.appendChild(el);
    }
    el.textContent = message;
    el.classList.add("show");
    clearTimeout(toast._t);
    toast._t = setTimeout(() => el.classList.remove("show"), 2600);
  }

  window.StartExtras = { TEMPLATES, bounds, fit, units, length, overall, viewportTheme, currentTheme, icon, toast };
})();
