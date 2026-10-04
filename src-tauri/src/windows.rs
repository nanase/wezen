//! 本体と設定の各ウィンドウ。

use crate::config::{Layout, WindowState};
use crate::state::{self, AppState};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{
    AppHandle, LogicalSize, Manager, Monitor, PhysicalPosition, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2Controller, ICoreWebView2Settings3,
};
use windows::core::Interface;

pub const MONITOR: &str = "monitor";
pub const SETTINGS: &str = "settings";

/// WebView2 に渡す引数。WebView2 はウィンドウどうしで 1 つの環境を共有するので、どのウィンドウにも同じものを渡す
/// （違うと 2 枚目を作れない）。
///
/// - `--disable-features=…`: wry の既定値。引数を渡すと wry は既定値を付けないので、ここで足す
/// - `--disable-gpu`: GPU のプロセスが確保するメモリ（約 85 MB）を減らす。描くのは小さなグラフだけなので、
///   CPU で描いても負荷は変わらなかった
const BROWSER_ARGS: &str =
    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --disable-gpu";

/// 最小化したウィンドウの位置。Windows はこの座標へ移したことにする
const MINIMIZED_POS: i32 = -30_000;

/// 本体ウィンドウを作り、保存した位置と大きさ、設定を当てて表示する。
///
/// 隠すときはウィンドウを閉じ、出すときに作り直す。閉じれば WebView2 のプロセスも終わり、
/// 隠している間のメモリをほぼ返せる。値の履歴は Rust 側で持っているので、作り直しても続きから描ける
pub fn create_monitor(app: &AppHandle) -> tauri::Result<()> {
    let size = WindowState::default_size(Layout::Stack);
    let win = WebviewWindowBuilder::new(app, MONITOR, WebviewUrl::App("monitor.html".into()))
        .title("Wezen")
        .theme(window_theme(app))
        .inner_size(size.width, size.height)
        .visible(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(true)
        .maximizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .additional_browser_args(BROWSER_ARGS)
        .build()?;
    apply_monitor_settings(app, true);
    restore_position(app, &win);
    disable_browser_keys(&win);

    let handle = app.clone();
    let minimized = AtomicBool::new(false);
    win.on_window_event(move |event| match event {
        WindowEvent::Moved(pos) => {
            if pos.x <= MINIMIZED_POS || pos.y <= MINIMIZED_POS {
                return;
            }
            handle.state::<AppState>().lock().window.position = Some((pos.x, pos.y));
            state::save_soon(&handle);
        }
        WindowEvent::Resized(size) => {
            let Some(win) = handle.get_webview_window(MONITOR) else {
                return;
            };
            // 最小化しただけでは WebView2 は描き続けるので、見えていないことを伝える
            let now_minimized = size.width == 0 || win.is_minimized().unwrap_or(false);
            if minimized.swap(now_minimized, Ordering::Relaxed) != now_minimized {
                set_webview_visible(&win, !now_minimized);
            }
            if now_minimized {
                return;
            }
            let scale = win.scale_factor().unwrap_or(1.0);
            let logical = size.to_logical::<f64>(scale);
            {
                let state = handle.state::<AppState>();
                let mut config = state.lock();
                let layout = config.settings.layout;
                config.window.set_size(
                    layout,
                    crate::config::Size {
                        width: logical.width,
                        height: logical.height,
                    },
                );
            }
            state::save_soon(&handle);
        }
        _ => {}
    });
    let _ = win.show();
    Ok(())
}

/// 設定を本体ウィンドウに当てる。`resize` が true なら、今のレイアウトで覚えている大きさにする。
pub fn apply_monitor_settings(app: &AppHandle, resize: bool) {
    let Some(win) = app.get_webview_window(MONITOR) else {
        return;
    };
    let (settings, size) = {
        let state = app.state::<AppState>();
        let config = state.lock();
        (
            config.settings.clone(),
            config.window.size(config.settings.layout),
        )
    };
    let min = WindowState::min_size(settings.layout);
    let _ = win.set_always_on_top(settings.topmost);
    let _ = win.set_skip_taskbar(!settings.taskbar);
    let _ = win.set_min_size(Some(LogicalSize::new(min.width, min.height)));
    if resize {
        let _ = win.set_size(LogicalSize::new(size.width, size.height));
    }
}

/// 覚えている位置へ戻す。その位置がどのディスプレイにも入らなければ、主ディスプレイの右下に置く。
fn restore_position(app: &AppHandle, win: &WebviewWindow) {
    let saved = app.state::<AppState>().lock().window.position;
    let monitors = app.available_monitors().unwrap_or_default();
    if let Some((x, y)) = saved {
        // 左上の角が少しでも見えていれば、つかんで戻せる
        let visible = monitors.iter().any(|m| {
            let area = m.work_area();
            let (left, top) = (area.position.x, area.position.y);
            let (right, bottom) = (left + area.size.width as i32, top + area.size.height as i32);
            (left..right - 40).contains(&x) && (top..bottom - 20).contains(&y)
        });
        if visible {
            let _ = win.set_position(PhysicalPosition::new(x, y));
            return;
        }
    }
    let monitor = app
        .primary_monitor()
        .ok()
        .flatten()
        .or_else(|| monitors.into_iter().next());
    if let Some(monitor) = monitor {
        place_bottom_right(win, &monitor);
    }
}

fn place_bottom_right(win: &WebviewWindow, monitor: &Monitor) {
    let area = monitor.work_area();
    let margin = (16.0 * monitor.scale_factor()).round() as i32;
    let Ok(size) = win.outer_size() else {
        return;
    };
    let x = area.position.x + area.size.width as i32 - size.width as i32 - margin;
    let y = area.position.y + area.size.height as i32 - size.height as i32 - margin;
    let _ = win.set_position(PhysicalPosition::new(
        x.max(area.position.x),
        y.max(area.position.y),
    ));
}

pub fn show_monitor(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(MONITOR) {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
        return;
    }
    // メニューのイベントの中（メインスレッド）でウィンドウを作ると固まるので、別のスレッドで作る
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(err) = create_monitor(&handle) {
            eprintln!("本体ウィンドウを開けませんでした: {err}");
        }
    });
}

