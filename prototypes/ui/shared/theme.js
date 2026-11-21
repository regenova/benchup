// BenchUp UI prototypes: light/dark theme switch shared by every prototype A screen.
// Load in <head> before the stylesheet so the saved theme applies without a flash.
(function () {
  "use strict";
  const KEY = "benchup-proto-theme";
  const root = document.documentElement;

  function saved() {
    try {
      const value = localStorage.getItem(KEY);
      return value === "dark" || value === "light" ? value : null;
    } catch (_) {
      return null;
    }
  }
  function initial() {
    const value = saved();
    if (value) return value;
    return window.matchMedia && matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }

  root.dataset.theme = initial();

  const SUN =
    '<svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M2 12h2M20 12h2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/></svg>';
  const MOON =
    '<svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z"/></svg>';

  function paintToggles() {
    const dark = root.dataset.theme === "dark";
    document.querySelectorAll("[data-theme-toggle]").forEach((button) => {
      // In dark mode the button offers light mode, and the other way round.
      button.innerHTML = dark ? SUN : MOON;
      const label = dark ? "Switch to light mode" : "Switch to dark mode";
      button.setAttribute("aria-label", label);
      button.setAttribute("title", label);
      button.setAttribute("aria-pressed", String(dark));
    });
  }

  function set(theme) {
    theme = theme === "dark" ? "dark" : "light";
    root.dataset.theme = theme;
    try {
      localStorage.setItem(KEY, theme);
    } catch (_) {}
    paintToggles();
    window.dispatchEvent(new CustomEvent("themechange", { detail: { theme } }));
  }

  const get = () => (root.dataset.theme === "dark" ? "dark" : "light");

  // Viewport palette for dark mode: a dark stage with a faint warm grid, a soft
  // shadow, wood lit a little lower so it doesn't glow, and legible labels.
  const VIEW_DARK = {
    grid: { minor: "rgba(255,236,200,.035)", major: "rgba(255,236,200,.08)" },
    shadow: "rgba(0,0,0,.62)",
    stroke: "rgba(12,9,6,.72)",
    selectedStroke: "#ff7d61",
    hoverStroke: "rgba(255,125,97,.7)",
    dimColor: "#ff7d61",
    dimText: "#ffd5ca",
    dimBg: "rgba(30,29,24,.94)",
    ambient: 0.5,
    diffuse: 0.42,
  };

  /**
   * Keep a BenchView in step with the theme. The view's own light values are
   * captured once, so light mode stays exactly as the page defined it. `extra`
   * adds page-specific dark overrides. Returns the apply function.
   */
  function bind(view, extra = {}) {
    const dark = Object.assign({}, VIEW_DARK, extra);
    const light = {};
    for (const key in dark) light[key] = view.theme[key];
    const apply = () => {
      Object.assign(view.theme, get() === "dark" ? dark : light);
      view.render();
    };
    window.addEventListener("themechange", apply);
    if (get() === "dark") apply();
    return apply;
  }

  window.benchTheme = {
    get,
    set,
    bind,
    /** Pick a value for the current theme. */
    pick: (light, dark) => (get() === "dark" ? dark : light),
    /** Accent colour usable in SVG attributes, which cannot read CSS variables. */
    accent: () => (get() === "dark" ? "#ff7d61" : "#d63b32"),
  };

  document.addEventListener("DOMContentLoaded", () => {
    paintToggles();
    document.addEventListener("click", (event) => {
      const button = event.target.closest && event.target.closest("[data-theme-toggle]");
      if (button) set(window.benchTheme.get() === "dark" ? "light" : "dark");
    });
  });

  // Keep several open tabs in step.
  window.addEventListener("storage", (event) => {
    if (event.key === KEY && (event.newValue === "dark" || event.newValue === "light") && event.newValue !== root.dataset.theme) {
      root.dataset.theme = event.newValue;
      paintToggles();
      window.dispatchEvent(new CustomEvent("themechange", { detail: { theme: event.newValue } }));
    }
  });
})();
