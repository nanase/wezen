import { setLang, t } from "./i18n.js";
import * as G from "./graph.js";
import { ICONS, applyTheme, fitCanvas, h, icon, invoke, listen, tauri } from "./ui.js";

const $ = (id) => document.getElementById(id);

/** 手元に残す長さ。Rust 側の履歴と同じ */
const KEEP_MS = 3_720_000;
/** バイト量のグラフの上限を、新しい値へ近づける速さ（時定数） */
const TOP_EASE_MS = 300;

let settings = null;
let panels = [];
let colors = {};
let dirty = true;
let lastDraw = 0;
/** 上限がまだ動いている途中か。「更新ごと」のスクロールでも、動き終わるまでは描き直す */
let easing = false;

const pct = (key, digits) => (s) => G.percent(s?.[key], digits);
const amount = (used, total) => (s) => `${G.bytes(s?.[used])} / ${G.bytes(s?.[total])}`;

/** 表示する項目。`keys` はグラフにする値、`text` は左上に重ねる行 */
const METRICS = [
  {
    id: "cpu",
    name: "CPU",
    keys: ["cpu"],
    colors: ["--c-cpu"],
    text: [pct("cpu", 2)],
    peak: (v) => G.percent(v, 2),
  },
  {
    id: "memory",
    name: "Memory",
    keys: ["mem"],
    colors: ["--c-mem"],
    text: [pct("mem", 0), amount("memUsed", "memTotal")],
    short: [pct("mem", 0), (s) => G.bytes(s?.memUsed)],
    peak: (v, s) => G.bytes((v / 100) * (s?.memTotal ?? 0)),
  },
  { id: "io", name: "I/O", keys: ["ioRo", "ioW"], labels: ["R+O", "W"] },
  {
    id: "gpu",
    name: "GPU",
    keys: ["gpu"],
    colors: ["--c-gpu"],
    text: [pct("gpu", 2), amount("vramUsed", "vramTotal")],
    short: [pct("gpu", 2), (s) => G.bytes(s?.vramUsed)],
    peak: (v) => G.percent(v, 2),
  },
  { id: "disk", name: "Disk", keys: ["diskR", "diskW"], labels: ["R", "W"] },
  { id: "net", name: "Network", keys: ["netR", "netS"], labels: ["R", "S"] },
];

for (const m of METRICS) {
  m.bytes = !!m.labels;
  if (m.bytes) {
    m.colors = ["--c-read", "--c-write"];
    m.peak = (v) => G.bytes(v);
  }
}

const series = new G.Series(METRICS.flatMap((m) => m.keys), KEEP_MS);

/** メモリはグラフを使用率で描くので、届いた値から求めておく */
function addSample(s) {
  const mem = s.memUsed != null && s.memTotal ? (s.memUsed / s.memTotal) * 100 : null;
  series.push({ ...s, mem });
}

function readColors() {
  const style = getComputedStyle(document.documentElement);
  colors = {};
  for (const name of ["--c-cpu", "--c-mem", "--c-gpu", "--c-read", "--c-write"]) {
    colors[name] = style.getPropertyValue(name).trim();
  }
  dirty = true;
}

// ---- パネルを組む ----

const observer = new ResizeObserver((entries) => {
  for (const entry of entries) {
    const panel = panels.find((p) => p.el === entry.target);
    if (panel) fitCanvas(panel, entry);
  }
  dirty = true;
});

