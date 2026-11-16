// Resolves themes.md tokens to concrete colors the canvas and chart engines
// can parse, and watches for theme switches. No Solid, no backend.

/** Reads a CSS color token as a resolved rgb/rgba string. */
export function resolveTokenColor(name: string): string {
  const probe = document.createElement("span");
  probe.style.display = "none";
  probe.style.color = `var(${name})`;
  document.body.appendChild(probe);
  const color = getComputedStyle(probe).color;
  probe.remove();
  return color;
}

/** Calls back when the theme preset or light/dark class changes. */
export function observeTheme(onChange: () => void): () => void {
  const observer = new MutationObserver(onChange);
  observer.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["class", "data-theme"],
  });
  return () => {
    observer.disconnect();
  };
}
