// グラフの計算と描画。本体と、設定画面のプレビューで使う。
//
// 値は時刻付きで届く。描くときに一定の時間刻みの格子へ並べ直すので、
// 更新間隔を途中で変えても、画面上の点の間隔は変わらない。

/** 画面上の点の間隔（CSS px） */
export const POINT_PX = 3;

/** ならしの強さごとの σ（格子の点の数） */
export const SIGMA = { off: 0, weak: 1, medium: 2, strong: 3.5 };

/** これより離れた値どうしは線でつながない（スリープなどで計測が止まっていた） */
const MAX_GAP_MS = 125_000;

/** バイト量のグラフの上限の下限。静かなときに小さな揺れが画面いっぱいに広がらないようにする */
const MIN_BYTES_TOP = 64 * 1024;

/** ならしの重みの片側の幅（点の数）。 */
export function radius(smoothing) {
  return Math.ceil((SIGMA[smoothing] ?? 0) * 3);
}

/**
 * 時刻付きの値を、時間刻み `dt` の格子（`start` から `count` 点）に並べ直す。
 *
 * - 区間 (g − dt, g] に値があれば、その最大値（山を消さないため）
 * - なければ前後の値を線でつなぐ
 * - 最後の値より後ろは、1 刻みぶんだけ最後の値を延ばす。それより先は null
 */
export function resample(ts, vs, start, dt, count) {
  const out = new Array(count).fill(null);
  const n = ts.length;
  if (!vs.some((v) => v != null)) return out;
  let i = 0;
  for (let k = 0; k < count; k++) {
    const g = start + k * dt;
    while (i < n && ts[i] <= g - dt) i++;
    let j = i;
    let max = null;
    for (; j < n && ts[j] <= g; j++) {
      if (vs[j] != null && (max == null || vs[j] > max)) max = vs[j];
    }
    if (max != null) {
      out[k] = max;
      continue;
    }
    let p = i - 1;
    while (p >= 0 && vs[p] == null) p--;
    if (p < 0) continue;
    let q = j;
    while (q < n && vs[q] == null) q++;
    if (q < n) {
      if (ts[q] - ts[p] <= MAX_GAP_MS) {
        out[k] = vs[p] + ((vs[q] - vs[p]) * (g - ts[p])) / (ts[q] - ts[p]);
      }
    } else if (g - ts[p] < 2 * dt) {
      out[k] = vs[p];
    }
  }
  return out;
}

/**
 * 前後を同じ重みで見るガウス平滑化。とがった山が、後ろへ裾を引かず左右対称のなだらかな山になる。
 * null の点は数えず、ある点だけで重みを割り直す（右端はまだ先の値がないので、ここでそうなる）。
 */
export function smooth(vals, smoothing) {
  const sigma = SIGMA[smoothing] ?? 0;
  if (!sigma) return vals.slice();
  const r = radius(smoothing);
  const w = [];
  for (let k = -r; k <= r; k++) w.push(Math.exp(-(k * k) / (2 * sigma * sigma)));
  return vals.map((v, i) => {
    if (v == null) return null;
    let sum = 0;
    let weight = 0;
    for (let k = -r; k <= r; k++) {
      const x = vals[i + k];
      if (x == null) continue;
      sum += x * w[k + r];
      weight += w[k + r];
    }
    return sum / weight;
  });
}

/**
 * 描く範囲と格子を決める。`width` はグラフの幅（CSS px）。
 *
 * 格子は時刻の `dt` の倍数に置く。描くたびに格子がずれると、区間の最大値が揺れてちらつくため。
 * 右端の時刻 `end` は格子の点の間に来るので、そのぶん `x0` で左へずらして描く。
 */
export function frame({ width, spanMs, smoothing, rightEdge, glide, intervalMs, now, lastT }) {
  const count = Math.max(16, Math.round(width / POINT_PX));
  const step = width / (count - 1);
  const dt = spanMs / (count - 1);
  const r = radius(smoothing);
  // 「遅らせて出す」では、右端より先の値がそろうまで待つ
  const lag = rightEdge === "delayed" ? r : 0;
  // なめらかに流すときは 1 間隔ぶん遅らせ、次の値が届くまでのあいだを流して埋める
  const end = (glide ? Math.min(now - intervalMs, lastT) : lastT) - lag * dt;
  const gridEnd = Math.ceil(end / dt) * dt;
  return {
    count,
    step,
    dt,
    r,
    end,
    lagMs: lag * dt,
    start: gridEnd - (count + r) * dt,
    total: count + 1 + 2 * r,
    x0: width - ((end - (gridEnd - count * dt)) / dt) * step,
  };
}

/** 1 系列を格子に並べ直してならし、見える範囲の `count + 1` 点を返す。 */
export function line(times, values, f, smoothing) {
  return smooth(resample(times, values, f.start, f.dt, f.total), smoothing).slice(f.r, f.r + f.count + 1);
}