function build() {
  const strip = settings.layout === "strip";
  document.body.className = `layout-${settings.layout}`;
  const list = settings.itemOrder
    .map((id) => METRICS.find((m) => m.id === id))
    .filter((m) => m && settings.items[m.id]);
  const box = $("panels");
  box.style.gridTemplateColumns = "";
  box.style.gridTemplateRows = "";
  if (settings.layout === "stack") {
    box.style.gridTemplateRows = `repeat(${list.length}, minmax(0, 1fr))`;
  } else if (settings.layout === "grid") {
    box.style.gridTemplateColumns = "repeat(2, minmax(0, 1fr))";
    box.style.gridTemplateRows = `repeat(${Math.ceil(list.length / 2)}, minmax(0, 1fr))`;
  } else {
    box.style.gridTemplateColumns = `22px repeat(${list.length}, minmax(0, 1fr))`;
  }

  observer.disconnect();
  panels = list.map((m) => {
    const canvas = h("canvas");
    const peak = h("div", { class: "peak hidden" });
    const overlay = h("div", { class: "overlay" });
    const el = h("div", { class: "panel" }, canvas, peak, overlay);
    return { m, el, canvas, ctx: canvas.getContext("2d"), peak, overlay, w: 0, h: 0, top: null, topAt: 0 };
  });
  const mark = strip ? [h("div", { class: "strip-mark" }, $("header").querySelector(".mark").cloneNode(true))] : [];
  box.replaceChildren(...mark, ...panels.map((p) => p.el));
  for (const p of panels) observer.observe(p.el);
  renderText();
  dirty = true;
}

// ---- 左上の文字 ----

function lines(m, s) {
  if (m.bytes) {
    return m.keys.map((k, i) => ({ label: m.labels[i], value: `${G.bytes(s?.[k])}/s`, color: m.colors[i] }));
  }
  return m.text.map((fn) => ({ value: fn(s) }));
}

function shortValues(m, s) {
  if (m.bytes) return m.keys.map((k, i) => ({ value: G.bytes(s?.[k]), color: `var(${m.colors[i]})` }));
  return m.short ? m.short.map((fn, i) => ({ value: fn(s), color: i ? "var(--muted)" : "var(--text)" })) : [{ value: m.text[0](s) }];
}

function renderText() {
  const s = series.latest;
  const strip = settings.layout === "strip";
  for (const p of panels) {
    const name = h("div", { class: "name" }, p.m.name);
    if (strip) {
      const values = shortValues(p.m, s).map((v) => h("span", { style: v.color ? `color: ${v.color}` : null }, v.value));
      p.overlay.replaceChildren(name, h("div", { class: "values" }, values));
      continue;
    }
    const rows = lines(p.m, s).map((l) =>
      h(
        "div",
        { class: "line" },
        l.color && h("span", { class: "swatch", style: `background: var(${l.color})` }),
        l.label && h("span", { class: "label" }, l.label),
        h("span", {}, l.value),
      ),
    );
    p.overlay.replaceChildren(name, ...rows);
  }
}

// ---- グラフ ----

/**
 * バイト量のグラフの上限。大きな山が来ても急に縮尺を変えず、少しずつ近づける。
 * 近づいている間は、山の頂上が一時的に上限を超えて、上の端で切れる。
 */
function easedTop(p, target, now) {
  if (p.top == null) {
    p.top = target;
  } else {
    const k = 1 - Math.exp(-Math.max(0, now - p.topAt) / TOP_EASE_MS);
    p.top += (target - p.top) * k;
    if (Math.abs(target - p.top) <= target * 0.002) p.top = target;
  }
  p.topAt = now;
  if (p.top !== target) easing = true;
  return p.top;
}

function render(now) {
  const last = series.latest;
  if (!last) return;
  easing = false;
  const span = settings.spanSecs * 1000;
  const strip = settings.layout === "strip";

  for (const p of panels) {
    if (!p.w || !p.h) continue;
    const f = G.frame(G.frameArgs(settings, p.w, now, last.t));
    const { x0, step, end } = f;
    const smoothed = p.m.keys.map((k) => G.line(series.times, series.cols[k], f, settings.smoothing));
    const log = p.m.bytes && settings.byteScale === "log";
    const top = p.m.bytes ? easedTop(p, G.bytesTop(smoothed), now) : 100;
    const scale = G.scaler(top, log);

    G.draw(p.ctx, {
      width: p.w,
      height: p.h,
      x0,
      step,
      curved: settings.smoothing !== "off",
      fillOpacity: settings.fillOpacity,
      series: smoothed.map((vals, i) => ({
        hs: vals.map((v) => (v == null ? null : scale(v))),
        color: colors[p.m.colors[i]],
      })),
    });

    if (strip || !settings.showPeak) {
      G.hidePeak(p.peak);
      continue;
    }
    // 左上の今の値より最大が小さく見えないよう、まだ流れ込んでいない最新の値も数える
    renderPeak(p, { smoothed, scale, x0, step, from: end - span, to: last.t, last });
  }
}

