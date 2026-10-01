//! アプリ全体の状態。設定と、その保存。

use crate::config::{Config, Settings};
use crate::i18n::Lang;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;
use tauri::{AppHandle, Manager};

pub struct AppState {
    config: Mutex<Config>,
    config_path: PathBuf,
    /// 保存を待っているか。ウィンドウを動かしている間は何度も変わるので、まとめて保存する
    save_pending: AtomicBool,
}

impl AppState {
    pub fn new(config_path: PathBuf) -> Self {
        Self {
            config: Mutex::new(Config::load(&config_path)),
            config_path,
            save_pending: AtomicBool::new(false),
        }
    }

    pub fn lock(&self) -> MutexGuard<'_, Config> {
        self.config.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn settings(&self) -> Settings {
        self.lock().settings.clone()
    }

    pub fn lang(&self) -> Lang {
        Lang::resolve(self.lock().settings.language)
    }

    pub fn save(&self) {
        let config = self.lock().clone();
        if let Err(err) = config.save(&self.config_path) {
            eprintln!("設定を保存できませんでした: {err}");
        }
    }
}

/// 少し待ってから保存する。待っている間の変更は 1 回の保存にまとめる。
pub fn save_soon(app: &AppHandle) {
    if app
        .state::<AppState>()
        .save_pending
        .swap(true, Ordering::AcqRel)
    {
        return;
    }
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(600));
        let state = handle.state::<AppState>();
        state.save_pending.store(false, Ordering::Release);
        state.save();
    });
}