/** 値を 0〜1 の高さにする。`log` は対数目盛り（1 kB を 1 とした log10）。 */
export function scaler(top, log) {
  if (!log) return (v) => Math.max(0, Math.min(1, v / top));
  const lt = Math.log10(1 + top / 1024);
  return (v) => Math.max(0, Math.min(1, Math.log10(1 + Math.max(0, v) / 1024) / lt));
}

/** バイト量のグラフの上限。見えている範囲の最大に少し余白を足す。 */
export function bytesTop(series) {
  let max = MIN_BYTES_TOP;
  for (const vals of series) for (const v of vals) if (v != null && v > max) max = v;
  return max * 1.15;
}

/**
 * 単調 3 次補間の制御点の傾き。山の頂点を越えて膨らまない。
 * xs は等間隔なので、傾きは 1 点あたりの変化で表す。
 */
function slopes(ys) {
  const n = ys.length;
  const m = new Array(n).fill(0);
  if (n < 2) return m;
  const d = [];
  for (let i = 0; i < n - 1; i++) d.push(ys[i + 1] - ys[i]);
  for (let i = 1; i < n - 1; i++) {
    const a = d[i - 1];
    const b = d[i];
    m[i] = (Math.sign(a) + Math.sign(b)) * Math.min(Math.abs(a), Math.abs(b), 0.25 * Math.abs(a + b)) || 0;
  }
  m[0] = (3 * d[0] - m[1]) / 2;
  m[n - 1] = (3 * d[n - 2] - m[n - 2]) / 2;
  return m;
}

/** null で途切れたところで分けた、連続する区間の [始め, 終わり]。 */
function runs(ys) {
  const out = [];
  let start = -1;
  ys.forEach((y, i) => {
    if (y != null && start < 0) start = i;
    if ((y == null || i === ys.length - 1) && start >= 0) {
      out.push([start, y == null ? i - 1 : i]);
      start = -1;
    }
  });
  return out;
}

function trace(ctx, x0, step, ys, a, b, curved) {
  ctx.moveTo(x0 + a * step, ys[a]);
  if (!curved || b - a < 2) {
    for (let i = a + 1; i <= b; i++) ctx.lineTo(x0 + i * step, ys[i]);
    return;
  }
  const seg = ys.slice(a, b + 1);
  const m = slopes(seg);
  for (let i = 0; i < seg.length - 1; i++) {
    const x = x0 + (a + i) * step;
    ctx.bezierCurveTo(x + step / 3, seg[i] + m[i] / 3, x + (2 * step) / 3, seg[i + 1] - m[i + 1] / 3, x + step, seg[i + 1]);
  }
}

/**
 * 1 つのパネルのグラフを描く。
 *
 * - `x0`: 格子の最初の点の x（CSS px）。`step`: 点の間隔
 * - `series`: [{ hs: 0〜1 の高さ（null を含む）, color }]
 */
export function draw(ctx, { width, height, x0, step, series, curved, fillOpacity }) {
  ctx.clearRect(0, 0, width, height);
  const top = 1;
  const usable = height - top;
  const shapes = series.map((s) => ({ ...s, ys: s.hs.map((v) => (v == null ? null : top + (1 - v) * usable)) }));
  const alpha = series.length > 1 ? fillOpacity * 0.8 : fillOpacity;
  for (const s of shapes) {
    ctx.beginPath();
    for (const [a, b] of runs(s.ys)) {
      trace(ctx, x0, step, s.ys, a, b, curved);
      ctx.lineTo(x0 + b * step, height);
      ctx.lineTo(x0 + a * step, height);
      ctx.closePath();
    }
    ctx.globalAlpha = alpha;
    ctx.fillStyle = s.color;
    ctx.fill();
  }
  ctx.globalAlpha = 1;
  ctx.lineWidth = 1.25;
  ctx.lineJoin = "round";
  for (const s of shapes) {
    ctx.beginPath();
    for (const [a, b] of runs(s.ys)) trace(ctx, x0, step, s.ys, a, b, curved);
    ctx.strokeStyle = s.color;
    ctx.stroke();
  }
}

/** バイト量。1024 ごとに単位を上げ、1000 を超えたら次の単位にする（「1002 kB」と出さない）。 */
export function bytes(v) {
  if (v == null || !Number.isFinite(v)) return "—";
  const units = ["B", "kB", "MB", "GB", "TB"];
  let i = 0;
  while (v >= 1000 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  const digits = i === 0 ? 0 : v < 10 ? 2 : v < 100 ? 1 : 0;
  return `${v.toFixed(digits).replace(/\.0+$/, "")} ${units[i]}`;
}

export function percent(v, digits) {
  if (v == null || !Number.isFinite(v)) return "—";
  return `${v.toFixed(digits)}%`;
}
