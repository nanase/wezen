import { ICONS, applyTheme, h, icon, invoke, listen, setLang, t } from "./i18n.js";
import * as G from "./graph.js";

const $ = (id) => document.getElementById(id);

/** 手元に残す長さ。プレビューは時間幅の分だけあれば足りる */
const KEEP_MS = 3_720_000;
const FRAME_MS = 33;

let view = null;
let gpus = [];
let section = "general";

// プレビューに使う CPU の値
let times = [];
let cpu = [];
let preview = null;
let lastDraw = 0;

const SECTIONS = [
  ["general", "navGeneral"],
  ["appearance", "navAppearance"],
  ["graph", "navGraph"],
  ["about", "navAbout"],
];

const ITEMS = [
  ["cpu", "CPU", "--c-cpu"],
  ["ram", "RAM", "--c-ram"],
  ["io", "I/O", "--c-read"],
  ["gpu", "GPU", "--c-gpu"],
  ["disk", "Disk", "--c-read"],
  ["net", "Network", "--c-read"],
];

async function save(patch) {
  view = await invoke("update_settings", { settings: { ...view.settings, ...patch } });
  render();
}

// ---- 共通の部品 ----

function row(label, sub, control, off = false) {
  return h(
    "div",
    { class: off ? "row off" : "row" },
    h("div", { class: "text" }, h("div", { class: "label" }, label), sub && h("div", { class: "sub" }, sub)),
    control,
  );
}

function toggle(checked, onChange, label) {
  return h("input", {
    type: "checkbox",
    role: "switch",
    class: "switch",
    checked,
    "aria-label": label,
    onchange: (e) => onChange(e.target.checked),
  });
}

function segmented(options, value, onChange, label, disabled = false) {
  return h(
    "div",
    { class: "segmented", role: "radiogroup", "aria-label": label },
    options.map(([v, text]) =>
      h(
        "button",
        { type: "button", role: "radio", "aria-checked": String(v === value), disabled, onclick: () => onChange(v) },
        text,
      ),
    ),
  );
}

function select(options, value, onChange, label) {
  const el = h(
    "select",
    { "aria-label": label, onchange: (e) => onChange(e.target.value) },
    options.map(([v, text]) => h("option", { value: v }, text)),
  );
  el.value = value;
  return el;
}

const group = (...rows) => h("div", { class: "group" }, rows);

// ---- 全般 ----

function intervalControl(s) {
  const secs = s.intervalMs / 1000;
  const range = h("input", { type: "range", min: "0.5", max: "60", step: "0.5", value: secs, "aria-label": t("interval") });
  const num = h("input", { type: "number", class: "num", min: "0.5", max: "60", step: "0.5", value: secs, "aria-label": t("intervalSecs") });
  const commit = (v) => {
    const n = Number(v);
    if (!(n >= 0.5)) {
      num.value = secs;
      return;
    }
    save({ intervalMs: Math.round(Math.min(60, n) * 2) * 500 });
  };
  range.addEventListener("input", () => {
    num.value = range.value;
  });
  range.addEventListener("change", () => commit(range.value));
  num.addEventListener("change", () => commit(num.value));
  return h("div", { class: "slider" }, range, num, h("span", {}, t("seconds")));
}

function renderGeneral(s) {
  const spans = [60, 120, 300, 600, 1800, 3600].map((v) => [v, v >= 3600 ? t("hours", v / 3600) : t("minutes", v / 60)]);
  const gpuOptions = [["auto", t("gpuAuto")], ...gpus.map((g) => [g.id, g.name])];
  return [
    h("h1", {}, t("navGeneral")),
    group(
      row(t("autostart"), null, toggle(s.autostart, (v) => save({ autostart: v }), t("autostart"))),
      row(t("topmost"), null, toggle(s.topmost, (v) => save({ topmost: v }), t("topmost"))),
      row(t("taskbar"), null, toggle(s.taskbar, (v) => save({ taskbar: v }), t("taskbar"))),
    ),
    h("h2", {}, t("measure")),
    group(
      row(t("interval"), null, intervalControl(s)),
      row(t("span"), null, segmented(spans, s.spanSecs, (v) => save({ spanSecs: v }), t("span"))),
      row(t("gpu"), null, select(gpuOptions, s.gpu ?? "auto", (v) => save({ gpu: v === "auto" ? null : v }), t("gpu"))),
    ),
    h("h2", {}, t("language")),
    group(
      row(
        t("languageLabel"),
        null,
        select(
          [
            ["auto", t("languageAuto")],
            ["ja", "日本語"],
            ["en", "English"],
          ],
          s.language,
          (v) => save({ language: v }),
          t("languageLabel"),
        ),
      ),
    ),
  ];
}

