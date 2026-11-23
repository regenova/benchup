// BenchUp UI prototypes: line illustrations of workshop tools (96 × 64, side views).
// One ink stroke and four flat tones (wood, steel, body, dark) so every tool reads as
// part of the same set in both themes. Use toolArt(id, width) to get an <svg>.
(function () {
  "use strict";

  // Saw teeth along a straight edge, as a zigzag path.
  function teeth(x1, x2, y, step = 3, depth = 3) {
    let d = `M${x1} ${y}`;
    for (let x = x1; x < x2; x += step) d += ` L${x + step / 2} ${y + depth} L${Math.min(x + step, x2)} ${y}`;
    return `<path d="${d}"/>`;
  }
  const ticks = (x1, x2, y, step, len) => {
    let d = "";
    for (let x = x1; x <= x2; x += step) d += `M${x} ${y}v${len}`;
    return `<path d="${d}"/>`;
  };

  const ART = {
    // ---------- Measuring & marking ----------
    tape: `<rect class="s" x="58" y="40" width="30" height="7"/>${ticks(63, 83, 40, 5, 3)}<path d="M88 37v13"/>
      <rect class="b" x="18" y="12" width="42" height="40" rx="10"/><circle class="s" cx="39" cy="32" r="9"/><circle cx="39" cy="32" r="2.5"/><rect class="d" x="24" y="46" width="14" height="6" rx="2"/>`,
    square: `<rect class="s" x="8" y="27" width="80" height="8"/>${ticks(46, 84, 27, 4, 3)}
      <path class="b" d="M22 16h18v34H30l-8-8z"/><rect class="p" x="27" y="22" width="8" height="5" rx="2.5"/><circle class="s" cx="31" cy="40" r="2.5"/>`,
    mgauge: `<rect class="w" x="10" y="29" width="76" height="8" rx="2"/><path d="M14 37v5"/>
      <rect class="w" x="50" y="14" width="13" height="38" rx="3"/><path d="M56.5 14v-3"/><circle class="s" cx="56.5" cy="8" r="3.5"/>`,
    mortgauge: `<rect class="w" x="10" y="29" width="76" height="8" rx="2"/><rect class="g" x="10" y="31" width="40" height="3"/><path d="M13 37v5M20 37v5"/>
      <rect class="w" x="52" y="14" width="13" height="38" rx="3"/><circle class="g" cx="78" cy="33" r="4"/><path d="M58.5 14v-3"/><circle class="s" cx="58.5" cy="8" r="3.5"/>`,
    knife: `<path class="s" d="M10 33l16-7h16v14H26z"/><path d="M18 33h10"/><rect class="w" x="40" y="26" width="46" height="14" rx="7"/><path d="M50 28v10M58 28v10"/>`,
    labels: `<rect class="p" x="44" y="14" width="40" height="38" rx="2"/><path d="M50 22h12M50 30h28M50 38h20M50 46h24"/><path class="a" d="M72 18l6 10h-12z"/>
      <circle class="w" cx="28" cy="38" r="15"/><circle class="h" cx="28" cy="38" r="7"/><path class="w" d="M28 23h16v8H32"/>`,

    // ---------- Hand tools ----------
    crosscut: `<path class="s" d="M6 24l56-6v26l-56-2z"/>${teeth(6, 62, 42.5, 3, 3)}
      <path class="w" d="M58 14h18a10 10 0 0 1 10 10v16a10 10 0 0 1-10 10H58z"/><rect class="h" x="66" y="22" width="12" height="20" rx="6"/>`,
    tenonsaw: `<rect class="s" x="6" y="22" width="56" height="22"/>${teeth(6, 62, 44, 2.5, 2.5)}<rect class="g" x="6" y="17" width="58" height="7" rx="1.5"/>
      <path class="w" d="M60 14h14a12 12 0 0 1 12 12v12a14 14 0 0 1-8 12l-6 4H60z"/><rect class="h" x="66" y="22" width="12" height="18" rx="6"/><circle class="g" cx="63" cy="26" r="1.8"/><circle class="g" cx="63" cy="38" r="1.8"/>`,
    chisels: `<path class="s" d="M8 22l4-6h34v10H8z"/><rect class="s" x="46" y="15" width="7" height="12"/><rect class="w" x="53" y="14" width="34" height="14" rx="5"/>
      <path class="s" d="M14 44l3-4h29v6H14z"/><rect class="s" x="46" y="38" width="7" height="10"/><rect class="w" x="53" y="37" width="30" height="12" rx="5"/>`,
    mortchisel: `<path class="s" d="M8 38l5-12h34v16H8z"/><path d="M14 38h33"/><rect class="s" x="47" y="24" width="7" height="20"/>
      <path class="w" d="M54 24h26l4 4v12l-4 4H54z"/><rect class="s" x="80" y="25" width="6" height="18" rx="2"/>`,
    mallet: `<rect class="w" x="40" y="30" width="9" height="30" rx="3"/><path class="w" d="M16 10h56l-4 22H20z"/><path d="M20 21h48"/>`,
    blockplane: `<path class="d" d="M14 46h68l-6-14H20z"/><path class="s" d="M36 32l10-14h10l-6 14z"/><circle class="g" cx="51" cy="22" r="3"/><circle class="g" cx="72" cy="38" r="3.5"/><path d="M14 46h68"/>`,
    jackplane: `<path class="d" d="M6 48h84v-8H6z"/><path class="d" d="M10 40h76l-4-6H14z"/><path class="s" d="M38 34l10-18h8l-6 18z"/><circle class="g" cx="52" cy="14" r="3"/>
      <ellipse class="w" cx="22" cy="28" rx="6" ry="7"/><path class="w" d="M62 34l6-20a6 6 0 0 1 10 2l-4 18z"/>`,
    jointerplane: `<path class="d" d="M2 46h92v-6H2z"/><path class="d" d="M5 40h86l-3-5H8z"/><path class="s" d="M42 35l8-16h7l-5 16z"/><circle class="g" cx="54" cy="17" r="2.6"/>
      <ellipse class="w" cx="20" cy="29" rx="5" ry="6"/><path class="w" d="M66 35l5-17a5 5 0 0 1 9 2l-4 15z"/>`,
    shoulderplane: `<path class="d" d="M12 46h70V32c-8-10-18-12-28-12H34c-12 0-18 6-22 12z"/><path class="s" d="M40 46l8-30h7l-7 30z"/><circle class="g" cx="64" cy="34" r="4"/><circle class="s" cx="26" cy="34" r="3"/>`,
    routerplane: `<rect class="d" x="10" y="36" width="76" height="8" rx="2"/><circle class="w" cx="20" cy="28" r="8"/><circle class="w" cx="76" cy="28" r="8"/>
      <rect class="s" x="44" y="16" width="8" height="20"/><circle class="g" cx="48" cy="13" r="4"/><path class="s" d="M46 44v10h10v-3h-7v-7z"/>`,
    ploughplane: `<path d="M6 25h32M6 33h32"/><rect class="w" x="6" y="20" width="7" height="30" rx="2"/><rect class="w" x="32" y="16" width="34" height="24" rx="3"/>
      <rect class="s" x="30" y="40" width="40" height="4"/><path class="s" d="M44 16l6-10h6l-4 10z"/><path class="w" d="M66 18l6 0a8 8 0 0 1 8 8v14H66z"/><circle class="g" cx="22" cy="25" r="2.5"/><circle class="g" cx="22" cy="33" r="2.5"/>`,
    scraper: `<rect class="s" x="22" y="14" width="52" height="34" rx="2" transform="rotate(-8 48 31)"/><path d="M28 50l44-6" transform="rotate(-8 48 31)"/><path d="M40 26l14-2M42 32l10-1.5" />`,
    rasp: `<path class="s" d="M6 31l56-5v14l-56-5z"/><path d="M14 31.5h2M22 30.8h2M30 30h2M38 29.2h2M46 28.5h2M54 27.8h2M18 34.5h2M26 35h2M34 35.6h2M42 36.2h2M50 36.8h2"/>
      <rect class="w" x="60" y="25" width="28" height="16" rx="7"/>`,

    // ---------- Power tools ----------
    drill: `<path class="s" d="M2 22h12"/><rect class="s" x="12" y="16" width="12" height="12" rx="2"/><rect class="b" x="22" y="12" width="44" height="20" rx="8"/>
      <path class="b" d="M42 32h16l-3 16H45z"/><rect class="d" x="34" y="46" width="32" height="10" rx="3"/><path d="M46 36h6"/><circle class="d" cx="58" cy="22" r="2"/>`,
    router: `<rect class="d" x="14" y="48" width="64" height="6" rx="2"/><path d="M24 22v26M68 22v26"/><rect class="b" x="32" y="10" width="28" height="38" rx="5"/><rect class="d" x="36" y="16" width="20" height="5" rx="2"/>
      <circle class="w" cx="16" cy="30" r="7"/><circle class="w" cx="76" cy="30" r="7"/><path d="M23 30h9M60 30h9"/><path class="s" d="M44 54v6h4v-6"/>`,
    ros: `<rect class="d" x="16" y="46" width="60" height="9" rx="4"/><path class="b" d="M20 46c0-20 12-30 26-30s26 10 26 30z"/><rect class="d" x="36" y="10" width="20" height="8" rx="4"/>
      <path class="d" d="M72 34h14v9H72"/><path d="M30 40h32"/>`,
    circsaw: `<circle class="s" cx="42" cy="38" r="18"/>${(() => { let d = ""; for (let i = 0; i < 16; i++) { const a = (i / 16) * Math.PI * 2; d += `M${42 + 18 * Math.cos(a)} ${38 + 18 * Math.sin(a)}l${2.5 * Math.cos(a + 1)} ${2.5 * Math.sin(a + 1)}`; } return `<path d="${d}"/>`; })()}
      <path class="b" d="M20 40a22 22 0 0 1 44 0z"/><rect class="s" x="8" y="42" width="74" height="4"/><path class="b" d="M56 22c8-10 22-10 28-2l-6 6c-4-4-10-4-14 0z"/><rect class="d" x="30" y="24" width="14" height="10" rx="3"/>`,
    jigsaw: `<path class="b" d="M16 38V24a10 10 0 0 1 10-10h40a10 10 0 0 1 10 10v14z"/><rect class="h" x="30" y="20" width="32" height="9" rx="4.5"/><rect class="s" x="14" y="38" width="64" height="5"/>
      <path class="s" d="M32 43v16h3V43"/><path d="M35 46l2 1-2 1M35 50l2 1-2 1M35 54l2 1-2 1"/>`,
    domino: `<rect class="b" x="24" y="16" width="52" height="24" rx="8"/><rect class="d" x="12" y="20" width="12" height="28" rx="2"/><rect class="s" x="10" y="30" width="4" height="10"/>
      <path class="d" d="M40 16v-6h28v6"/><rect class="d" x="76" y="24" width="12" height="8" rx="3"/><path d="M34 28h30"/>`,

    // ---------- Stationary machines (three-quarter view) ----------
    tablesaw: `<path class="b" d="M12 34l30 8v16l-30-8z"/><path class="c" d="M42 42l42-10v16l-42 10z"/><path class="s" d="M12 34l42-10 30 8-42 10z"/>
      <path class="s" d="M36 33a13 13 0 0 1 24-6z"/><path d="M38 31.5l3-1M44 27l2-2M51 24.5l1.5-2.5"/><path class="d" d="M58 25l18 4.5-2 2-18-4.5z"/><path d="M20 33l26-6.5"/>`,
    mitersaw: `<ellipse class="s" cx="48" cy="52" rx="38" ry="7"/><rect class="d" x="18" y="40" width="60" height="7" rx="1"/><path class="b" d="M70 46l4-22-30-12-4 6z"/>
      <circle class="s" cx="38" cy="30" r="14"/><path class="b" d="M24 30a14 14 0 0 1 28 0z"/><path class="d" d="M46 12l10-6 6 4-8 8z"/>`,
    jointer: `<path class="b" d="M26 36l18 6v16l-18-6z"/><path class="c" d="M44 42l26-8v16l-26 8z"/><path class="s" d="M6 30l38-10 46 12-38 10z"/><path d="M44 26l8 2"/>
      <path class="s" d="M46 20l30 8v-10l-30-8z"/>`,
    planer: `<path class="b" d="M20 22l26 8v22l-26-8z"/><path class="c" d="M46 30l30-8v22l-30 8z"/><path class="b" d="M20 22l30-8 26 8-30 8z"/>
      <path class="s" d="M4 40l16 4M76 36l16-4"/><path d="M4 40l16-4M76 36l16 4"/><circle class="s" cx="48" cy="12" r="4"/><path d="M48 16v3"/>`,
    bandsaw: `<path class="b" d="M28 6h28a10 10 0 0 1 10 10v36H52V18H42v34H28z"/><path class="s" d="M48 18v34"/><path class="s" d="M24 38h40l6 4H30z"/><rect class="d" x="24" y="52" width="46" height="6" rx="1"/>
      <circle class="s" cx="40" cy="12" r="2"/>`,
    drillpress: `<rect class="s" x="34" y="12" width="6" height="44"/><path class="b" d="M22 8h40a4 4 0 0 1 4 4v10H22z"/><path class="s" d="M50 22v8h4v-8"/><path class="s" d="M51 30v6"/>
      <path d="M66 16l14-6M66 16l14 4"/><circle class="d" cx="80" cy="10" r="2"/><circle class="d" cx="80" cy="20" r="2"/><rect class="s" x="30" y="38" width="36" height="5" rx="1"/><rect class="d" x="18" y="55" width="40" height="6" rx="1"/>`,
    mortiser: `<rect class="s" x="30" y="12" width="6" height="44"/><path class="b" d="M20 8h30a4 4 0 0 1 4 4v14H20z"/><rect class="s" x="40" y="26" width="7" height="12"/><path d="M43.5 38v4"/>
      <path class="w" d="M54 14l30-6 1 5-31 6z"/><rect class="w" x="26" y="44" width="44" height="6"/><rect class="d" x="16" y="55" width="44" height="6" rx="1"/>`,
    routertable: `<path class="s" d="M8 26l40-10 40 10-40 10z"/><path class="w" d="M18 26l34-8.5"/><path d="M14 28v28M48 36v24M82 28v28"/><rect class="b" x="40" y="36" width="16" height="16" rx="3"/>
      <path class="s" d="M48 28v-4"/>`,

    // ---------- Bits, blades & jigs ----------
    forstner: `<rect class="s" x="22" y="6" width="6" height="30"/><path class="s" d="M14 36h22v8l-3 4H17l-3-4z"/><path d="M25 48v4"/>
      <rect class="s" x="60" y="12" width="5" height="26"/><path class="s" d="M54 38h17v6l-2 3H56l-2-3z"/><path d="M62.5 47v3"/>`,
    straightbit: `<rect class="s" x="44" y="4" width="8" height="30"/><rect class="s" x="41" y="34" width="14" height="24" rx="1"/><path d="M44 36l8 20M48 36v4"/>`,
    roundover: `<rect class="s" x="44" y="4" width="8" height="28"/><path class="s" d="M32 32h32v4a12 12 0 0 0-12 12h-8a12 12 0 0 0-12-12z"/><rect class="d" x="42" y="48" width="12" height="8" rx="2"/>`,
    chamferbit: `<rect class="s" x="44" y="4" width="8" height="28"/><path class="s" d="M30 32h36l-12 16h-12z"/><rect class="d" x="42" y="48" width="12" height="8" rx="2"/>`,
    edgeguide: `<path d="M8 24h62M8 38h62"/><rect class="s" x="6" y="22" width="4" height="4"/><rect class="s" x="6" y="36" width="4" height="4"/><rect class="d" x="68" y="12" width="10" height="40" rx="2"/>
      <rect class="w" x="78" y="14" width="6" height="36"/><circle class="b" cx="60" cy="31" r="5"/>`,
    tenonjig: `<rect class="b" x="8" y="48" width="74" height="8" rx="1"/><rect class="s" x="30" y="10" width="8" height="38"/><rect class="w" x="40" y="6" width="12" height="42"/>
      <path d="M54 24h18"/><circle class="b" cx="76" cy="24" r="5"/><rect class="s" x="52" y="20" width="4" height="8"/>`,
    dado: `<circle class="s" cx="36" cy="32" r="22"/><circle class="s" cx="46" cy="32" r="22"/><circle class="s" cx="56" cy="32" r="22"/><circle class="h" cx="56" cy="32" r="4"/>
      <path d="M56 10v4M78 32h-4M56 54v-4"/>`,
    shootboard: `<path class="w" d="M6 44h84v10H6z"/><path class="w" d="M34 36h56v8H34z"/><rect class="w" x="74" y="24" width="9" height="12"/><path class="d" d="M8 44V28h24v16z"/><path class="s" d="M28 28l6-6"/>`,

    // ---------- Clamping & assembly ----------
    barclamps: `<rect class="s" x="4" y="29" width="88" height="5"/><path class="b" d="M74 14h10v30H74z"/><path class="b" d="M18 14h10v30H18z"/><rect class="d" x="70" y="14" width="4" height="22"/><rect class="d" x="28" y="14" width="4" height="22"/>
      <path class="w" d="M20 44h6l2 14h-10z"/>`,
    fclamps: `<rect class="s" x="20" y="6" width="6" height="52"/><rect class="s" x="20" y="6" width="34" height="8"/><path class="s" d="M26 36h28v8H26z"/><path d="M46 14v-0M46 44v-24"/>
      <rect class="d" x="42" y="16" width="8" height="4"/><rect class="w" x="42" y="44" width="8" height="16" rx="3"/>`,
    strap: `<rect x="12" y="16" width="72" height="34" rx="17" class="n" style="stroke-width:7;stroke:var(--art-body)"/><rect x="12" y="16" width="72" height="34" rx="17"/>
      <rect class="s" x="36" y="8" width="24" height="16" rx="2"/><path class="d" d="M60 12l12-4 2 6-12 4z"/>`,
    glue: `<path class="d" d="M42 22l4-14h4l4 14z"/><path class="b" d="M32 22h32v32a4 4 0 0 1-4 4H36a4 4 0 0 1-4-4z"/><rect class="p" x="36" y="32" width="24" height="16" rx="2"/><path d="M40 38h16M40 43h10"/>`,

    // ---------- Finishing ----------
    rags: `<path class="p" d="M8 54c4-14 14-20 26-16 8-8 22-8 30 0 4 4 4 10 2 16z"/><path d="M24 46c6-2 14 2 18 8M44 42c6-2 12 2 16 8"/>
      <rect class="b" x="66" y="20" width="22" height="34" rx="2"/><rect class="d" x="64" y="14" width="26" height="7" rx="2"/><rect class="p" x="70" y="30" width="14" height="14" rx="1"/>`,
    brushes: `<path class="d" d="M8 26h18l4 6-4 6H8z"/><rect class="s" x="26" y="25" width="10" height="14"/><rect class="w" x="36" y="28" width="50" height="8" rx="4"/><circle class="h" cx="80" cy="32" r="1.8"/>
      <path class="d" d="M14 46h12l3 4-3 4H14z"/><rect class="s" x="26" y="45" width="7" height="10"/><rect class="w" x="33" y="47" width="40" height="6" rx="3"/>`,
  };

  // Shared look: ink stroke plus tones, all from CSS variables so the art follows the theme.
  const CSS = `
  :root { --art-ink:#3a332b; --art-wood:#e2bf8a; --art-steel:#d5d9de; --art-body:#e98a6f; --art-body-2:#d26e53; --art-dark:#4a4f57; --art-brass:#d9b55a; --art-paper:#fbf6ea; --art-accent:#d63b32; --art-hole:var(--panel); }
  :root[data-theme="dark"] { --art-ink:#cdbfa6; --art-wood:#c9a06a; --art-steel:#a9afb6; --art-body:#e07a5f; --art-body-2:#b65d45; --art-dark:#3c4048; --art-brass:#bf9a45; --art-paper:#e9e1cf; --art-accent:#ff7d61; }
  svg.art * { stroke: var(--art-ink); stroke-width: 1.5; stroke-linejoin: round; stroke-linecap: round; fill: none; }
  svg.art .w { fill: var(--art-wood); } svg.art .s { fill: var(--art-steel); } svg.art .b { fill: var(--art-body); }
  svg.art .c { fill: var(--art-body-2); } svg.art .d { fill: var(--art-dark); } svg.art .g { fill: var(--art-brass); }
  svg.art .p { fill: var(--art-paper); } svg.art .a { fill: var(--art-accent); } svg.art .h { fill: var(--art-hole); }
  svg.art .n { fill: none; }`;
  const style = document.createElement("style");
  style.textContent = CSS;
  document.head.appendChild(style);

  window.toolArt = function (id, width = 96, cls = "") {
    const h = Math.round((width * 64) / 96);
    return `<svg class="art ${cls}" width="${width}" height="${h}" viewBox="0 0 96 64" aria-hidden="true">${ART[id] || ""}</svg>`;
  };
  window.toolArt.has = (id) => !!ART[id];
})();
