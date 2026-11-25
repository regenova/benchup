// Minimal stroke icon set for the prototypes (24×24, currentColor).
(function () {
  const I = {
    select: '<path d="M6 4l12 7-5.5 1.5L10 18z"/>',
    board: '<path d="M3 15l9-5 9 5-9 5z"/><path d="M3 15v-2l9-5 9 5v2"/>',
    panel: '<rect x="4" y="5" width="16" height="14" rx="1"/><path d="M4 9h16"/>',
    draw: '<path d="M4 20l4-1 11-11-3-3L5 16z"/><path d="M14 6l3 3"/>',
    rect: '<rect x="4" y="6" width="16" height="12" rx="1"/>',
    pushpull: '<path d="M4 17l8 4 8-4-8-4z"/><path d="M12 13V3M8.5 6.5L12 3l3.5 3.5"/>',
    move: '<path d="M12 3v18M3 12h18M12 3l-3 3M12 3l3 3M12 21l-3-3M12 21l3-3M3 12l3-3M3 12l3 3M21 12l-3-3M21 12l-3 3"/>',
    rotate: '<path d="M20 12a8 8 0 1 1-2.3-5.7"/><path d="M20 4v4h-4"/>',
    mirror: '<path d="M12 3v18" stroke-dasharray="2 2"/><path d="M9 7L4 17h5zM15 7l5 10h-5z"/>',
    array: '<rect x="3" y="8" width="5" height="8" rx="1"/><rect x="9.5" y="8" width="5" height="8" rx="1"/><rect x="16" y="8" width="5" height="8" rx="1"/>',
    measure: '<path d="M3 16L16 3l5 5L8 21z"/><path d="M7 12l2 2M10 9l2 2M13 6l2 2"/>',
    joint: '<path d="M4 9h6V6h4v3h6v6h-6v3h-4v-3H4z"/>',
    tenon: '<rect x="3" y="8" width="9" height="8"/><path d="M12 10h5v4h-5"/><path d="M19 6v12"/>',
    dovetail: '<path d="M3 6h6l-2 4h4l-2-4h6l-2 4h4v8H3z"/>',
    dowel: '<rect x="3" y="7" width="18" height="10" rx="1"/><circle cx="8" cy="12" r="1.6"/><circle cx="16" cy="12" r="1.6"/>',
    halflap: '<path d="M3 8h10v4h8v4H11v-4H3z"/>',
    finger: '<path d="M3 7h4v4h4V7h4v4h4v6H3z"/>',
    groove: '<path d="M3 8h18v8H3z"/><path d="M10 8v4h4V8"/>',
    screw: '<path d="M12 3v14M9 6h6M9 9h6M9 12h6M12 17l-2 4h4z"/>',
    section: '<path d="M4 8l8-4 8 4-8 4z"/><path d="M4 8v8l8 4 8-4V8"/><path d="M2 13h20" stroke-dasharray="3 2"/>',
    paint: '<path d="M12 3s6 6.5 6 11a6 6 0 0 1-12 0c0-4.5 6-11 6-11z"/>',
    grain: '<path d="M4 7c4-2 6 2 10 0s4-1 6 0M4 12c4-2 6 2 10 0s4-1 6 0M4 17c4-2 6 2 10 0s4-1 6 0"/>',
    finish: '<path d="M5 20l4-4M9 16l8-8 3 3-8 8zM14 6l4 4"/>',
    stock: '<rect x="3" y="5" width="18" height="4" rx="1"/><rect x="3" y="10" width="18" height="4" rx="1"/><rect x="3" y="15" width="12" height="4" rx="1"/>',
    erase: '<path d="M8 20h12M5 15l9-9 5 5-9 9H8z"/>',
    orbit: '<ellipse cx="12" cy="12" rx="9" ry="4"/><circle cx="12" cy="12" r="2.5"/>',
    search: '<circle cx="11" cy="11" r="6"/><path d="M20 20l-4.5-4.5"/>',
    share: '<path d="M12 15V3M8 7l4-4 4 4"/><path d="M5 12v7h14v-7"/>',
    download: '<path d="M12 4v11M8 11l4 4 4-4"/><path d="M5 19h14"/>',
    undo: '<path d="M9 14L4 9l5-5"/><path d="M4 9h11a5 5 0 0 1 0 10h-3"/>',
    redo: '<path d="M15 14l5-5-5-5"/><path d="M20 9H9a5 5 0 0 0 0 10h3"/>',
    eye: '<path d="M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>',
    plus: '<path d="M12 5v14M5 12h14"/>',
    chevronDown: '<path d="M6 9l6 6 6-6"/>',
    chevronRight: '<path d="M9 6l6 6-6 6"/>',
    layers: '<path d="M12 3l9 5-9 5-9-5z"/><path d="M3 13l9 5 9-5"/>',
    more: '<circle cx="5" cy="12" r="1.2"/><circle cx="12" cy="12" r="1.2"/><circle cx="19" cy="12" r="1.2"/>',
    copy: '<rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V5a1 1 0 0 0-1-1H5a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h3"/>',
    hide: '<path d="M3 3l18 18M10.6 5.1A10 10 0 0 1 12 5c6 0 10 7 10 7a17 17 0 0 1-3 3.6M6.6 6.6A17 17 0 0 0 2 12s4 7 10 7a9.6 9.6 0 0 0 4.4-1"/>',
    cube: '<path d="M12 3l8 4.5v9L12 21l-8-4.5v-9z"/><path d="M4 7.5l8 4.5 8-4.5M12 12v9"/>',
    front: '<rect x="5" y="5" width="14" height="14" rx="1"/>',
    top: '<path d="M5 9l7-4 7 4-7 4z"/>',
    list: '<path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01"/>',
    sheet: '<rect x="3" y="4" width="18" height="16" rx="1"/><path d="M3 15h18M14 15v5"/>',
    cutlist: '<rect x="3" y="4" width="18" height="16" rx="1"/><path d="M3 9h18M9 9v11"/>',
    print: '<path d="M6 9V3h12v6M6 18H4v-7h16v7h-2"/><rect x="6" y="14" width="12" height="7"/>',
    check: '<path d="M5 12l5 5 9-10"/>',
    sun: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M2 12h2M20 12h2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>',
    command: '<path d="M9 6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3z"/>',
    magnet: '<path d="M6 3v8a6 6 0 0 0 12 0V3h-4v8a2 2 0 0 1-4 0V3z"/>',
    walk: '<circle cx="13" cy="4" r="2"/><path d="M10 21l2-6 3 3v3M9 11l3-3 3 3 3 1M12 8l-1 5"/>',
    roundover: '<path d="M4 20V10a6 6 0 0 1 6-6h10"/><path d="M4 20h16V4" stroke-dasharray="2 2"/>',
    chamfer: '<path d="M4 20V9l5-5h11"/><path d="M4 20h16V4" stroke-dasharray="2 2"/>',
    profile: '<path d="M4 20V13c3 0 3-4 6-4s3-5 6-5h4"/><path d="M4 20h16V4" stroke-dasharray="2 2"/>',
    curve: '<path d="M3 7h18v10h-3c0-4-3-6-6-6s-6 2-6 6H3z"/>',
    hole: '<rect x="4" y="4" width="16" height="16" rx="1"/><circle cx="12" cy="12" r="3.5"/><path d="M12 7v1.5M12 15.5V17"/>',
    carve: '<path d="M4 20l7-7"/><path d="M11 13l6-8 2 2-8 6z"/><path d="M14 19c2-1 4-1 6 0"/>',
    lock: '<rect x="5" y="11" width="14" height="10" rx="2"/><path d="M8 11V7a4 4 0 0 1 8 0v4"/>',
  };

  window.icon = function (name, size = 18, extra = "") {
    return `<svg class="ic" width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" ${extra}>${I[name] || ""}</svg>`;
  };
  // Replace <i data-icon="name" data-size="16"></i> placeholders.
  window.hydrateIcons = function (root = document) {
    root.querySelectorAll("i[data-icon]").forEach((n) => {
      n.outerHTML = window.icon(n.dataset.icon, Number(n.dataset.size || 18));
    });
  };
})();
