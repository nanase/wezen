//! 画面から呼ぶコマンド。

use crate::config::Settings;
use crate::i18n::Lang;
use crate::metrics::gpu::Adapter;
use crate::sampler::{Sample, Sampler};
use crate::state::AppState;
use crate::{tray, windows};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_autostart::ManagerExt as _;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    settings: Settings,
    lang: Lang,
    version: &'static str,
}

fn view(state: &AppState) -> SettingsView {
    let settings = state.settings();
    SettingsView {
        lang: Lang::resolve(settings.language),
        settings,
        version: env!("CARGO_PKG_VERSION"),
    }
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> SettingsView {
    view(&state)
}

#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    sampler: State<'_, Sampler>,
    settings: Settings,
) -> SettingsView {
    let mut settings = settings.normalized();
    let old = state.settings();
    if settings.autostart != old.autostart {
        let autolaunch = app.autolaunch();
        let result = if settings.autostart {
            autolaunch.enable()
        } else {
            autolaunch.disable()
        };
        if let Err(err) = result {
            eprintln!("自動起動を切り替えられませんでした: {err}");
            settings.autostart = old.autostart;
        }
    }
    state.lock().settings = settings.clone();
    state.save();
    if settings.interval_ms != old.interval_ms {
        sampler.wake();
    }

    windows::apply_monitor_settings(&app, settings.layout != old.layout);
    if settings.theme != old.theme {
        windows::apply_theme(&app);
    }
    if settings.language != old.language {
        tray::refresh_menu(&app);
        windows::set_settings_title(&app);
    }
    let view = view(&state);
    let _ = app.emit("settings-changed", view.clone());
    view
}

#[tauri::command]
pub fn get_history(sampler: State<'_, Sampler>) -> Vec<Sample> {
    sampler.history()
}

#[tauri::command]
pub fn get_gpus() -> Vec<Adapter> {
    crate::metrics::gpu::adapters()
}

#[tauri::command]
pub fn hide_monitor(app: AppHandle) {
    windows::hide_monitor(&app);
}

#[tauri::command]
pub fn open_settings(app: AppHandle) {
    windows::open_settings(&app);
}

#[tauri::command]
pub fn quit(app: AppHandle) {
    app.exit(0);
}
