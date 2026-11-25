// BenchUp UI prototypes: the stage bar in the header.
//
// Load this right after the <nav class="stages"> element, before the page's own
// scripts, so pages attach their handlers to the rearranged stage links. The header
// is the same on every project screen; screen-specific actions live in each sidebar.
//
//   ?nav=split     (default) Three numbered stages for the model, plus a separate,
//                  unnumbered group of outputs: Cut list, Estimate, Plan and Drawings.
//                  Estimate can be hidden from My workshop → How you work.
//   ?nav=dropdown  Design, Joinery, Shaping and a fourth "Preparation" stage that opens
//                  a menu with Cut list, Estimate, Planning and Shop drawings.
//
// The choice is remembered across pages and reloads.
(function () {
  "use strict";
  const KEY = "benchup-proto-nav";
  const SHOW_ESTIMATE = "benchup-proto-show-estimate";
  const VARIANTS = ["split", "dropdown"];
  const store = {
    get(key) { try { return localStorage.getItem(key); } catch (_) { return null; } },
    set(key, value) { try { localStorage.setItem(key, value); } catch (_) {} },
  };

  const asked = new URLSearchParams(location.search).get("nav");
  if (VARIANTS.includes(asked)) store.set(KEY, asked);
  const variant = VARIANTS.includes(store.get(KEY)) ? store.get(KEY) : "split";
  window.benchNav = { variant, VARIANTS, set: (v) => { store.set(KEY, v); location.reload(); } };

  const nav = document.currentScript && document.currentScript.previousElementSibling;
  if (!nav || !nav.classList.contains("stages")) return;

  const css = `
    .navwrap { display: flex; align-items: center; gap: 10px; }
    /* Injected controls copy the stage look without the .stage class, which pages bind to. */
    .stagex { height: 34px; padding: 0 14px 0 8px; border-radius: 8px; display: flex; align-items: center; gap: 8px; color: var(--muted); font-weight: 500; text-decoration: none; }
    .stagex .n { width: 20px; height: 20px; border-radius: 50%; display: grid; place-items: center; font-size: 11px; border: 1px solid var(--line-strong); color: var(--muted); background: var(--panel); }
    .stagex:hover { color: var(--ink); }
    .stagex.done .n { background: var(--done-bg); border-color: var(--done-line); color: var(--done-ink); }
    .stagex.active { background: var(--panel); color: var(--ink); box-shadow: var(--stage-shadow), 0 0 0 1px var(--line); }
    .stagex.active .n { background: var(--accent); border-color: var(--accent); color: var(--on-accent); }
    .stagex.locked { opacity: .5; }
    .stage-menu { position: relative; display: flex; }
    .stage-trigger .chev { color: var(--faint); margin-left: -2px; transition: transform .15s; }
    .stage-menu.open .stage-trigger .chev { transform: rotate(180deg); }
    .stage-trigger .sub { color: var(--muted); font-weight: 400; }
    .stage-pop { position: absolute; top: calc(100% + 8px); left: 50%; transform: translateX(-50%); width: 330px; background: var(--panel); border: 1px solid var(--line); border-radius: 12px; box-shadow: var(--pop-shadow); padding: 6px; display: none; z-index: 40; }
    .stage-menu.open .stage-pop { display: grid; gap: 2px; }
    .stage-pop .stage, .stage-pop .stagex { height: auto; padding: 9px 10px; display: grid; grid-template-columns: 22px 1fr auto; gap: 10px; align-items: center; text-align: left; text-decoration: none; color: var(--ink); width: 100%; font-weight: 500; box-shadow: none; }
    .stage-pop .stage:hover, .stage-pop .stagex:hover { background: var(--hover); }
    .stage-pop .stage.active, .stage-pop .stagex.active { background: var(--accent-soft); box-shadow: none; }
    .stage-pop .stage .n { display: none; }
    .stage-pop .ico { color: var(--muted); }
    .stage-pop .active .ico { color: var(--accent); }
    .stage-pop small { display: block; color: var(--muted); font-weight: 400; font-size: 11.5px; margin-top: 1px; }
    .stage-pop .tick { color: var(--done-ink); font-size: 12px; visibility: hidden; }
    .stage-pop .done .tick { visibility: visible; }
    .stage-pop .stage.locked::after { display: none; }
    .stage-pop .stage.locked { opacity: .5; }
    /* The outputs get their own lighter pill, clearly apart from the numbered model stages. */
    .nav-split .navwrap { gap: 28px; }
    .outputs { display: flex; align-items: center; gap: 2px; padding: 3px; border: 1px solid var(--line); border-radius: 11px; position: relative; }
    .outputs::before { content: ""; position: absolute; left: -15px; top: 8px; bottom: 8px; width: 1px; background: var(--line-strong); }
    .outputs .stage, .outputs .stagex { height: 32px; padding: 0 10px; gap: 6px; font-size: 12.5px; text-decoration: none; color: var(--muted); box-shadow: none; background: none; }
    .outputs .stage .n { display: none; }
    .outputs .stage:hover, .outputs .stagex:hover { color: var(--ink); background: var(--hover); }
    .outputs .stage.active, .outputs .stagex.active { color: var(--accent); background: var(--accent-soft); box-shadow: none; }
    .outputs .stage.locked, .outputs .stagex.locked { opacity: .45; }
    .outputs .stage.hidden-estimate { display: none; }
  `;
  const style = document.createElement("style");
  style.textContent = css;
  document.head.appendChild(style);

  const items = [...nav.querySelectorAll(".stage")];
  const label = (el) => el.textContent.replace(/^\s*\d+/, "").trim();
  const find = (name) => items.find((el) => label(el) === name);
  const model = ["Design", "Joinery", "Shaping"].map(find).filter(Boolean);
  const outputs = ["Cut list", "Estimate", "Planning"].map(find).filter(Boolean);
  const names = new Map(outputs.map((el) => [el, label(el)]));
  const isDrawings = /a-shop-drawings\.html$/.test(location.pathname);
  const INFO = {
    "Cut list": { icon: "cutlist", sub: "Cut sizes, board layouts and what to buy" },
    Estimate: { icon: "estimate", sub: "Time, cost and the client quote" },
    Planning: { icon: "list", sub: "Build steps that fit your tools" },
    Drawings: { icon: "print", sub: "Printable plans for every part" },
  };
  const iconTag = (name, size = 16) => `<i data-icon="${name}" data-size="${size}"></i>`;
  const setText = (el, text) => { for (const n of [...el.childNodes]) if (n.nodeType === 3) n.remove(); el.appendChild(document.createTextNode(text)); };

  // The wrapper keeps the header's three-column grid intact.
  const wrap = document.createElement("div");
  wrap.className = "navwrap";
  nav.replaceWith(wrap);
  wrap.appendChild(nav);
  document.body.classList.add("nav-" + variant);

  if (variant === "dropdown") {
    const menu = document.createElement("div");
    menu.className = "stage-menu";
    menu.innerHTML = `<button class="stagex stage-trigger" type="button" aria-haspopup="true" aria-expanded="false"><span class="n">4</span><span class="lbl">Preparation</span><span class="sub"></span><span class="chev">${iconTag("chevronDown", 14)}</span></button><div class="stage-pop" role="menu"></div>`;
    nav.appendChild(menu);
    const trigger = menu.querySelector(".stage-trigger");
    const pop = menu.querySelector(".stage-pop");
    for (const el of outputs) {
      const name = names.get(el);
      setText(el, "");
      el.insertAdjacentHTML("beforeend", `<span class="ico">${iconTag(INFO[name].icon, 18)}</span><span class="nm">${name}<small>${INFO[name].sub}</small></span><span class="tick">✓</span>`);
      el.setAttribute("role", "menuitem");
      pop.appendChild(el);
    }
    const drawings = document.createElement("a");
    drawings.className = "stagex" + (isDrawings ? " active" : "");
    drawings.href = "a-shop-drawings.html";
    drawings.setAttribute("role", "menuitem");
    drawings.innerHTML = `<span class="ico">${iconTag(INFO.Drawings.icon, 18)}</span><span class="nm">Shop drawings<small>${INFO.Drawings.sub}</small></span><span class="tick">✓</span>`;
    if (outputs[0] && outputs[0].classList.contains("locked")) drawings.classList.add("locked");
    drawings.addEventListener("click", (e) => { if (drawings.classList.contains("locked")) e.preventDefault(); });
    pop.appendChild(drawings);
    names.set(drawings, "Shop drawings");
    const menuItems = [...outputs, drawings];
    const sync = () => {
      // Drawings follow the other outputs: locked until the project has parts.
      const lock = !!outputs[0] && outputs[0].classList.contains("locked");
      if (drawings.classList.contains("locked") !== lock) drawings.classList.toggle("locked", lock);
      const active = menuItems.find((el) => el.classList.contains("active"));
      trigger.classList.toggle("active", !!active);
      trigger.classList.toggle("done", outputs.every((el) => el.classList.contains("done")));
      trigger.classList.toggle("locked", outputs.every((el) => el.classList.contains("locked")));
      trigger.querySelector(".sub").textContent = active ? "· " + names.get(active) : "";
    };
    new MutationObserver(sync).observe(pop, { subtree: true, attributes: true, attributeFilter: ["class"] });
    sync();
    const close = () => { menu.classList.remove("open"); trigger.setAttribute("aria-expanded", "false"); };
    trigger.addEventListener("click", (e) => {
      e.stopPropagation();
      const open = !menu.classList.contains("open");
      menu.classList.toggle("open", open);
      trigger.setAttribute("aria-expanded", String(open));
    });
    pop.addEventListener("click", (e) => { if (!e.target.closest(".locked")) close(); });
    document.addEventListener("click", (e) => { if (!menu.contains(e.target)) close(); });
    document.addEventListener("keydown", (e) => { if (e.key === "Escape") close(); });
  }

  if (variant === "split") {
    const group = document.createElement("div");
    group.className = "outputs";
    group.setAttribute("aria-label", "Outputs");
    for (const el of outputs) {
      const name = names.get(el);
      setText(el, "");
      el.insertAdjacentHTML("afterbegin", iconTag(INFO[name].icon, 15));
      el.appendChild(document.createTextNode(name === "Planning" ? "Plan" : name));
      el.classList.remove("done");
      group.appendChild(el);
    }
    // Shop drawings join the outputs instead of being the header's primary button.
    const drawings = document.createElement("a");
    drawings.className = "stagex" + (isDrawings ? " active" : "");
    drawings.href = "a-shop-drawings.html";
    drawings.innerHTML = `${iconTag("print", 15)}Drawings`;
    if (outputs[0] && outputs[0].classList.contains("locked")) drawings.classList.add("locked");
    // Follow the other outputs: locked until the project has parts.
    new MutationObserver(() => drawings.classList.toggle("locked", !!outputs[0] && outputs[0].classList.contains("locked"))).observe(outputs[0] || drawings, { attributes: true, attributeFilter: ["class"] });
    drawings.addEventListener("click", (e) => { if (drawings.classList.contains("locked")) e.preventDefault(); });
    group.appendChild(drawings);
    wrap.appendChild(group);

    // Estimate is hidden for woodworkers who never price their work. The preference lives in
    // My workshop → How you work, not in the navigation; an open Estimate stays visible.
    const estimate = outputs.find((el) => names.get(el) === "Estimate");
    if (estimate && store.get(SHOW_ESTIMATE) === "0" && !estimate.classList.contains("active")) estimate.classList.add("hidden-estimate");
  }

  // Pages hydrate icons in their own scripts; this covers any page that runs before icons load.
  document.addEventListener("DOMContentLoaded", () => { if (window.hydrateIcons) window.hydrateIcons(wrap); });
})();
