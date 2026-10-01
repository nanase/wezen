//! トレイアイコン。アプリのアイコンをそのまま出す。

use crate::i18n;
use crate::state::AppState;
use crate::windows;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

const TRAY_ID: &str = "wezen";

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Wezen")
        .menu(&menu(app)?)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => windows::show_monitor(app),
            "settings" => windows::open_settings(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                windows::toggle_monitor(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

/// 言語を変えたときに、メニューを作り直す。
pub fn refresh_menu(app: &AppHandle) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    match menu(app) {
        Ok(menu) => {
            let _ = tray.set_menu(Some(menu));
        }
        Err(err) => eprintln!("トレイのメニューを作れませんでした: {err}"),
    }
}

fn menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let t = i18n::texts(app.state::<AppState>().lang());
    let show = MenuItem::with_id(app, "show", t.show, true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", t.settings, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", t.quit, true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    Menu::with_items(app, &[&show, &settings, &separator, &quit])
}