// ---- 表示 ----

function renderAppearance(s) {
  const shown = ITEMS.filter(([id]) => s.items[id]);
  const cells = () => shown.map(([, , color]) => h("div", { class: "cell" }, h("i", { style: `background: var(${color})` })));
  const layouts = [
    ["stack", "layoutStack"],
    ["grid", "layoutGrid"],
    ["strip", "layoutStrip"],
  ];
  return [
    h("h1", {}, t("navAppearance")),
    group(
      row(
        t("theme"),
        null,
        segmented(
          [
            ["system", t("themeSystem")],
            ["light", t("themeLight")],
            ["dark", t("themeDark")],
          ],
          s.theme,
          (v) => save({ theme: v }),
          t("theme"),
        ),
      ),
    ),
    h("h2", {}, t("layout")),
    h(
      "div",
      { class: "cards", role: "radiogroup", "aria-label": t("layout") },
      layouts.map(([id, key]) =>
        h(
          "button",
          { type: "button", role: "radio", class: "card", "aria-checked": String(s.layout === id), onclick: () => save({ layout: id }) },
          h("div", { class: "mini-wrap" }, h("div", { class: `mini mini-${id}` }, cells())),
          h("div", { class: "label" }, t(key)),
        ),
      ),
    ),
    h("h2", {}, t("items")),
    h(
      "div",
      { class: "items" },
      ITEMS.map(([id, name, color]) => {
        const on = s.items[id];
        // 最後に残った 1 項目は外せない
        const last = on && shown.length === 1;
        return h(
          "button",
          {
            type: "button",
            role: "checkbox",
            class: "item",
            "aria-checked": String(on),
            disabled: last,
            onclick: () => save({ items: { ...s.items, [id]: !on } }),
          },
          h("span", { class: "box" }, on && icon(ICONS.check, 12, 2.4)),
          h("span", { class: "dot", style: `background: var(${color})` }),
          h("span", {}, name),
        );
      }),
    ),
  ];
}

// ---- グラフ ----

/** 本体のグラフが、右端の値を待つために遅れる秒数 */
function lagSeconds(s) {
  const f = frameFor(s, view.graphWidth, Date.now(), Date.now());
  return Math.max(1, Math.round(G.frame({ ...f, rightEdge: "delayed" }).lagMs / 1000));
}

function frameFor(s, width, now, lastT) {
  return {
    width,
    spanMs: s.spanSecs * 1000,
    smoothing: s.smoothing,
    rightEdge: s.rightEdge,
    glide: s.scroll === "glide",
    intervalMs: s.intervalMs,
    now,
    lastT,
  };
}

