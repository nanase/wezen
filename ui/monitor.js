import { ICONS, applyTheme, h, icon, invoke, listen, setLang, t, tauri } from "./i18n.js";
import * as G from "./graph.js";

const $ = (id) => document.getElementById(id);

/** 手元に残す長さ。Rust 側の履歴と同じ */
const KEEP_MS = 3_720_000;
/** なめらかに流すときの描き直しの間隔 */
const GLIDE_FRAME_MS = 33;

let settings = null;
let samples = [];
let times = [];
/** グラフにする値を、項目ごとの配列にしたもの。描くたびに作り直さないよう、届いたときに足す */
let columns = {};
let panels = [];
let colors = {};
let dirty = true;
let lastDraw = 0;

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
    id: "ram",
    name: "RAM",
    keys: ["ram"],
    colors: ["--c-ram"],
    text: [pct("ram", 0), amount("ramUsed", "ramTotal")],
    short: [pct("ram", 0), (s) => G.bytes(s?.ramUsed)],
    peak: (v, s) => G.bytes((v / 100) * (s?.ramTotal ?? 0)),
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

/** RAM はグラフを使用率で描くので、届いた値から求めておく */
function normalize(s) {
  const ram = s.ramUsed != null && s.ramTotal ? (s.ramUsed / s.ramTotal) * 100 : null;
  return { ...s, ram };
}

function addSample(s) {
  const n = normalize(s);
  samples.push(n);
  times.push(n.t);
  for (const m of METRICS) for (const k of m.keys) (columns[k] ??= []).push(n[k]);
  const cut = n.t - KEEP_MS;
  let drop = 0;
  while (drop < times.length && times[drop] < cut) drop++;
  if (drop) {
    samples = samples.slice(drop);
    times = times.slice(drop);
    for (const k of Object.keys(columns)) columns[k] = columns[k].slice(drop);
  }
}

function readColors() {
  const style = getComputedStyle(document.documentElement);
  colors = {};
  for (const name of ["--c-cpu", "--c-ram", "--c-gpu", "--c-read", "--c-write"]) {
    colors[name] = style.getPropertyValue(name).trim();
  }
  dirty = true;
}

// ---- パネルを組む ----

const observer = new ResizeObserver((entries) => {
  for (const entry of entries) {
    const panel = panels.find((p) => p.el === entry.target);
    if (!panel) continue;
    const { width, height } = entry.contentRect;
    const dpr = window.devicePixelRatio || 1;
    panel.w = width;
    panel.h = height;
    panel.canvas.width = Math.round(width * dpr);
    panel.canvas.height = Math.round(height * dpr);
    panel.ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  }
  dirty = true;
});

function build() {
  const strip = settings.layout === "strip";
  document.body.className = `layout-${settings.layout}`;
  const list = METRICS.filter((m) => settings.items[m.id]);
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
    return { m, el, canvas, ctx: canvas.getContext("2d"), peak, overlay, w: 0, h: 0, peakKey: "" };
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
  const s = samples[samples.length - 1];
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

function render(now) {
  if (!samples.length) return;
  const last = samples[samples.length - 1];
  const span = settings.spanSecs * 1000;
  const strip = settings.layout === "strip";

  for (const p of panels) {
    if (!p.w || !p.h) continue;
    const f = G.frame({
      width: p.w,
      spanMs: span,
      smoothing: settings.smoothing,
      rightEdge: settings.rightEdge,
      glide: settings.scroll === "glide",
      intervalMs: settings.intervalMs,
      now,
      lastT: last.t,
    });
    const { x0, step, end } = f;
    const smoothed = p.m.keys.map((k) => G.line(times, columns[k], f, settings.smoothing));
    const log = p.m.bytes && settings.byteScale === "log";
    const top = p.m.bytes ? G.bytesTop(smoothed) : 100;
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
      p.peak.classList.add("hidden");
      continue;
    }
    // 左上の今の値より最大が小さく見えないよう、まだ流れ込んでいない最新の値も数える
    renderPeak(p, { smoothed, scale, x0, step, from: end - span, to: last.t, last });
  }
}

/** ピーク。数値はならす前の最大値、位置はならした山の頂上にする。 */
function renderPeak(p, { smoothed, scale, x0, step, from, to, last }) {
  let raw = null;
  for (let i = samples.length - 1; i >= 0 && times[i] > from; i--) {
    if (times[i] > to) continue;
    for (const k of p.m.keys) {
      const v = samples[i][k];
      if (v != null && (raw == null || v > raw)) raw = v;
    }
  }
  let best = null;
  let at = 0;
  for (const vals of smoothed) {
    vals.forEach((v, i) => {
      if (v != null && (best == null || v >= best)) {
        best = v;
        at = i;
      }
    });
  }
  if (raw == null || best == null) {
    p.peak.classList.add("hidden");
    return;
  }
  const x = x0 + at * step;
  const y = 1 + (1 - scale(best)) * (p.h - 1);
  // 山が左上の文字の下にあるときは、重ならないよう右上へ移す
  const corner = settings.peakPos === "corner" || x < p.w * 0.35;
  const text = corner ? `${t("peakMax")} ${p.m.peak(raw, last)}` : p.m.peak(raw, last);
  const left = Math.min(p.w * 0.91, x);
  const key = `${text}|${corner}|${corner ? "" : `${left.toFixed(1)},${Math.max(1, y - 15).toFixed(1)}`}`;
  if (key === p.peakKey) return;
  p.peakKey = key;
  p.peak.textContent = text;
  p.peak.classList.remove("hidden");
  p.peak.classList.toggle("corner", corner);
  p.peak.classList.toggle("at", !corner);
  p.peak.style.left = corner ? "" : `${left}px`;
  p.peak.style.top = corner ? "" : `${Math.max(1, y - 15)}px`;
}

function loop(now) {
  requestAnimationFrame(loop);
  if (!settings || document.hidden) return;
  const glide = settings.scroll === "glide";
  if (glide ? now - lastDraw < GLIDE_FRAME_MS : !dirty) return;
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
    !prev || prev.layout !== settings.layout || JSON.stringify(prev.items) !== JSON.stringify(settings.items);
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
  for (const s of history) addSample(s);
  const lastT = times[times.length - 1] ?? 0;
  for (const s of early) if (s.t > lastT) addSample(s);
  apply(view);
  readColors();
  requestAnimationFrame(loop);
}

init();