/// 本体ウィンドウを閉じる。終了はしない（トレイのメニューか設定画面から）。
pub fn hide_monitor(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(MONITOR) {
        let _ = win.destroy();
    }
}

/// トレイアイコンのクリック。表示中なら隠し、隠れていれば表示する。
pub fn toggle_monitor(app: &AppHandle) {
    let shown = app
        .get_webview_window(MONITOR)
        .is_some_and(|win| !win.is_minimized().unwrap_or(false));
    if shown {
        hide_monitor(app);
    } else {
        show_monitor(app);
    }
}

/// 設定のテーマをウィンドウに反映する。None は Windows の設定に従う。
pub fn window_theme(app: &AppHandle) -> Option<tauri::Theme> {
    match app.state::<AppState>().lock().settings.theme {
        crate::config::Theme::System => None,
        crate::config::Theme::Dark => Some(tauri::Theme::Dark),
        crate::config::Theme::Light => Some(tauri::Theme::Light),
    }
}

pub fn apply_theme(app: &AppHandle) {
    let theme = window_theme(app);
    for label in [MONITOR, SETTINGS] {
        if let Some(win) = app.get_webview_window(label) {
            let _ = win.set_theme(theme);
        }
    }
}

pub fn set_settings_title(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(SETTINGS) {
        let lang = app.state::<AppState>().lang();
        let _ = win.set_title(crate::i18n::texts(lang).settings_title);
    }
}

/// 設定画面を開く。本体ウィンドウが出ていれば、そのディスプレイの中央に出す。
pub fn open_settings(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(SETTINGS) {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
        return;
    }
    let monitor = app
        .get_webview_window(MONITOR)
        .filter(|w| w.is_visible().unwrap_or(false))
        .and_then(|w| w.current_monitor().ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    let title = crate::i18n::texts(app.state::<AppState>().lang()).settings_title;
    let theme = window_theme(app);
    let handle = app.clone();
    // Windows では、メニューのイベントや同期コマンドの中（メインスレッド）でウィンドウを作ると
    // WebView2 の初期化を待ったまま固まる。別のスレッドで作る
    tauri::async_runtime::spawn(async move {
        let result =
            WebviewWindowBuilder::new(&handle, SETTINGS, WebviewUrl::App("settings.html".into()))
                .title(title)
                .additional_browser_args(BROWSER_ARGS)
                .inner_size(800.0, 600.0)
                .min_inner_size(640.0, 480.0)
                .theme(theme)
                .visible(false)
                .build();
        match result {
            Ok(win) => {
                disable_browser_keys(&win);
                match &monitor {
                    Some(monitor) => center_on(&win, monitor),
                    None => {
                        let _ = win.center();
                    }
                }
                let _ = win.show();
                let _ = win.set_focus();
            }
            Err(err) => eprintln!("設定画面を開けませんでした: {err}"),
        }
    });
}

/// WebView2 を直接操作する。失敗したら `what` を添えて知らせる。
fn with_controller<F>(win: &WebviewWindow, what: &'static str, f: F)
where
    F: FnOnce(ICoreWebView2Controller) -> windows::core::Result<()> + Send + 'static,
{
    let result = win.with_webview(move |webview| {
        if let Err(err) = f(webview.controller()) {
            eprintln!("{what}: {err}");
        }
    });
    if let Err(err) = result {
        eprintln!("{what}: {err}");
    }
}

/// WebView2 のブラウザー向けのキー操作（F5 の再読み込み、Ctrl+P の印刷、Ctrl+F の検索など）を止める。
/// デバッグビルドでは、F12 で開発者ツールを開けるよう残す
fn disable_browser_keys(win: &WebviewWindow) {
    if cfg!(debug_assertions) {
        return;
    }
    with_controller(
        win,
        "WebView2 のキー操作を止められませんでした",
        |controller| unsafe {
            controller
                .CoreWebView2()?
                .Settings()?
                .cast::<ICoreWebView2Settings3>()?
                .SetAreBrowserAcceleratorKeysEnabled(false)
        },
    );
}

/// ウィンドウが見えなくなったこと、また見えるようになったことを WebView2 に伝える。
/// 見えていない間は、画面の描画（requestAnimationFrame）が止まる
fn set_webview_visible(win: &WebviewWindow, visible: bool) {
    with_controller(
        win,
        "WebView2 に表示の状態を伝えられませんでした",
        move |controller| unsafe { controller.SetIsVisible(visible) },
    );
}

/// ウィンドウをディスプレイの作業領域の中央に置く。
///
/// 先に作業領域の左上へ移してから大きさを測る。拡大率の違うディスプレイへ移ると、Windows が大きさを変えるため
fn center_on(win: &WebviewWindow, monitor: &Monitor) {
    let area = monitor.work_area();
    let _ = win.set_position(area.position);
    let Ok(size) = win.outer_size() else {
        return;
    };
    let center = |start: i32, len: u32, win: u32| start + (len as i32 - win as i32).max(0) / 2;
    let _ = win.set_position(PhysicalPosition::new(
        center(area.position.x, area.size.width, size.width),
        center(area.position.y, area.size.height, size.height),
    ));
}