function renderGraph(s) {
  const canvas = h("canvas");
  const value = h("div", { class: "value" });
  const peak = h("div", { class: "peak hidden" });
  const lag = h("span", { class: "lag" });
  const box = h(
    "div",
    { class: "preview" },
    canvas,
    peak,
    h("div", { class: "overlay" }, h("div", { class: "name" }, "CPU"), value),
  );
  preview = { canvas, ctx: canvas.getContext("2d"), value, peak, lag, box, w: 0, h: 0 };
  new ResizeObserver(([entry]) => {
    const { width, height } = entry.contentRect;
    const dpr = window.devicePixelRatio || 1;
    preview.w = width;
    preview.h = height;
    canvas.width = Math.round(width * dpr);
    canvas.height = Math.round(height * dpr);
    preview.ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  }).observe(box);

  const smoothingOn = s.smoothing !== "off";
  const fill = h("input", { type: "range", min: "0.1", max: "0.8", step: "0.05", value: s.fillOpacity, "aria-label": t("fillOpacity") });
  const fillText = h("span", { class: "value" }, `${Math.round(s.fillOpacity * 100)}%`);
  fill.addEventListener("input", () => {
    // 動かしている間はプレビューだけ変え、離したときに保存する
    view.settings.fillOpacity = Number(fill.value);
    fillText.textContent = `${Math.round(fill.value * 100)}%`;
  });
  fill.addEventListener("change", () => save({ fillOpacity: Number(fill.value) }));

  return [
    h("h1", {}, t("navGraph")),
    h(
      "div",
      { class: "chart-wrap" },
      h(
        "div",
        { class: "legend" },
        h("span", {}, h("i", { style: "background: var(--muted); opacity: 0.6" }), t("previewRaw")),
        h("span", {}, h("i", { style: "background: var(--c-cpu)" }), t("previewLine")),
        lag,
      ),
      box,
    ),
    h("h2", {}, t("smoothness")),
    group(
      row(
        t("smoothing"),
        null,
        segmented(
          [
            ["off", t("smoothingOff")],
            ["weak", t("smoothingWeak")],
            ["medium", t("smoothingMedium")],
            ["strong", t("smoothingStrong")],
          ],
          s.smoothing,
          (v) => save({ smoothing: v }),
          t("smoothing"),
        ),
      ),
      row(
        t("rightEdge"),
        smoothingOn ? t("rightEdgeLag", lagSeconds(s)) : t("rightEdgeOff"),
        segmented(
          [
            ["now", t("rightEdgeNow")],
            ["delayed", t("rightEdgeDelayed")],
          ],
          s.rightEdge,
          (v) => save({ rightEdge: v }),
          t("rightEdge"),
          !smoothingOn,
        ),
        !smoothingOn,
      ),
      row(
        t("scroll"),
        null,
        segmented(
          [
            ["glide", t("scrollGlide")],
            ["step", t("scrollStep")],
          ],
          s.scroll,
          (v) => save({ scroll: v }),
          t("scroll"),
        ),
      ),
    ),
    h("h2", {}, t("scaleAndPeak")),
    group(
      row(
        t("byteScale"),
        t("byteScaleSub"),
        segmented(
          [
            ["linear", t("scaleLinear")],
            ["log", t("scaleLog")],
          ],
          s.byteScale,
          (v) => save({ byteScale: v }),
          t("byteScale"),
        ),
      ),
      row(t("showPeak"), null, toggle(s.showPeak, (v) => save({ showPeak: v }), t("showPeak"))),
      row(
        t("peakPos"),
        null,
        segmented(
          [
            ["peak", t("peakAtPeak")],
            ["corner", t("peakCorner")],
          ],
          s.peakPos,
          (v) => save({ peakPos: v }),
          t("peakPos"),
          !s.showPeak,
        ),
        !s.showPeak,
      ),
      row(t("fillOpacity"), null, h("div", { class: "slider" }, fill, fillText)),
    ),
  ];
}

/**
 * プレビューを描く。本体のグラフと同じ格子で計算してから、プレビューの幅へ引き伸ばす。
 * ならしの効き方と右端の遅れが、本体と同じに見える。
 */
function drawPreview(now) {
  const p = preview;
  if (!p || !p.w || !times.length) return;
  const s = view.settings;
  const lastT = times[times.length - 1];
  const f = G.frame(frameFor(s, view.graphWidth, now, lastT));
  const ratio = p.w / view.graphWidth;
  const step = f.step * ratio;
  const x0 = f.x0 * ratio;
  const vals = G.line(times, cpu, f, s.smoothing);
  const style = getComputedStyle(document.documentElement);
  G.draw(p.ctx, {
    width: p.w,
    height: p.h,
    x0,
    step,
    curved: s.smoothing !== "off",
    fillOpacity: s.fillOpacity,
    series: [{ hs: vals.map((v) => (v == null ? null : v / 100)), color: style.getPropertyValue("--c-cpu").trim() }],
  });

  // 元のデータを細い線で重ねる
  const ctx = p.ctx;
  const from = f.end - s.spanSecs * 1000 - f.dt;
  ctx.beginPath();
  let started = false;
  for (let i = 0; i < times.length; i++) {
    if (times[i] < from || cpu[i] == null) continue;
    const x = p.w - ((f.end - times[i]) / f.dt) * step;
    const y = 1 + (1 - cpu[i] / 100) * (p.h - 1);
    if (started) ctx.lineTo(x, y);
    else ctx.moveTo(x, y);
    started = true;
  }
  ctx.globalAlpha = 0.55;
  ctx.lineWidth = 1;
  ctx.strokeStyle = style.getPropertyValue("--muted").trim();
  ctx.stroke();
  ctx.globalAlpha = 1;

  p.value.textContent = G.percent(cpu[cpu.length - 1], 2);
  p.lag.textContent = f.lagMs ? t("lagged", Math.max(1, Math.round(f.lagMs / 1000))) : t("realtime");
  drawPreviewPeak(vals, x0, step, from, lastT);
}

