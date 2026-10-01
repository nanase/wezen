//! 設定の保存。アプリの設定ディレクトリの config.json に置く。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    System,
    Dark,
    Light,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Language {
    Auto,
    Ja,
    En,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Layout {
    /// A. 縦積み
    Stack,
    /// B. 2 列
    Grid,
    /// C. 帯
    Strip,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Smoothing {
    Off,
    Weak,
    Medium,
    Strong,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RightEdge {
    /// 先の値がそろう前から、あるぶんだけで平均して出す
    Now,
    /// 前後の値がそろうまで待って出す
    Delayed,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Scroll {
    Glide,
    Step,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ByteScale {
    Linear,
    Log,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PeakPos {
    Peak,
    Corner,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct Items {
    pub cpu: bool,
    /// 以前は物理メモリだけの「ram」だった
    #[serde(alias = "ram")]
    pub memory: bool,
    pub io: bool,
    pub gpu: bool,
    pub disk: bool,
    pub net: bool,
}

impl Default for Items {
    fn default() -> Self {
        Self {
            cpu: true,
            memory: true,
            io: true,
            gpu: true,
            disk: true,
            net: true,
        }
    }
}

impl Items {
    pub fn count(&self) -> usize {
        [
            self.cpu,
            self.memory,
            self.io,
            self.gpu,
            self.disk,
            self.net,
        ]
        .iter()
        .filter(|&&on| on)
        .count()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub autostart: bool,
    pub topmost: bool,
    pub taskbar: bool,
    pub interval_ms: u32,
    pub span_secs: u32,
    /// 表示する GPU の LUID。None は専用メモリが最も多い GPU
    pub gpu: Option<String>,
    pub language: Language,
    pub theme: Theme,
    pub layout: Layout,
    pub items: Items,
    pub smoothing: Smoothing,
    pub right_edge: RightEdge,
    pub scroll: Scroll,
    pub byte_scale: ByteScale,
    pub show_peak: bool,
    pub peak_pos: PeakPos,
    pub fill_opacity: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            autostart: false,
            topmost: true,
            taskbar: false,
            interval_ms: 1000,
            span_secs: 120,
            gpu: None,
            language: Language::Auto,
            theme: Theme::System,
            layout: Layout::Stack,
            items: Items::default(),
            smoothing: Smoothing::Weak,
            right_edge: RightEdge::Now,
            scroll: Scroll::Glide,
            byte_scale: ByteScale::Linear,
            show_peak: true,
            peak_pos: PeakPos::Peak,
            fill_opacity: 0.35,
        }
    }
}

impl Settings {
    pub const MIN_INTERVAL_MS: u32 = 500;
    pub const MAX_INTERVAL_MS: u32 = 60_000;
    pub const SPANS: [u32; 6] = [60, 120, 300, 600, 1800, 3600];

    pub fn normalized(mut self) -> Self {
        // 0.5 秒刻みにそろえる
        let ms = self
            .interval_ms
            .clamp(Self::MIN_INTERVAL_MS, Self::MAX_INTERVAL_MS);
        self.interval_ms = (ms + 250) / 500 * 500;
        if !Self::SPANS.contains(&self.span_secs) {
            self.span_secs = *Self::SPANS
                .iter()
                .min_by_key(|s| s.abs_diff(self.span_secs))
                .unwrap_or(&120);
        }
        self.fill_opacity = if self.fill_opacity.is_finite() {
            self.fill_opacity.clamp(0.1, 0.8)
        } else {
            0.35
        };
        if self.items.count() == 0 {
            self.items = Items::default();
        }
        self
    }
}

/// ウィンドウの大きさ（CSS px）。
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

impl Size {
    const fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }
}

/// 本体ウィンドウの位置と、レイアウトごとの大きさ。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct WindowState {
    /// 左上の位置（物理ピクセル）。None は初めての起動
    pub position: Option<(i32, i32)>,
    pub stack: Size,
    pub grid: Size,
    pub strip: Size,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            position: None,
            stack: Self::default_size(Layout::Stack),
            grid: Self::default_size(Layout::Grid),
            strip: Self::default_size(Layout::Strip),
        }
    }
}

impl WindowState {
    pub const fn default_size(layout: Layout) -> Size {
        match layout {
            Layout::Stack => Size::new(340.0, 440.0),
            Layout::Grid => Size::new(560.0, 300.0),
            Layout::Strip => Size::new(944.0, 58.0),
        }
    }

    pub const fn min_size(layout: Layout) -> Size {
        match layout {
            Layout::Stack => Size::new(200.0, 260.0),
            Layout::Grid => Size::new(360.0, 200.0),
            Layout::Strip => Size::new(480.0, 44.0),
        }
    }

    pub fn size(&self, layout: Layout) -> Size {
        match layout {
            Layout::Stack => self.stack,
            Layout::Grid => self.grid,
            Layout::Strip => self.strip,
        }
    }

    /// 本体のグラフ 1 枚の幅（CSS px）の見積もり。設定画面で、遅れの秒数を出すのに使う。
    pub fn graph_width(&self, layout: Layout, items: usize) -> f64 {
        // 枠 1px、パネルの外側の余白 6px、パネルどうしの間 4px（ui/monitor.css）
        let inner = self.size(layout).width - 2.0;
        let n = items.max(1) as f64;
        let width = match layout {
            Layout::Stack => inner - 12.0,
            Layout::Grid => (inner - 12.0 - 4.0) / 2.0,
            // 帯は左に 22px の印を置き、外側の余白は 4px
            Layout::Strip => (inner - 8.0 - 22.0 - 4.0 * n) / n,
        };
        // パネルの枠の 1px ずつを引く
        (width - 2.0).max(40.0)
    }

    pub fn set_size(&mut self, layout: Layout, size: Size) {
        let min = Self::min_size(layout);
        let size = Size::new(size.width.max(min.width), size.height.max(min.height));
        match layout {
            Layout::Stack => self.stack = size,
            Layout::Grid => self.grid = size,
            Layout::Strip => self.strip = size,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub settings: Settings,
    pub window: WindowState,
}

impl Config {
    pub fn load(path: &Path) -> Config {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Config::default();
        };
        match serde_json::from_str::<Config>(&text) {
            Ok(mut config) => {
                config.settings = config.settings.normalized();
                config
            }
            Err(err) => {
                // 読めない設定で上書きしないよう、退避してから既定値で始める
                eprintln!("config.json を読めませんでした: {err}");
                let _ = std::fs::rename(path, path.with_extension("json.broken"));
                Config::default()
            }
        }
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let text = serde_json::to_string_pretty(self)?;
        write_atomic(path, text.as_bytes())
    }
}

/// 同じディレクトリに一時ファイルを書いてから置き換える。
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".wezen-tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_fields_and_missing_fields_are_tolerated() {
        let config: Config =
            serde_json::from_str(r#"{"settings":{"intervalMs":1250,"future":1}}"#).unwrap();
        let settings = config.settings.normalized();
        assert_eq!(settings.interval_ms, 1500);
        assert_eq!(settings.layout, Layout::Stack);
        assert_eq!(config.window, WindowState::default());
    }

    #[test]
    fn old_ram_item_becomes_memory() {
        let items: Items = serde_json::from_str(r#"{"ram":false}"#).unwrap();
        assert!(!items.memory);
    }

    #[test]
    fn values_out_of_range_are_pulled_back() {
        let settings = Settings {
            interval_ms: 100,
            span_secs: 700,
            fill_opacity: 5.0,
            items: Items {
                cpu: false,
                memory: false,
                io: false,
                gpu: false,
                disk: false,
                net: false,
            },
            ..Settings::default()
        }
        .normalized();
        assert_eq!(settings.interval_ms, 500);
        assert_eq!(settings.span_secs, 600);
        assert_eq!(settings.fill_opacity, 0.8);
        assert_eq!(settings.items, Items::default());

        let settings = Settings {
            interval_ms: 90_000,
            ..Settings::default()
        }
        .normalized();
        assert_eq!(settings.interval_ms, 60_000);
    }

    #[test]
    fn window_size_keeps_the_minimum() {
        let mut window = WindowState::default();
        window.set_size(Layout::Strip, Size::new(100.0, 10.0));
        assert_eq!(
            window.size(Layout::Strip),
            WindowState::min_size(Layout::Strip)
        );
        assert_eq!(
            window.size(Layout::Stack),
            WindowState::default_size(Layout::Stack)
        );
    }
}
