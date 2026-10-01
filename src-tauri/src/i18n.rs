//! Rust 側で出す文言（トレイのメニューとツールチップ、ウィンドウのタイトル）。画面の文言は ui/i18n.js にある。

use crate::config::Language;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Lang {
    Ja,
    En,
}

impl Lang {
    /// 設定で選んだ言語。「システムに合わせる」なら Windows の表示言語に従う。
    pub fn resolve(setting: Language) -> Lang {
        match setting {
            Language::Ja => Lang::Ja,
            Language::En => Lang::En,
            Language::Auto => match sys_locale::get_locale() {
                Some(locale) if locale.to_lowercase().starts_with("ja") => Lang::Ja,
                _ => Lang::En,
            },
        }
    }
}

pub struct Texts {
    pub show: &'static str,
    pub settings: &'static str,
    pub quit: &'static str,
    pub settings_title: &'static str,
}

pub fn texts(lang: Lang) -> &'static Texts {
    match lang {
        Lang::Ja => &Texts {
            show: "Wezen を表示",
            settings: "設定",
            quit: "Wezen を終了",
            settings_title: "Wezen の設定",
        },
        Lang::En => &Texts {
            show: "Show Wezen",
            settings: "Settings",
            quit: "Quit Wezen",
            settings_title: "Wezen Settings",
        },
    }
}
