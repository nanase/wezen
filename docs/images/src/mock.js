// README の画像を作るための見本データ。Tauri の代わりに window.__TAURI__ を用意し、
// 実際の画面（ui/）に見本の値を渡す。実在のマシンの値は使わない。

(() => {
  const K = 1024;
  const M = K * K;
  const G = M * K;

  // URL の ?theme=light&layout=grid で、テーマとレイアウトを選ぶ
  const params = new URLSearchParams(location.search);

  const settings = {
    autostart: false,
    topmost: true,
    taskbar: false,
    intervalMs: 1000,
    spanSecs: 120,
    gpu: null,
    language: "ja",
    theme: params.get("theme") ?? "dark",
    layout: params.get("layout") ?? "stack",
    items: { cpu: true, memory: true, io: true, gpu: true, disk: true, net: true },
    itemOrder: ["cpu", "memory", "io", "gpu", "disk", "net"],
    smoothing: "weak",
    rightEdge: "now",
    scroll: "glide",
    byteScale: "linear",
    showPeak: true,
    peakPos: "peak",
    fillOpacity: 0.35,
  };

  // 毎回同じ絵になるよう、乱数の種を固定する。テーマごとに少し違う絵にする
  let seed = settings.theme === "light" ? 20261002 : 1002;
  const rand = () => {
    seed = (seed * 16807) % 2147483647;
    return seed / 2147483647;
  };

  // ゆっくり動く値に、ときどき山を足す
  const state = { cpu: 16, mem: 0.46, gpu: 22 };
  const humps = { cpu: [], io: [], disk: [], net: [] };
  const hump = (list, i, chance, size, width) => {
    if (rand() < chance) list.push({ at: i + width * 2, h: size * (0.4 + rand() * 0.6), w: width * (0.6 + rand() * 0.8) });
    return list.reduce((sum, b) => sum + b.h * Math.exp(-((i - b.at) ** 2) / (2 * b.w * b.w)), 0);
  };
  const next = (t, i) => {
    state.cpu += (16 - state.cpu) * 0.12 + (rand() - 0.5) * 5;
    state.mem = Math.min(0.5, Math.max(0.44, state.mem + (rand() - 0.5) * 0.004));
    state.gpu += (24 + 8 * Math.sin(i / 11) - state.gpu) * 0.25 + (rand() - 0.5) * 3;
    const io = hump(humps.io, i, 0.07, 60 * M, 1.5);
    const disk = hump(humps.disk, i, 0.05, 45 * M, 1.5);
    const net = hump(humps.net, i, 0.06, 7 * M, 3);
    return {
      t,
      cpu: Math.min(100, Math.max(2, state.cpu + hump(humps.cpu, i, 0.04, 30, 3))),
      memUsed: state.mem * 48 * G,
      memTotal: 48 * G,
      ioRo: io + 2 * M * rand(),
      ioW: io * 0.3 * rand() + M * rand(),
      gpu: Math.min(100, Math.max(1, state.gpu)),
      vramUsed: 3.2 * G + 0.1 * G * Math.sin(i / 20),
      vramTotal: 12 * G,
      diskR: disk * 0.7 + 200 * K * rand(),
      diskW: disk * 0.4 * rand() + 400 * K * rand(),
      netR: net + 80 * K * rand(),
      netS: net * 0.12 + 40 * K * rand(),
    };
  };

  const now = Date.now();
  const history = [];
  let i = 0;
  for (let n = 170; n > 0; n--) history.push(next(now - n * settings.intervalMs, i++));

  const handlers = {};
  setInterval(() => handlers.sample?.({ payload: next(Date.now(), i++) }), settings.intervalMs);

  const results = {
    get_settings: () => ({
      settings: structuredClone(settings),
      lang: "ja",
      version: "0.1.0",
      repository: "https://github.com/nanase/wezen",
      graphWidth: 326,
    }),
    get_history: () => history,
  };

  window.__TAURI__ = {
    core: { invoke: async (cmd) => (results[cmd] ? results[cmd]() : null) },
    event: { listen: async (event, fn) => (handlers[event] = fn, () => {}) },
    window: { getCurrentWindow: () => ({ startDragging() {}, startResizeDragging() {} }) },
  };
})();
