//! Wezen: Windows のリソースの使用状況を小さなグラフで表示する。

mod commands;
mod config;
mod i18n;
mod sampler;
mod state;
mod tray;
mod windows;

use state::AppState;
use tauri::{Manager, RunEvent};
use tauri_plugin_autostart::ManagerExt as _;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            windows::show_monitor(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let config_path = app.path().app_config_dir()?.join("config.json");
            app.manage(AppState::new(config_path));
            app.manage(sampler::Sampler::default());

            // スタートアップの登録は Windows の設定からも外せるので、実際の状態に合わせる
            if let Ok(enabled) = app.autolaunch().is_enabled() {
                app.state::<AppState>().lock().settings.autostart = enabled;
            }

            let handle = app.handle();
            windows::apply_theme(handle);
            windows::setup_monitor(handle);
            tray::create(handle)?;
            sampler::start(handle, Box::new(sampler::mock::Mock::new()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::get_history,
            commands::update_settings,
            commands::hide_monitor,
            commands::open_settings,
            commands::quit,
        ])
        .build(tauri::generate_context!())
        .expect("Wezen を起動できませんでした")
        .run(|_app, event| {
            // 設定画面を閉じても動き続ける。終了はトレイのメニューか設定画面から
            if let RunEvent::ExitRequested {
                api, code: None, ..
            } = event
            {
                api.prevent_exit();
            }
        });
}
