//! 計測のループと履歴。設定した間隔ごとに値を取り、画面へ送る。

use crate::config::Settings;
use crate::state::AppState;
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

/// 1 回分の計測値。取れなかった値は None にする。
///
/// 量は B/s、メモリは B、使用率は %。
#[derive(Clone, Copy, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    /// 計った時刻（UNIX 時間のミリ秒）
    pub t: i64,
    pub cpu: Option<f64>,
    /// メモリはコミット済みの量（物理メモリ + ページファイル）
    pub mem_used: Option<f64>,
    pub mem_total: Option<f64>,
    pub io_ro: Option<f64>,
    pub io_w: Option<f64>,
    pub gpu: Option<f64>,
    pub vram_used: Option<f64>,
    pub vram_total: Option<f64>,
    pub disk_r: Option<f64>,
    pub disk_w: Option<f64>,
    pub net_r: Option<f64>,
    pub net_s: Option<f64>,
}

/// 値の取り出し元。
pub trait Source: Send {
    /// 前回からの差分で求める値は、初回は None になる
    fn sample(&mut self, settings: &Settings) -> Sample;
}

/// 履歴を残す長さ。時間幅の最大（1 時間）に、ならしと流し込みの余白を足す
const KEEP_MS: i64 = 3_600_000 + 120_000;

#[derive(Default)]
pub struct Sampler {
    history: Mutex<VecDeque<Sample>>,
    /// 間隔を変えたときに、待っているループを起こす
    wake: (Mutex<bool>, Condvar),
}

impl Sampler {
    pub fn history(&self) -> Vec<Sample> {
        self.lock().iter().copied().collect()
    }

    pub fn wake(&self) {
        let (flag, cvar) = &self.wake;
        *flag.lock().unwrap_or_else(|e| e.into_inner()) = true;
        cvar.notify_all();
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, VecDeque<Sample>> {
        self.history.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn push(&self, sample: Sample) {
        let mut history = self.lock();
        history.push_back(sample);
        while history.front().is_some_and(|s| sample.t - s.t > KEEP_MS) {
            history.pop_front();
        }
    }

    /// `deadline` まで待つ。途中で起こされたら早めに戻る。
    fn sleep_until(&self, deadline: Instant) {
        let (flag, cvar) = &self.wake;
        let mut woken = flag.lock().unwrap_or_else(|e| e.into_inner());
        while !*woken {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            woken = cvar
                .wait_timeout(woken, deadline - now)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
        *woken = false;
    }
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

/// 計測のスレッドを始める。値は "sample" イベントで全ウィンドウへ送る。
pub fn start(app: &AppHandle, mut source: Box<dyn Source>) {
    let handle = app.clone();
    std::thread::spawn(move || loop {
        let started = Instant::now();
        let settings = handle.state::<AppState>().settings();
        let mut sample = source.sample(&settings);
        sample.t = now_ms();
        handle.state::<Sampler>().push(sample);
        let _ = handle.emit("sample", sample);

        handle
            .state::<Sampler>()
            .sleep_until(started + Duration::from_millis(settings.interval_ms as u64));
    });
}

/// 見本のデータ。画面の確認と、README の画像づくりに使う。
pub mod mock {
    use super::{Sample, Source};
    use crate::config::Settings;

    const K: f64 = 1024.0;
    const M: f64 = K * K;
    const G: f64 = M * K;

    pub struct Mock {
        seed: u64,
        cpu: f64,
        mem: f64,
        gpu: f64,
        vram: f64,
    }

    impl Mock {
        pub fn new() -> Self {
            Self {
                seed: super::now_ms() as u64 | 1,
                cpu: 15.0,
                mem: 0.52,
                gpu: 14.0,
                vram: 1.67 * G,
            }
        }

        /// 0 以上 1 未満の乱数（xorshift）
        fn rand(&mut self) -> f64 {
            self.seed ^= self.seed << 13;
            self.seed ^= self.seed >> 7;
            self.seed ^= self.seed << 17;
            (self.seed >> 11) as f64 / (1u64 << 53) as f64
        }

        /// ふだんは `base` 以下、ときどき `big` 以下、まれに `huge` 前後
        fn burst(&mut self, base: f64, p_big: f64, big: f64, p_huge: f64, huge: f64) -> f64 {
            let x = self.rand();
            if x < p_huge {
                huge * (0.5 + self.rand() * 0.5)
            } else if x < p_huge + p_big {
                big * self.rand()
            } else {
                base * self.rand()
            }
        }
    }

    impl Source for Mock {
        fn sample(&mut self, _settings: &Settings) -> Sample {
            let jitter = self.rand() - 0.5;
            self.cpu = (self.cpu + (15.0 - self.cpu) * 0.15 + jitter * 7.0).clamp(3.0, 100.0);
            let spike = if self.rand() < 0.04 {
                20.0 + self.rand() * 20.0
            } else {
                0.0
            };
            let jitter = self.rand() - 0.5;
            self.mem = (self.mem + jitter * 0.003).clamp(0.5, 0.54);
            let jitter = self.rand() - 0.5;
            self.gpu = (self.gpu + (14.0 - self.gpu) * 0.2 + jitter * 4.0).clamp(2.0, 100.0);
            let jitter = self.rand() - 0.5;
            self.vram = (self.vram + jitter * 8.0 * M).clamp(1.6 * G, 1.75 * G);
            let net_r = if self.rand() < 0.25 {
                0.4 * M + self.rand() * 5.6 * M
            } else {
                self.burst(40.0 * K, 0.1, 600.0 * K, 0.0, 0.0)
            };
            let net_s = if self.rand() < 0.2 {
                0.1 * M + self.rand() * 1.4 * M
            } else {
                self.burst(30.0 * K, 0.1, 300.0 * K, 0.0, 0.0)
            };
            Sample {
                t: 0,
                cpu: Some((self.cpu + spike).min(100.0)),
                mem_used: Some(self.mem * 74.0 * G),
                mem_total: Some(74.0 * G),
                io_ro: Some(self.burst(3.0 * M, 0.08, 60.0 * M, 0.005, G)),
                io_w: Some(self.burst(M, 0.06, 25.0 * M, 0.003, 300.0 * M)),
                gpu: Some(self.gpu),
                vram_used: Some(self.vram),
                vram_total: Some(7.87 * G),
                disk_r: Some(self.burst(120.0 * K, 0.03, 30.0 * M, 0.006, 440.0 * M)),
                disk_w: Some(self.burst(400.0 * K, 0.05, 40.0 * M, 0.004, 200.0 * M)),
                net_r: Some(net_r),
                net_s: Some(net_s),
            }
        }
    }
}
