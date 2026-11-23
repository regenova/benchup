// BenchUp UI prototypes: wood stock (supplier catalogue, boards in the shop, offcuts)
// and a stock-aware cutting plan. Stock is a user preference shared by every project,
// like the workshop's tools; which board each part comes from belongs to the project.
(function () {
  "use strict";

  const SPECIES = {
    oak: { name: "White oak", kind: "solid", color: "#cfa771" },
    walnut: { name: "Black walnut", kind: "solid", color: "#7b5338" },
    maple: { name: "Hard maple", kind: "solid", color: "#ead9b5" },
    pine: { name: "Pine", kind: "solid", color: "#e8cf9a" },
    plywood: { name: "Birch plywood", kind: "sheet", color: "#e6d3a8" },
  };

  // What the supplier sells: actual sizes in millimetres and the price per piece.
  const C = (id, species, L, W, T, price, name) => ({ id, species, L, W, T, price, name: name || `${SPECIES[species].name} ${SPECIES[species].kind === "sheet" ? "sheet" : "board"}` });
  const CATALOG = [
    C("oak-2400", "oak", 2400, 140, 27, 38),
    C("oak-1200", "oak", 1200, 140, 27, 20),
    C("oak-52", "oak", 1200, 120, 52, 46, "White oak board, 52 mm"),
    C("walnut-2000", "walnut", 2000, 200, 25, 68),
    C("walnut-1000", "walnut", 1000, 200, 25, 36),
    C("maple-2400", "maple", 2400, 160, 27, 44),
    C("pine-2x4", "pine", 2400, 89, 38, 6, "Pine 2 × 4"),
    C("ply-quarter", "plywood", 1250, 625, 18, 24, "Birch plywood, quarter sheet"),
    C("ply-full", "plywood", 2500, 1250, 18, 78, "Birch plywood, full sheet"),
  ];
  const BY_ID = Object.fromEntries(CATALOG.map((c) => [c.id, c]));

  const KEY = "benchup-proto-stock";
  const DAY = 864e5;
  const ago = (days) => new Date(Date.now() - days * DAY).toISOString();
  // Offcuts nobody has confirmed for this long are no longer counted on by cut lists.
  const STALE_DAYS = 180;
  const RECENT_DAYS = 14;
  const SEEN = { o1: 40, o2: 20, o3: 400, o4: 260 };
  function sample() {
    return {
      onHand: { "oak-2400": 1, "pine-2x4": 3 },
      boardSeen: {},
      offcuts: [
        { id: "o1", species: "walnut", L: 950, W: 190, T: 25, from: "Side table", seen: ago(SEEN.o1) },
        { id: "o2", species: "oak", L: 700, W: 140, T: 27, from: "Toy box for Leo", seen: ago(SEEN.o2) },
        { id: "o3", species: "plywood", L: 820, W: 410, T: 18, from: "Living room bookshelf", seen: ago(SEEN.o3) },
        { id: "o4", species: "maple", L: 380, W: 120, T: 27, from: "Toy box for Leo", seen: ago(SEEN.o4) },
      ],
      useStock: true,
      costing: "purchase",
    };
  }
  function loadStock() {
    try {
      const saved = JSON.parse(localStorage.getItem(KEY) || "null");
      const stock = saved ? Object.assign(sample(), saved) : sample();
      // Older saved data has no confirmation dates: fall back to the sample's, or today.
      stock.offcuts.forEach((o) => { if (!o.seen) o.seen = ago(SEEN[o.id] || 0); });
      stock.boardSeen = stock.boardSeen || {};
      return stock;
    } catch (_) {
      return sample();
    }
  }
  const resetStock = () => { try { localStorage.removeItem(KEY); } catch (_) {} };
  const daysSince = (iso) => Math.floor((Date.now() - new Date(iso).getTime()) / DAY);
  const isStale = (o) => daysSince(o.seen) > STALE_DAYS;
  const isRecent = (iso) => !!iso && daysSince(iso) <= RECENT_DAYS;
  function seenText(iso) {
    const d = daysSince(iso);
    if (d <= 1) return "Confirmed today";
    if (d < 14) return `Confirmed ${d} days ago`;
    if (d < 60) return `Confirmed ${Math.round(d / 7)} weeks ago`;
    return `Not confirmed for ${Math.round(d / 30)} months`;
  }

  /** "Still have it?" answers, shared by the cut list, Planning and the stock tab. */
  function confirmSource(stock, src) {
    if (src.kind === "offcut") { const o = stock.offcuts.find((x) => x.id === src.offcut.id); if (o) o.seen = new Date().toISOString(); }
    else if (src.kind === "stock") stock.boardSeen[src.catalog.id] = new Date().toISOString();
  }
  function goneSource(stock, src) {
    if (src.kind === "offcut") stock.offcuts = stock.offcuts.filter((x) => x.id !== src.offcut.id);
    else if (src.kind === "stock") { const id = src.catalog.id; stock.onHand[id] = Math.max(0, (stock.onHand[id] || 0) - 1); if (!stock.onHand[id]) delete stock.onHand[id]; }
  }
  const sourceConfirmed = (stock, src) => src.kind === "offcut" ? isRecent((stock.offcuts.find((x) => x.id === src.offcut.id) || {}).seen) : isRecent(stock.boardSeen[src.catalog.id]);

  /** A toast with an Undo button; restores a snapshot of the stock when clicked. */
  function undoToast(text, snapshot, onUndo) {
    let t = document.getElementById("stockToast");
    if (!t) {
      t = document.createElement("div");
      t.id = "stockToast";
      t.setAttribute("role", "status");
      t.style.cssText = "position:fixed;left:50%;bottom:28px;transform:translate(-50%,20px);opacity:0;background:var(--ink);color:var(--panel);padding:8px 8px 8px 16px;border-radius:10px;font-size:13px;transition:opacity .18s,transform .18s;z-index:40;display:flex;gap:12px;align-items:center;max-width:620px;pointer-events:none";
      document.body.appendChild(t);
    }
    t.innerHTML = `<span>${text}</span>${snapshot ? '<button style="all:unset;cursor:pointer;padding:5px 10px;border-radius:7px;font-weight:600;color:var(--panel);background:rgba(255,255,255,.14)">Undo</button>' : ""}`;
    t.style.opacity = "1"; t.style.transform = "translate(-50%,0)"; t.style.pointerEvents = "auto";
    const btn = t.querySelector("button");
    if (btn) btn.onclick = () => { saveStock(snapshot); hide(); onUndo && onUndo(); };
    function hide() { t.style.opacity = "0"; t.style.transform = "translate(-50%,20px)"; t.style.pointerEvents = "none"; }
    clearTimeout(undoToast.t);
    undoToast.t = setTimeout(hide, 5000);
  }
  const saveStock = (stock) => { try { localStorage.setItem(KEY, JSON.stringify(stock)); } catch (_) {} };

  // ---------- Cutting plan ----------
  const KERF = 5;
  const MIN_OFFCUT = { L: 300, W: 60 };

  /** Parts of the sample model as rectangles to cut, in millimetres. */
  function itemsFromParts(parts, size) {
    return parts.map((p) => {
      const s = size(p);
      return { id: p.id, name: p.name, species: p.material, l: Math.round(s.length * 10), w: Math.round(s.width * 10), t: Math.round(s.thickness * 10) };
    });
  }

  function tryPlace(src, it) {
    for (const shelf of src.shelves) {
      if (shelf.x + it.l <= src.L && it.w <= shelf.h) {
        src.items.push(Object.assign({}, it, { x: shelf.x, y: shelf.y }));
        shelf.x += it.l + KERF;
        return true;
      }
    }
    const used = src.shelves.reduce((a, s) => a + s.h + KERF, 0);
    if (it.l <= src.L && used + it.w <= src.W) {
      src.shelves.push({ x: it.l + KERF, y: used, h: it.w });
      src.items.push(Object.assign({}, it, { x: 0, y: used }));
      return true;
    }
    return false;
  }
  // A part is milled from stock at least as thick, but not wastefully thicker.
  const okT = (T, it) => T >= it.t && T - it.t <= 10;
  const fitsEmpty = (L, W, it) => it.l <= L && it.w <= W;
  const open = (kind, base) => Object.assign({ kind, shelves: [], items: [] }, base);

  /**
   * Packs the parts onto offcuts and boards you already have first (when useStock is on),
   * then onto boards to buy. Shelf packing with a 5 mm kerf; illustrative, not optimal.
   */
  function cutPlan(items, stock, useStock = stock.useStock) {
    // Parts wider than any board of their species are glued up from strips.
    const pieces = [];
    for (const it of items) {
      const widths = [...CATALOG.filter((c) => c.species === it.species && okT(c.T, it)).map((c) => c.W), ...stock.offcuts.filter((o) => o.species === it.species && okT(o.T, it)).map((o) => o.W)];
      const maxW = Math.max(...widths, 1);
      if (it.w > maxW) {
        const n = Math.ceil(it.w / maxW);
        for (let i = 0; i < n; i++) pieces.push(Object.assign({}, it, { w: Math.ceil(it.w / n) + 5, name: `${it.name}, strip ${i + 1} of ${n}`, strip: true }));
      } else pieces.push(it);
    }
    pieces.sort((a, b) => b.l * b.w - a.l * a.w);

    const sources = [];
    const offcutsLeft = useStock ? stock.offcuts.filter((o) => !isStale(o)) : [];
    const boardsLeft = useStock ? Object.assign({}, stock.onHand) : {};
    for (const it of pieces) {
      if (sources.some((s) => s.species === it.species && okT(s.T, it) && tryPlace(s, it))) continue;
      // An offcut that fits, smallest first so big ones stay free for big parts.
      const off = offcutsLeft.filter((o) => o.species === it.species && okT(o.T, it) && fitsEmpty(o.L, o.W, it)).sort((a, b) => a.L * a.W - b.L * b.W)[0];
      if (off) {
        offcutsLeft.splice(offcutsLeft.indexOf(off), 1);
        const s = open("offcut", { species: off.species, L: off.L, W: off.W, T: off.T, offcut: off, label: `Offcut from ${off.from}` });
        sources.push(s); tryPlace(s, it); continue;
      }
      const have = CATALOG.filter((c) => c.species === it.species && (boardsLeft[c.id] || 0) > 0 && okT(c.T, it) && fitsEmpty(c.L, c.W, it)).sort((a, b) => a.L * a.W - b.L * b.W)[0];
      if (have) {
        boardsLeft[have.id]--;
        const s = open("stock", { species: have.species, L: have.L, W: have.W, T: have.T, catalog: have, label: "In your shop" });
        sources.push(s); tryPlace(s, it); continue;
      }
      const buy = CATALOG.filter((c) => c.species === it.species && okT(c.T, it) && fitsEmpty(c.L, c.W, it)).sort((a, b) => b.L * b.W - a.L * a.W)[0];
      if (buy) {
        const s = open("buy", { species: buy.species, L: buy.L, W: buy.W, T: buy.T, catalog: buy, label: "To buy" });
        sources.push(s); tryPlace(s, it);
      }
    }
    // A bought board only needs to be as big as what is cut from it: pick the cheapest that holds it.
    for (const s of sources.filter((x) => x.kind === "buy")) {
      const usedL = Math.max(...s.shelves.map((sh) => sh.x - KERF));
      const usedW = s.shelves.reduce((a, sh) => a + sh.h + KERF, 0) - KERF;
      const best = CATALOG.filter((c) => c.species === s.species && c.T === s.T && c.L >= usedL && c.W >= usedW).sort((a, b) => a.price - b.price)[0];
      if (best && best.id !== s.catalog.id) Object.assign(s, { catalog: best, L: best.L, W: best.W, T: best.T });
    }
    // Pieces worth keeping afterwards.
    for (const s of sources) {
      s.leftovers = [];
      let y = 0;
      for (const sh of s.shelves) {
        const restL = s.L - sh.x;
        if (restL >= MIN_OFFCUT.L && sh.h >= MIN_OFFCUT.W) s.leftovers.push({ x: sh.x, y: sh.y, L: restL, W: sh.h });
        y = sh.y + sh.h + KERF;
      }
      if (s.W - y >= MIN_OFFCUT.W && s.L >= MIN_OFFCUT.L) s.leftovers.push({ x: 0, y, L: s.L, W: s.W - y });
    }
    const fromStock = sources.filter((s) => s.kind !== "buy");
    const toBuy = sources.filter((s) => s.kind === "buy");
    const buyGroups = {};
    for (const s of toBuy) {
      const g = (buyGroups[s.catalog.id] = buyGroups[s.catalog.id] || { catalog: s.catalog, qty: 0 });
      g.qty++;
    }
    const buyList = Object.values(buyGroups);
    // Unconfirmed offcuts that would fit something you are about to buy: worth asking about.
    const maybe = !useStock ? [] : stock.offcuts.filter((o) => isStale(o) && toBuy.some((s) => s.species === o.species && s.items.some((it) => okT(o.T, it) && fitsEmpty(o.L, o.W, it))));
    const buyCost = buyList.reduce((a, g) => a + g.qty * g.catalog.price, 0);
    const stockValue = fromStock.reduce((a, s) => a + (s.kind === "stock" ? s.catalog.price : 0), 0);
    const area = (s) => s.L * s.W;
    const usedArea = sources.reduce((a, s) => a + s.items.reduce((b, it) => b + it.l * it.w, 0), 0);
    const totalArea = sources.reduce((a, s) => a + area(s), 0) || 1;
    return { pieces, sources, fromStock, toBuy, buyList, maybe, buyCost, stockValue, waste: Math.round((1 - usedArea / totalArea) * 100), leftovers: sources.reduce((a, s) => a + s.leftovers.length, 0) };
  }

  // ---------- Drawing: a board in isometric, in the same sketch language as the tools ----------
  function shade(hex, f) {
    const n = parseInt(hex.slice(1), 16);
    const c = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((v) => Math.max(0, Math.min(255, Math.round(v * f))));
    return `rgb(${c[0]},${c[1]},${c[2]})`;
  }
  /** An isometric board at true proportions, fitted into width × height (thickness gets a 3 px minimum). */
  function boardArt(L, W, T, species, width = 120, height = 64, ref = null) {
    const c30 = Math.cos(Math.PI / 6), s30 = 0.5;
    // ref lets several boards share a scale, so lengths compare honestly.
    const RL = ref ? ref.L : L, RW = ref ? ref.W : W;
    const k = Math.min((width - 8) / ((RL + RW) * c30), (height - 8) / ((RL + RW) * s30 + 30));
    const l = L * k, w = W * k, t = Math.max(3, T * k);
    const P = (x, y, z) => [x * c30 - y * c30, (x + y) * s30 - z];
    const pts = { a: P(0, 0, t), b: P(l, 0, t), c: P(l, w, t), d: P(0, w, t), e: P(l, 0, 0), f: P(l, w, 0), g: P(0, w, 0) };
    const xs = Object.values(pts).map((p) => p[0]), ys = Object.values(pts).map((p) => p[1]);
    const ox = (width - (Math.max(...xs) - Math.min(...xs))) / 2 - Math.min(...xs);
    const oy = (height - (Math.max(...ys) - Math.min(...ys))) / 2 - Math.min(...ys);
    const q = (...ks) => ks.map((key) => `${(pts[key][0] + ox).toFixed(1)},${(pts[key][1] + oy).toFixed(1)}`).join(" ");
    const base = SPECIES[species].color;
    const grain = [0.3, 0.55, 0.8].map((f) => {
      const p1 = P(0, w * f, t), p2 = P(l, w * f, t);
      return `<line x1="${(p1[0] + ox).toFixed(1)}" y1="${(p1[1] + oy).toFixed(1)}" x2="${(p2[0] + ox).toFixed(1)}" y2="${(p2[1] + oy).toFixed(1)}" stroke="rgba(60,40,20,.18)" stroke-width=".6"/>`;
    }).join("");
    return `<svg class="board-art" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}" aria-hidden="true">
      <polygon points="${q("a", "b", "c", "d")}" fill="${shade(base, 1.05)}" stroke="var(--art-ink, #3a332b)" stroke-width=".8" stroke-linejoin="round"/>
      ${grain}
      <polygon points="${q("d", "c", "f", "g")}" fill="${shade(base, 0.86)}" stroke="var(--art-ink, #3a332b)" stroke-width=".8" stroke-linejoin="round"/>
      <polygon points="${q("b", "e", "f", "c")}" fill="${shade(base, 0.7)}" stroke="var(--art-ink, #3a332b)" stroke-width=".8" stroke-linejoin="round"/>
    </svg>`;
  }

  const fmtSize = (o) => `${o.L} × ${o.W} × ${o.T}`;

  window.BenchStock = {
    SPECIES, CATALOG, BY_ID, MIN_OFFCUT, STALE_DAYS,
    loadStock, saveStock, resetStock, itemsFromParts, cutPlan, boardArt, fmtSize,
    daysSince, isStale, isRecent, seenText, confirmSource, goneSource, sourceConfirmed, undoToast,
  };
})();
