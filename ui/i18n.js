// 画面の文言と、画面に共通の部品。Rust 側の文言は src-tauri/src/i18n.rs にある。

const ja = {
  pin: "常に手前に表示",
  settings: "設定",
  hide: "隠す",
  peakMax: "最大",

  navGeneral: "全般",
  navAppearance: "表示",
  navGraph: "グラフ",
  navAbout: "情報",
  quit: "Wezen を終了",
  autostart: "Windows の起動時に開始",
  topmost: "常に手前に表示",
  taskbar: "タスクバーにボタンを出す",
  measure: "計測",
  interval: "更新間隔",
  intervalSecs: "更新間隔（秒）",
  seconds: "秒",
  span: "グラフの時間幅",
  minutes: (n) => `${n} 分`,
  hours: (n) => `${n} 時間`,
  gpu: "GPU",
  gpuAuto: "自動（専用メモリが最も多い GPU）",
  language: "言語",
  languageLabel: "表示する言語",
  languageAuto: "システムに合わせる",
  theme: "テーマ",
  themeSystem: "システム",
  themeLight: "ライト",
  themeDark: "ダーク",
  layout: "レイアウト",
  layoutStack: "縦積み",
  layoutGrid: "2 列",
  layoutStrip: "帯",
  items: "表示する項目",
  moveItem: (name) => `${name} を移動`,
  previewRaw: "元のデータ",
  previewLine: "表示される線",
  realtime: "リアルタイム",
  lagged: (n) => `約 ${n} 秒遅れて表示`,
  smoothness: "なめらかさ",
  smoothing: "ならしの強さ",
  smoothingOff: "なし",
  smoothingWeak: "弱",
  smoothingMedium: "中",
  smoothingStrong: "強",
  rightEdge: "右端の扱い",
  rightEdgeNow: "すぐ出す",
  rightEdgeDelayed: "遅らせて出す",
  rightEdgeLag: (n) => `遅らせると約 ${n} 秒遅れます`,
  rightEdgeOff: "ならしが「なし」のときは無効",
  scroll: "スクロール",
  scrollGlide: "なめらか",
  scrollStep: "更新ごと",
  scaleAndPeak: "目盛りと表示",
  byteScale: "通信量の目盛り",
  byteScaleSub: "I/O、Disk、Network",
  scaleLinear: "線形",
  scaleLog: "対数",
  showPeak: "ピークを表示",
  peakPos: "ピークの位置",
  peakAtPeak: "山の上",
  peakCorner: "右上",
  fillOpacity: "塗りの濃さ",
  aboutDesc: "CPU、メモリ、GPU、ディスク、ネットワークの使用状況を、小さなグラフで表示します",
  version: "バージョン",
  license: "ライセンス",
  repository: "リポジトリ",
};

const en = {
  pin: "Keep on top",
  settings: "Settings",
  hide: "Hide",
  peakMax: "Max",

  navGeneral: "General",
  navAppearance: "Appearance",
  navGraph: "Graph",
  navAbout: "About",
  quit: "Quit Wezen",
  autostart: "Start with Windows",
  topmost: "Keep on top",
  taskbar: "Show in the taskbar",
  measure: "Measurement",
  interval: "Update interval",
  intervalSecs: "Update interval (seconds)",
  seconds: "s",
  span: "Graph time span",
  minutes: (n) => `${n} min`,
  hours: (n) => `${n} h`,
  gpu: "GPU",
  gpuAuto: "Automatic (most dedicated memory)",
  language: "Language",
  languageLabel: "Display language",
  languageAuto: "Match the system",
  theme: "Theme",
  themeSystem: "System",
  themeLight: "Light",
  themeDark: "Dark",
  layout: "Layout",
  layoutStack: "Stacked",
  layoutGrid: "2 columns",
  layoutStrip: "Strip",
  items: "Items to show",
  moveItem: (name) => `Move ${name}`,
  previewRaw: "Raw data",
  previewLine: "Drawn line",
  realtime: "Real time",
  lagged: (n) => `About ${n} s behind`,
  smoothness: "Smoothness",
  smoothing: "Smoothing",
  smoothingOff: "Off",
  smoothingWeak: "Weak",
  smoothingMedium: "Medium",
  smoothingStrong: "Strong",
  rightEdge: "Right edge",
  rightEdgeNow: "Show now",
  rightEdgeDelayed: "Wait for data",
  rightEdgeLag: (n) => `Waiting delays the graph by about ${n} s`,
  rightEdgeOff: "Not used when smoothing is off",
  scroll: "Scrolling",
  scrollGlide: "Smooth",
  scrollStep: "Per update",
  scaleAndPeak: "Scale and labels",
  byteScale: "Byte scale",
  byteScaleSub: "I/O, Disk, Network",
  scaleLinear: "Linear",
  scaleLog: "Log",
  showPeak: "Show the peak",
  peakPos: "Peak position",
  peakAtPeak: "On the peak",
  peakCorner: "Top right",
  fillOpacity: "Fill opacity",
  aboutDesc: "Shows CPU, memory, GPU, disk and network usage in small graphs.",
  version: "Version",
  license: "License",
  repository: "Repository",
};

let dict = ja;

export function setLang(value) {
  dict = value === "en" ? en : ja;
  document.documentElement.lang = value === "en" ? "en" : "ja";
}

export function t(key, ...args) {
  const v = dict[key] ?? ja[key] ?? key;
  return typeof v === "function" ? v(...args) : v;
}

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

// WebView2 の既定の右クリックメニュー（戻る、印刷など）は Wezen では使わない
document.addEventListener("contextmenu", (e) => e.preventDefault());

export const tauri = window.__TAURI__;
export const invoke = (cmd, args) => tauri.core.invoke(cmd, args);
export const listen = (event, fn) => tauri.event.listen(event, fn);
