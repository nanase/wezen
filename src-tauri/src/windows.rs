//! 本体と設定の各ウィンドウ。

use crate::config::WindowState;
use crate::state::{self, AppState};
use tauri::{
    AppHandle, LogicalSize, Manager, Monitor, PhysicalPosition, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};

pub const MONITOR: &str = "monitor";
pub const SETTINGS: &str = "settings";

/// 最小化したウィンドウの位置。Windows はこの座標へ移したことにする
const MINIMIZED_POS: i32 = -30_000;

/// 起動時に、保存した位置と大きさ、設定を本体ウィンドウに当てて表示する。
pub fn setup_monitor(app: &AppHandle) {
    let Some(win) = app.get_webview_window(MONITOR) else {
        return;
    };
    apply_monitor_settings(app, true);
    restore_position(app, &win);
    disable_browser_keys(&win);

    let handle = app.clone();
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
            if size.width == 0 || win.is_minimized().unwrap_or(false) {
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
        // 閉じるボタンでは隠すだけにする。終了はトレイのメニューか設定画面から
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            hide_monitor(&handle);
        }
        _ => {}
    });
    let _ = win.show();
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
    }
}

pub fn hide_monitor(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(MONITOR) {
        let _ = win.hide();
    }
}

/// トレイアイコンのクリック。表示中なら隠し、隠れていれば表示する。
pub fn toggle_monitor(app: &AppHandle) {
    let Some(win) = app.get_webview_window(MONITOR) else {
        return;
    };
    let shown = win.is_visible().unwrap_or(false) && !win.is_minimized().unwrap_or(false);
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

/// WebView2 のブラウザー向けのキー操作（F5 の再読み込み、Ctrl+P の印刷、Ctrl+F の検索など）を止める。
/// デバッグビルドでは、F12 で開発者ツールを開けるよう残す
fn disable_browser_keys(win: &WebviewWindow) {
    if cfg!(debug_assertions) {
        return;
    }
    let result = win.with_webview(|webview| {
        use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3;
        use windows::core::Interface;
        let result = unsafe {
            webview
                .controller()
                .CoreWebView2()
                .and_then(|core| core.Settings())
                .and_then(|settings| settings.cast::<ICoreWebView2Settings3>())
                .and_then(|settings| settings.SetAreBrowserAcceleratorKeysEnabled(false))
        };
        if let Err(err) = result {
            eprintln!("WebView2 のキー操作を止められませんでした: {err}");
        }
    });
    if let Err(err) = result {
        eprintln!("WebView2 のキー操作を止められませんでした: {err}");
    }
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
