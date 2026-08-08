use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::power::{PowerAction, SmartConditions};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub duration_input: String,
    pub action: PowerAction,
    #[serde(default)]
    pub conditions: SmartConditions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub at_unix: i64,
    pub action: PowerAction,
    pub action_label: String,
    pub duration_seconds: u64,
    pub duration_label: String,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub last_action: PowerAction,
    pub last_duration_input: String,
    pub presets: Vec<String>,
    pub minimize_to_tray: bool,
    pub launch_on_startup: bool,
    pub notify_before_seconds: u64,
    pub license_key: Option<String>,
    #[serde(default = "default_profiles")]
    pub profiles: Vec<Profile>,
    #[serde(default)]
    pub history: Vec<HistoryEntry>,
    #[serde(default = "default_true")]
    pub sound_enabled: bool,
    #[serde(default = "default_true")]
    pub notify_at_5m: bool,
    #[serde(default = "default_true")]
    pub notify_at_1m: bool,
    #[serde(default)]
    pub widget_enabled: bool,
    #[serde(default = "default_accent")]
    pub accent: String,
    #[serde(default = "default_hotkey_open")]
    pub hotkey_open: String,
    #[serde(default = "default_hotkey_cancel")]
    pub hotkey_cancel: String,
    #[serde(default = "default_true")]
    pub auto_check_updates: bool,
}

fn default_true() -> bool {
    true
}

fn default_accent() -> String {
    "#e2a84a".into()
}

fn default_hotkey_open() -> String {
    "CommandOrControl+Shift+A".into()
}

fn default_hotkey_cancel() -> String {
    "CommandOrControl+Shift+X".into()
}

fn default_profiles() -> Vec<Profile> {
    vec![
        Profile {
            id: "film".into(),
            name: "Fin de film".into(),
            duration_input: "2h".into(),
            action: PowerAction::Shutdown,
            conditions: SmartConditions::default(),
        },
        Profile {
            id: "sleep45".into(),
            name: "Je dors".into(),
            duration_input: "45m".into(),
            action: PowerAction::Sleep,
            conditions: SmartConditions::default(),
        },
        Profile {
            id: "download".into(),
            name: "Fin de download".into(),
            duration_input: "30m".into(),
            action: PowerAction::Shutdown,
            conditions: SmartConditions {
                cpu_below_percent: Some(15.0),
                cpu_for_seconds: Some(120),
                ..SmartConditions::default()
            },
        },
    ]
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            last_action: PowerAction::Shutdown,
            last_duration_input: "30m".into(),
            presets: vec![
                "15m".into(),
                "30m".into(),
                "1h".into(),
                "2h".into(),
            ],
            minimize_to_tray: true,
            launch_on_startup: false,
            notify_before_seconds: 60,
            license_key: None,
            profiles: default_profiles(),
            history: Vec::new(),
            sound_enabled: true,
            notify_at_5m: true,
            notify_at_1m: true,
            widget_enabled: false,
            accent: default_accent(),
            hotkey_open: default_hotkey_open(),
            hotkey_cancel: default_hotkey_cancel(),
            auto_check_updates: true,
        }
    }
}

fn settings_path() -> Result<PathBuf, String> {
    let dir = dirs::config_dir()
        .ok_or_else(|| "Dossier de config introuvable".to_string())?
        .join("AquertyStop");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("settings.json"))
}

pub fn load() -> AppSettings {
    let Ok(path) = settings_path() else {
        return AppSettings::default();
    };
    let Ok(raw) = fs::read_to_string(path) else {
        return AppSettings::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save(settings: &AppSettings) -> Result<(), String> {
    let path = settings_path()?;
    let raw = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(path, raw).map_err(|e| e.to_string())
}

pub fn push_history(settings: &mut AppSettings, entry: HistoryEntry) {
    settings.history.insert(0, entry);
    settings.history.truncate(30);
}
