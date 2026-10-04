// 画面に共通の部品と、Tauri の呼び出し。

export function applyTheme(theme) {
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  const resolve = () => (theme === "system" ? (media.matches ? "dark" : "light") : theme);
  document.documentElement.dataset.theme = resolve();
  media.onchange = () => {
    document.documentElement.dataset.theme = resolve();
    document.dispatchEvent(new Event("themechange"));
  };
  document.dispatchEvent(new Event("themechange"));
}

/** 子要素と属性を持つ要素を作る。文字列は textContent として入れる。 */
export function h(tag, attrs = {}, ...children) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (v === false || v == null) continue;
    if (k === "class") el.className = v;
    else if (k === "style") el.setAttribute("style", v);
    else if (k.startsWith("on")) el.addEventListener(k.slice(2).toLowerCase(), v);
    else el.setAttribute(k, v === true ? "" : v);
  }
  for (const c of children.flat()) {
    if (c == null || c === false) continue;
    el.append(c instanceof Node ? c : document.createTextNode(String(c)));
  }
  return el;
}

/** 線で描くアイコン。path は 24×24 の座標。 */
export function icon(paths, size = 16, width = 1.7) {
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("width", size);
  svg.setAttribute("height", size);
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("aria-hidden", "true");
  for (const d of [].concat(paths)) {
    const p = document.createElementNS(ns, "path");
    p.setAttribute("d", d);
    p.setAttribute("stroke-width", width);
    svg.append(p);
  }
  return svg;
}

export const ICONS = {
  pin: [
    "M12 17v5",
    "M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z",
  ],
  gear: [
    "M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z",
    "M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0z",
  ],
  close: ["M18 6 6 18", "m6 6 12 12"],
  check: "M5 12l5 5L20 7",
  grip: ["M9 6h.01", "M15 6h.01", "M9 12h.01", "M15 12h.01", "M9 18h.01", "M15 18h.01"],
};

/**
 * ResizeObserver で測った大きさに、グラフの canvas の解像度を合わせる。
 * `graph` は { canvas, ctx, w, h }。w と h には CSS px の大きさを入れる。
 */
export function fitCanvas(graph, entry) {
  const { width, height } = entry.contentRect;
  const dpr = window.devicePixelRatio || 1;
  graph.w = width;
  graph.h = height;
  graph.canvas.width = Math.round(width * dpr);
  graph.canvas.height = Math.round(height * dpr);
  graph.ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
}

// WebView2 の既定の右クリックメニュー（戻る、印刷など）は Wezen では使わない
document.addEventListener("contextmenu", (e) => e.preventDefault());

export const tauri = window.__TAURI__;
export const invoke = (cmd, args) => tauri.core.invoke(cmd, args);
export const listen = (event, fn) => tauri.event.listen(event, fn);