function drawPreviewPeak(vals, x0, step, from, to) {
  const p = preview;
  const s = view.settings;
  let raw = null;
  for (let i = times.length - 1; i >= 0 && times[i] > from; i--) {
    if (times[i] <= to && cpu[i] != null && (raw == null || cpu[i] > raw)) raw = cpu[i];
  }
  let best = null;
  let at = 0;
  vals.forEach((v, i) => {
    if (v != null && (best == null || v >= best)) {
      best = v;
      at = i;
    }
  });
  if (!s.showPeak || raw == null || best == null) {
    p.peak.classList.add("hidden");
    return;
  }
  const x = x0 + at * step;
  const corner = s.peakPos === "corner" || x < p.w * 0.35;
  p.peak.textContent = corner ? `${t("peakMax")} ${G.percent(raw, 2)}` : G.percent(raw, 2);
  p.peak.classList.remove("hidden");
  p.peak.classList.toggle("corner", corner);
  p.peak.classList.toggle("at", !corner);
  p.peak.style.left = corner ? "" : `${Math.min(p.w * 0.91, x)}px`;
  p.peak.style.top = corner ? "" : `${Math.max(1, (1 - best / 100) * (p.h - 1) - 14)}px`;
}

function loop(now) {
  requestAnimationFrame(loop);
  if (section !== "graph" || document.hidden || now - lastDraw < FRAME_MS) return;
  lastDraw = now;
  drawPreview(Date.now());
}

// ---- 情報 ----

function renderAbout() {
  const repo = view.repository.replace(/^https?:\/\//, "");
  return [
    h("h1", {}, t("navAbout")),
    h(
      "div",
      { class: "about-head" },
      h("img", { src: "icon.png", width: 72, height: 72, alt: "" }),
      h("div", {}, h("div", { class: "about-name" }, "Wezen"), h("div", { class: "about-desc" }, t("aboutDesc"))),
    ),
    h(
      "div",
      { class: "kv" },
      h("div", { class: "key" }, t("version")),
      h("div", {}, view.version),
      h("div", { class: "key" }, t("license")),
      h("div", {}, "MIT License"),
      h("div", { class: "key" }, t("repository")),
      h(
        "div",
        {},
        h(
          "a",
          {
            class: "link",
            href: view.repository,
            onclick: (e) => {
              e.preventDefault();
              invoke("open_repository");
            },
          },
          repo,
        ),
      ),
    ),
  ];
}

// ---- 全体 ----

function renderNav() {
  $("nav").replaceChildren(
    h(
      "div",
      { class: "brand" },
      h("img", { src: "icon.png", width: 32, height: 32, alt: "" }),
      h("div", {}, h("div", { class: "brand-name" }, "Wezen"), h("div", { class: "brand-version" }, view.version)),
    ),
    h(
      "div",
      { class: "nav-items" },
      SECTIONS.map(([id, key]) =>
        h(
          "button",
          {
            type: "button",
            "aria-current": section === id ? "page" : null,
            onclick: () => {
              section = id;
              render();
            },
          },
          t(key),
        ),
      ),
    ),
    h("button", { type: "button", class: "button", onclick: () => invoke("quit") }, t("quit")),
  );
}

function render() {
  const s = view.settings;
  setLang(view.lang);
  applyTheme(s.theme);
  renderNav();
  preview = null;
  const content = {
    general: renderGeneral,
    appearance: renderAppearance,
    graph: renderGraph,
    about: renderAbout,
  }[section](s);
  $("content").replaceChildren(...content);
}

function addSample(sample) {
  times.push(sample.t);
  cpu.push(sample.cpu ?? null);
  const cut = sample.t - KEEP_MS;
  let drop = 0;
  while (drop < times.length && times[drop] < cut) drop++;
  if (drop) {
    times = times.slice(drop);
    cpu = cpu.slice(drop);
  }
}

async function init() {
  await listen("sample", (e) => addSample(e.payload));
  await listen("settings-changed", (e) => {
    view = e.payload;
    render();
  });
  view = await invoke("get_settings");
  gpus = await invoke("get_gpus");
  const history = await invoke("get_history");
  const lastT = times[times.length - 1] ?? Infinity;
  const early = { times, cpu };
  times = [];
  cpu = [];
  for (const sample of history) if (sample.t < lastT) addSample(sample);
  early.times.forEach((t0, i) => addSample({ t: t0, cpu: early.cpu[i] }));
  render();
  requestAnimationFrame(loop);
}

init();
