//! Wezen: Windows のリソースの使用状況を小さなグラフで表示する。

mod commands;
mod config;
mod i18n;
mod metrics;
mod sampler;
mod state;
mod tray;
mod windows;

use state::AppState;
use tauri::{Manager, RunEvent};
use tauri_plugin_autostart::ManagerExt as _;

/// 値の取り出し元。デバッグビルドでは `--mock` を付けると見本のデータにする。
fn source() -> Box<dyn sampler::Source> {
    #[cfg(debug_assertions)]
    if std::env::args().any(|a| a == "--mock") {
        return Box::new(sampler::mock::Mock::new());
    }
    Box::new(metrics::Collector::new())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // 動作確認用。デバッグビルドでは 2 つ目の起動に --debug-settings を付けると設定画面を開く
            #[cfg(debug_assertions)]
            if args.iter().any(|a| a == "--debug-settings") {
                windows::open_settings(app);
                return;
            }
            let _ = args;
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
            windows::create_monitor(handle)?;
            tray::create(handle)?;
            sampler::start(handle, source());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::get_history,
            commands::get_gpus,
            commands::update_settings,
            commands::hide_monitor,
            commands::open_settings,
            commands::open_repository,
            commands::quit,
        ])
        .build(tauri::generate_context!())
        .expect("Wezen を起動できませんでした")
        .run(|_app, event| {
            // ウィンドウをすべて閉じても動き続ける。終了はトレイのメニューか設定画面から
            if let RunEvent::ExitRequested {
                api, code: None, ..
            } = event
            {
                api.prevent_exit();
            }
        });
}