/** ピーク。数値はならす前の最大値、位置はならした山の頂上にする。 */
function renderPeak(p, { smoothed, scale, x0, step, from, to, last }) {
  const raw = G.maxIn(series.times, p.m.keys.map((k) => series.cols[k]), from, to);
  const top = G.summit(smoothed);
  if (raw == null || top.value == null) {
    G.hidePeak(p.peak);
    return;
  }
  G.placePeak(p.peak, {
    x: x0 + top.index * step,
    y: 1 + (1 - scale(top.value)) * (p.h - 1),
    w: p.w,
    corner: settings.peakPos === "corner",
    value: p.m.peak(raw, last),
    maxLabel: t("peakMax"),
  });
}

/** 描き直しの間隔。なめらかに流すときは、いちばん幅の広いグラフが目に見えて流れる間隔にする */
function frameMs() {
  if (easing) return G.MIN_FRAME_MS;
  if (settings.scroll !== "glide") return Infinity;
  const width = Math.max(0, ...panels.map((p) => p.w));
  return G.glideFrameMs(width, settings.spanSecs * 1000, window.devicePixelRatio || 1);
}

function loop(now) {
  requestAnimationFrame(loop);
  if (!settings || document.hidden) return;
  if (!dirty && now - lastDraw < frameMs()) return;
  lastDraw = now;
  dirty = false;
  render(Date.now());
}

// ---- ヘッダーとウィンドウ ----

function renderHeader() {
  const set = (id, paths, label, width = 1.7) => {
    const el = $(id);
    el.replaceChildren(icon(paths, 15, width));
    el.setAttribute("aria-label", label);
    el.title = label;
  };
  set("pin", ICONS.pin, t("pin"));
  $("pin").setAttribute("aria-pressed", String(settings.topmost));
  set("settings", ICONS.gear, t("settings"), 1.6);
  set("hide", ICONS.close, t("hide"));
}

function apply(view) {
  const prev = settings;
  settings = view.settings;
  setLang(view.lang);
  applyTheme(settings.theme);
  renderHeader();
  const rebuild =
    !prev ||
    prev.layout !== settings.layout ||
    JSON.stringify(prev.items) !== JSON.stringify(settings.items) ||
    prev.itemOrder.join() !== settings.itemOrder.join();
  if (rebuild) build();
  else renderText();
  dirty = true;
}

async function save(patch) {
  apply(await invoke("update_settings", { settings: { ...settings, ...patch } }));
}

$("pin").addEventListener("click", () => save({ topmost: !settings.topmost }));
$("settings").addEventListener("click", () => invoke("open_settings"));
$("hide").addEventListener("click", () => invoke("hide_monitor"));

const win = tauri.window.getCurrentWindow();
for (const el of document.querySelectorAll(".resize")) {
  el.addEventListener("mousedown", (e) => {
    if (e.button === 0) win.startResizeDragging(el.dataset.dir);
  });
}

document.addEventListener("themechange", readColors);
document.addEventListener("visibilitychange", () => {
  dirty = true;
});

async function init() {
  // 履歴を取る前に届いた値も落とさないよう、先に受け始める
  const early = [];
  await listen("sample", (e) => {
    if (!settings) {
      early.push(e.payload);
      return;
    }
    addSample(e.payload);
    renderText();
    dirty = true;
  });
  await listen("settings-changed", (e) => apply(e.payload));

  const view = await invoke("get_settings");
  const history = await invoke("get_history");
  for (const s of [...history, ...early]) addSample(s);
  apply(view);
  readColors();
  requestAnimationFrame(loop);
}

init();
