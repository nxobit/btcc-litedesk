use crate::runtime_paths;
use gpui::*;
use gpui_component::{Theme, ThemeRegistry};
use serde::{Deserialize, Serialize};
use std::fs;

const DEFAULT_THEME: &str = "Ayu Light";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserSettings {
    #[serde(default = "default_theme_name")]
    theme: String,
    #[serde(default = "default_show_total_balance")]
    show_total_balance: bool,
    #[serde(default = "default_show_wallet_addresses")]
    show_wallet_addresses: bool,
    #[serde(default = "default_show_wallet_balances")]
    show_wallet_balances: bool,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            theme: default_theme_name(),
            show_total_balance: default_show_total_balance(),
            show_wallet_addresses: default_show_wallet_addresses(),
            show_wallet_balances: default_show_wallet_balances(),
        }
    }
}

fn default_theme_name() -> String {
    DEFAULT_THEME.to_string()
}

fn default_show_total_balance() -> bool {
    true
}

fn default_show_wallet_addresses() -> bool {
    true
}

fn default_show_wallet_balances() -> bool {
    true
}

pub fn init(cx: &mut App) {
    let themes_dir = runtime_paths::themes_dir();
    let theme_name = load_user_settings().theme;

    apply_theme(&theme_name, None, cx);
    if let Err(err) = ThemeRegistry::watch_dir(themes_dir, cx, |cx| {
        let theme_name = load_user_settings().theme;
        apply_theme(&theme_name, None, cx);
    }) {
        eprintln!("failed to watch desktop-gpui themes: {err}");
    }
}

pub fn theme_names(cx: &App) -> Vec<SharedString> {
    ThemeRegistry::global(cx)
        .sorted_themes()
        .into_iter()
        .map(|theme| theme.name.clone())
        .collect()
}

pub fn apply_theme(name: &str, window: Option<&mut Window>, cx: &mut App) {
    let config = ThemeRegistry::global(cx)
        .themes()
        .iter()
        .find(|(theme_name, _)| theme_name.to_string() == name)
        .map(|(_, config)| config.clone());

    if let Some(config) = config {
        Theme::global_mut(cx).apply_config(&config);
        if let Some(window) = window {
            window.refresh();
        } else {
            cx.refresh_windows();
        }
    }
}

pub fn apply_and_save_theme(name: &str, window: Option<&mut Window>, cx: &mut App) {
    let mut settings = load_user_settings();
    settings.theme = name.to_string();
    apply_theme(name, window, cx);
    if let Err(err) = save_user_settings(&settings) {
        eprintln!("failed to save desktop-gpui user settings: {err}");
    }
}

pub fn load_show_total_balance() -> bool {
    load_user_settings().show_total_balance
}

pub fn save_show_total_balance(show: bool) {
    let mut settings = load_user_settings();
    settings.show_total_balance = show;
    if let Err(err) = save_user_settings(&settings) {
        eprintln!("failed to save desktop-gpui user settings: {err}");
    }
}

pub fn load_show_wallet_addresses() -> bool {
    load_user_settings().show_wallet_addresses
}

pub fn save_show_wallet_addresses(show: bool) {
    let mut settings = load_user_settings();
    settings.show_wallet_addresses = show;
    if let Err(err) = save_user_settings(&settings) {
        eprintln!("failed to save desktop-gpui user settings: {err}");
    }
}

pub fn load_show_wallet_balances() -> bool {
    load_user_settings().show_wallet_balances
}

pub fn save_show_wallet_balances(show: bool) {
    let mut settings = load_user_settings();
    settings.show_wallet_balances = show;
    if let Err(err) = save_user_settings(&settings) {
        eprintln!("failed to save desktop-gpui user settings: {err}");
    }
}

fn load_user_settings() -> UserSettings {
    let path = runtime_paths::settings_path();
    if !path.exists() {
        let settings = UserSettings::default();
        if let Err(err) = save_user_settings(&settings) {
            eprintln!("failed to initialize desktop-gpui user settings: {err}");
        }
        return settings;
    }

    match fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<UserSettings>(&text).ok())
    {
        Some(settings) => settings,
        None => UserSettings::default(),
    }
}

fn save_user_settings(settings: &UserSettings) -> std::io::Result<()> {
    let path = runtime_paths::settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(settings).unwrap_or_else(|_| {
        format!(
            "{{\n  \"theme\": {}\n}}",
            serde_json::to_string(DEFAULT_THEME).unwrap_or_else(|_| "\"Ayu Light\"".to_string())
        )
    });
    fs::write(path, format!("{text}\n"))
}
