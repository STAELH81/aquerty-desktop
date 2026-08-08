mod conditions;
mod license;
mod power;
mod settings;

use chrono::Utc;
use conditions::ConditionTracker;
use license::LicenseInfo;
use parking_lot::Mutex;
use power::{PowerAction, SmartConditions};
use serde::{Deserialize, Serialize};
use settings::{AppSettings, HistoryEntry};
use std::time::Duration;
use sysinfo::System;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::webview::WebviewWindowBuilder;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tauri_plugin_notification::NotificationExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleSnapshot {
    pub active: bool,
    pub action: Option<PowerAction>,
    pub action_label: Option<String>,
    pub remaining_seconds: u64,
    pub total_seconds: u64,
    pub ends_at_unix: Option<i64>,
    pub conditions: SmartConditions,
    pub condition_status: Option<power::ConditionStatus>,
    pub waiting_for_conditions: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertPayload {
    pub stage: String,
    pub remaining_seconds: u64,
    pub sound: bool,
}

#[derive(Debug, Clone)]
struct ActiveSchedule {
    action: PowerAction,
    total_seconds: u64,
    ends_at_unix: i64,
    conditions: SmartConditions,
    notified_5m: bool,
    notified_1m: bool,
    notified_soon: bool,
    waiting_for_conditions: bool,
}

struct AppStateInner {
    settings: AppSettings,
    schedule: Option<ActiveSchedule>,
    tracker: ConditionTracker,
    system: System,
    last_condition_status: Option<power::ConditionStatus>,
}

pub struct AppState(Mutex<AppStateInner>);

impl AppState {
    fn new() -> Self {
        let settings = settings::load();
        Self(Mutex::new(AppStateInner {
            settings,
            schedule: None,
            tracker: ConditionTracker::default(),
            system: System::new_all(),
            last_condition_status: None,
        }))
    }
}

fn is_pro(settings: &AppSettings) -> bool {
    license::info_from_key(settings.license_key.as_deref()).is_pro
}

fn ensure_allowed(action: PowerAction, conditions: &SmartConditions, pro: bool) -> Result<(), String> {
    if action.requires_pro() && !pro {
        return Err(
            "Fonction Pro : veille, hibernation et verrouillage."
                .into(),
        );
    }
    if conditions.requires_pro() && !pro {
        return Err(
            "Fonction Pro : conditions réservées à la licence Pro.".into(),
        );
    }
    Ok(())
}

fn snapshot_from(inner: &AppStateInner) -> ScheduleSnapshot {
    let now = Utc::now().timestamp();
    match &inner.schedule {
        Some(s) => {
            let remaining = if s.waiting_for_conditions {
                0
            } else {
                (s.ends_at_unix - now).max(0) as u64
            };
            ScheduleSnapshot {
                active: true,
                action: Some(s.action),
                action_label: Some(s.action.label().into()),
                remaining_seconds: remaining,
                total_seconds: s.total_seconds,
                ends_at_unix: Some(s.ends_at_unix),
                conditions: s.conditions.clone(),
                condition_status: inner.last_condition_status.clone(),
                waiting_for_conditions: s.waiting_for_conditions,
            }
        }
        None => ScheduleSnapshot {
            active: false,
            action: None,
            action_label: None,
            remaining_seconds: 0,
            total_seconds: 0,
            ends_at_unix: None,
            conditions: SmartConditions::default(),
            condition_status: None,
            waiting_for_conditions: false,
        },
    }
}

fn format_duration_label(seconds: u64) -> String {
    let h = seconds / 3600;
    let m = (seconds % 3600) / 60;
    let s = seconds % 60;
    let mut parts = Vec::new();
    if h > 0 {
        parts.push(format!("{h}h"));
    }
    if m > 0 {
        parts.push(format!("{m}m"));
    }
    if s > 0 || parts.is_empty() {
        parts.push(format!("{s}s"));
    }
    parts.join("")
}

fn sync_widget(app: &AppHandle, settings: &AppSettings, snap: &ScheduleSnapshot) {
    let Some(widget) = app.get_webview_window("widget") else {
        return;
    };
    let should_show = settings.widget_enabled && snap.active;
    if should_show {
        let _ = widget.show();
    } else {
        let _ = widget.hide();
    }
}

fn do_cancel(app: &AppHandle, state: &AppState) -> ScheduleSnapshot {
    power::cancel_windows_shutdown();
    let mut inner = state.0.lock();
    if let Some(schedule) = inner.schedule.take() {
        let elapsed = schedule
            .total_seconds
            .saturating_sub((schedule.ends_at_unix - Utc::now().timestamp()).max(0) as u64);
        settings::push_history(
            &mut inner.settings,
            HistoryEntry {
                at_unix: Utc::now().timestamp(),
                action: schedule.action,
                action_label: schedule.action.label().into(),
                duration_seconds: schedule.total_seconds,
                duration_label: format_duration_label(schedule.total_seconds),
                cancelled: true,
            },
        );
        let _ = elapsed;
        let _ = settings::save(&inner.settings);
    }
    inner.last_condition_status = None;
    inner.tracker = ConditionTracker::default();
    let snap = snapshot_from(&inner);
    sync_widget(app, &inner.settings, &snap);
    let _ = app.emit("schedule-updated", &snap);
    let _ = app.emit("settings-updated", &inner.settings);
    snap
}

#[tauri::command]
fn get_schedule(state: State<'_, AppState>) -> ScheduleSnapshot {
    snapshot_from(&state.0.lock())
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> AppSettings {
    state.0.lock().settings.clone()
}

#[tauri::command]
fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    mut new_settings: AppSettings,
) -> Result<AppSettings, String> {
    let mut inner = state.0.lock();
    if new_settings.license_key.is_none() {
        new_settings.license_key = inner.settings.license_key.clone();
    }
    // Preserve history from server state if client sends empty accidentally
    if new_settings.history.is_empty() && !inner.settings.history.is_empty() {
        new_settings.history = inner.settings.history.clone();
    }
    if !is_pro(&new_settings) && new_settings.presets.len() > 4 {
        new_settings.presets.truncate(4);
    }
    if !is_pro(&new_settings) && new_settings.profiles.len() > 3 {
        new_settings.profiles.truncate(3);
    }
    settings::save(&new_settings)?;
    inner.settings = new_settings.clone();
    let snap = snapshot_from(&inner);
    sync_widget(&app, &inner.settings, &snap);
    let _ = app.emit("settings-updated", &new_settings);
    Ok(new_settings)
}

#[tauri::command]
fn get_license(state: State<'_, AppState>) -> LicenseInfo {
    let inner = state.0.lock();
    license::info_from_key(inner.settings.license_key.as_deref())
}

#[tauri::command]
fn activate_license(state: State<'_, AppState>, key: String) -> Result<LicenseInfo, String> {
    if !license::validate_key(&key) {
        return Err("Clé de licence invalide.".into());
    }
    let mut inner = state.0.lock();
    inner.settings.license_key = Some(key.trim().to_uppercase());
    settings::save(&inner.settings)?;
    Ok(license::info_from_key(inner.settings.license_key.as_deref()))
}

#[tauri::command]
fn deactivate_license(state: State<'_, AppState>) -> Result<LicenseInfo, String> {
    let mut inner = state.0.lock();
    inner.settings.license_key = None;
    settings::save(&inner.settings)?;
    Ok(license::info_from_key(None))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScheduleRequest {
    action: PowerAction,
    seconds: u64,
    conditions: Option<SmartConditions>,
}

#[tauri::command]
fn schedule_power(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ScheduleRequest,
) -> Result<ScheduleSnapshot, String> {
    let conditions = request.conditions.unwrap_or_default();
    if request.seconds == 0 && conditions.is_empty() {
        return Err("Le délai doit être supérieur à 0, ou ajoutez une condition.".into());
    }

    let mut inner = state.0.lock();
    let pro = is_pro(&inner.settings);
    ensure_allowed(request.action, &conditions, pro)?;

    let now = Utc::now().timestamp();
    let total = if request.seconds == 0 {
        1
    } else {
        request.seconds
    };
    inner.tracker = ConditionTracker::default();
    inner.schedule = Some(ActiveSchedule {
        action: request.action,
        total_seconds: total,
        ends_at_unix: now + total as i64,
        conditions,
        notified_5m: false,
        notified_1m: false,
        notified_soon: false,
        waiting_for_conditions: false,
    });
    inner.settings.last_action = request.action;
    let _ = settings::save(&inner.settings);

    let snap = snapshot_from(&inner);
    sync_widget(&app, &inner.settings, &snap);
    let _ = app.emit("schedule-updated", &snap);
    Ok(snap)
}

#[tauri::command]
fn cancel_schedule(app: AppHandle, state: State<'_, AppState>) -> Result<ScheduleSnapshot, String> {
    Ok(do_cancel(&app, state.inner()))
}

#[tauri::command]
fn clear_history(app: AppHandle, state: State<'_, AppState>) -> Result<AppSettings, String> {
    let mut inner = state.0.lock();
    inner.settings.history.clear();
    settings::save(&inner.settings)?;
    let _ = app.emit("settings-updated", &inner.settings);
    Ok(inner.settings.clone())
}

#[tauri::command]
fn list_processes(state: State<'_, AppState>) -> Vec<String> {
    let mut inner = state.0.lock();
    conditions::list_process_names(&mut inner.system, 120)
}

#[tauri::command]
fn parse_duration(input: String) -> Result<u64, String> {
    parse_duration_inner(&input)
}

#[tauri::command]
fn set_widget_visible(app: AppHandle, state: State<'_, AppState>, visible: bool) -> Result<(), String> {
    let mut inner = state.0.lock();
    inner.settings.widget_enabled = visible;
    settings::save(&inner.settings)?;
    let snap = snapshot_from(&inner);
    sync_widget(&app, &inner.settings, &snap);
    let _ = app.emit("settings-updated", &inner.settings);
    Ok(())
}

fn parse_duration_inner(input: &str) -> Result<u64, String> {
    let temps = input.trim().to_lowercase().replace(' ', "");
    if temps.is_empty() {
        return Err("Temps vide.".into());
    }

    if temps.chars().all(|c| c.is_ascii_digit()) {
        let minutes: u64 = temps.parse().map_err(|_| "Nombre invalide".to_string())?;
        let total = minutes.saturating_mul(60);
        if total == 0 {
            return Err("Le temps doit être supérieur à 0.".into());
        }
        return Ok(total);
    }

    let re = regex_lite_duration(&temps)?;
    if re == 0 {
        return Err("Le temps doit être supérieur à 0.".into());
    }
    Ok(re)
}

fn regex_lite_duration(temps: &str) -> Result<u64, String> {
    if !temps
        .chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false)
    {
        return Err("Format invalide. Exemples : 30m, 1h20m, 2h30m15s".into());
    }

    let mut rest = temps;
    let mut hours: u64 = 0;
    let mut minutes: u64 = 0;
    let mut seconds: u64 = 0;
    let mut saw_unit = false;

    if let Some(idx) = rest.find('h') {
        let (num, after) = rest.split_at(idx);
        if num.is_empty() || !num.chars().all(|c| c.is_ascii_digit()) {
            return Err("Format invalide.".into());
        }
        hours = num.parse().map_err(|_| "Heures invalides".to_string())?;
        rest = &after[1..];
        saw_unit = true;
    }

    if let Some(idx) = rest.find('m') {
        let (num, after) = rest.split_at(idx);
        if num.is_empty() || !num.chars().all(|c| c.is_ascii_digit()) {
            return Err("Format invalide.".into());
        }
        minutes = num.parse().map_err(|_| "Minutes invalides".to_string())?;
        rest = &after[1..];
        saw_unit = true;
    }

    if let Some(idx) = rest.find('s') {
        let (num, after) = rest.split_at(idx);
        if num.is_empty() || !num.chars().all(|c| c.is_ascii_digit()) {
            return Err("Format invalide.".into());
        }
        seconds = num.parse().map_err(|_| "Secondes invalides".to_string())?;
        rest = &after[1..];
        saw_unit = true;
    }

    if !rest.is_empty() || !saw_unit {
        return Err("Format invalide. Exemples : 30m, 1h20m, 2h30m15s".into());
    }

    Ok(hours
        .saturating_mul(3600)
        .saturating_add(minutes.saturating_mul(60))
        .saturating_add(seconds))
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn emit_alert(app: &AppHandle, stage: &str, remaining: u64, sound: bool, body: &str) {
    let _ = app
        .notification()
        .builder()
        .title("Aquerty Stop")
        .body(body)
        .show();
    let _ = app.emit(
        "timer-alert",
        AlertPayload {
            stage: stage.into(),
            remaining_seconds: remaining,
            sound,
        },
    );
}

fn tick(app: &AppHandle, state: &AppState) {
    let mut fire_action: Option<(PowerAction, u64)> = None;
    let mut alerts: Vec<(String, u64, bool, String)> = Vec::new();
    let snap: ScheduleSnapshot;
    let settings_snapshot: AppSettings;

    {
        let mut inner = state.0.lock();
        let Some(schedule) = inner.schedule.clone() else {
            return;
        };

        let now = Utc::now().timestamp();
        let notify_before = inner.settings.notify_before_seconds.max(1);
        let conditions = schedule.conditions.clone();
        let sound = inner.settings.sound_enabled;
        let notify_5m = inner.settings.notify_at_5m;
        let notify_1m = inner.settings.notify_at_1m;

        let status = if conditions.requires_pro() || !conditions.is_empty() {
            let AppStateInner {
                tracker, system, ..
            } = &mut *inner;
            Some(tracker.evaluate(system, &conditions, now))
        } else {
            None
        };
        inner.last_condition_status = status.clone();

        let remaining = (schedule.ends_at_unix - now).max(0) as u64;
        let timer_done = now >= schedule.ends_at_unix;
        let conditions_ok = status.as_ref().map(|s| s.all_met).unwrap_or(true);

        if notify_5m
            && !schedule.notified_5m
            && !schedule.waiting_for_conditions
            && remaining <= 300
            && remaining > 60
        {
            if let Some(s) = inner.schedule.as_mut() {
                s.notified_5m = true;
            }
            alerts.push((
                "5m".into(),
                remaining,
                sound,
                "Plus que 5 minutes avant l'action.".into(),
            ));
        }

        if notify_1m
            && !schedule.notified_1m
            && !schedule.waiting_for_conditions
            && remaining <= 60
            && remaining > 0
        {
            if let Some(s) = inner.schedule.as_mut() {
                s.notified_1m = true;
                s.notified_soon = true;
            }
            alerts.push((
                "1m".into(),
                remaining,
                sound,
                "Plus qu'une minute, action imminente.".into(),
            ));
        } else if !schedule.notified_soon
            && !schedule.waiting_for_conditions
            && remaining <= notify_before
            && remaining > 0
            && !notify_1m
        {
            if let Some(s) = inner.schedule.as_mut() {
                s.notified_soon = true;
            }
            alerts.push((
                "soon".into(),
                remaining,
                sound,
                "Action imminente.".into(),
            ));
        }

        if timer_done && !conditions_ok {
            if let Some(s) = inner.schedule.as_mut() {
                s.waiting_for_conditions = true;
            }
        }

        if timer_done && conditions_ok {
            fire_action = Some((schedule.action, schedule.total_seconds));
            inner.schedule = None;
        }

        snap = snapshot_from(&inner);
        settings_snapshot = inner.settings.clone();
    }

    let title = if snap.active {
        if snap.waiting_for_conditions {
            "Aquerty Stop - en attente".into()
        } else {
            let m = snap.remaining_seconds / 60;
            let s = snap.remaining_seconds % 60;
            format!("Aquerty Stop - {m:02}:{s:02}")
        }
    } else {
        "Aquerty Stop".into()
    };
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_title(&title);
    }
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_tooltip(Some(&title));
    }
    if let Some(widget) = app.get_webview_window("widget") {
        let _ = widget.set_title(&format_duration_label(snap.remaining_seconds));
    }
    sync_widget(app, &settings_snapshot, &snap);
    let _ = app.emit("schedule-updated", &snap);

    for (stage, remaining, sound, body) in alerts {
        emit_alert(app, &stage, remaining, sound, &body);
    }

    if let Some((action, total_seconds)) = fire_action {
        {
            let mut inner = state.0.lock();
            settings::push_history(
                &mut inner.settings,
                HistoryEntry {
                    at_unix: Utc::now().timestamp(),
                    action,
                    action_label: action.label().into(),
                    duration_seconds: total_seconds,
                    duration_label: format_duration_label(total_seconds),
                    cancelled: false,
                },
            );
            let _ = settings::save(&inner.settings);
            let _ = app.emit("settings-updated", &inner.settings);
        }
        let _ = app
            .notification()
            .builder()
            .title("Aquerty Stop")
            .body(format!("{} en cours…", action.label()))
            .show();
        let _ = app.emit(
            "timer-alert",
            AlertPayload {
                stage: "fire".into(),
                remaining_seconds: 0,
                sound: settings_snapshot.sound_enabled,
            },
        );
        let _ = power::execute(action);
    }
}

fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let show_i = MenuItem::with_id(app, "show", "Ouvrir", true, None::<&str>)?;
    let cancel_i = MenuItem::with_id(app, "cancel", "Annuler l'action", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "Quitter", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_i, &cancel_i, &quit_i])?;

    let icon = app
        .default_window_icon()
        .cloned()
        .expect("default window icon");

    let _tray = TrayIconBuilder::with_id("main")
        .icon(icon)
        .menu(&menu)
        .tooltip("Aquerty Stop")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "cancel" => {
                if let Some(state) = app.try_state::<AppState>() {
                    let _ = do_cancel(app, state.inner());
                }
            }
            "quit" => {
                power::cancel_windows_shutdown();
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

fn register_hotkeys(app: &AppHandle) -> Result<(), String> {
    let settings = app.state::<AppState>().0.lock().settings.clone();
    let open = settings.hotkey_open.clone();
    let cancel = settings.hotkey_cancel.clone();

    let _ = app.global_shortcut().unregister_all();

    let open_shortcut: Shortcut = open
        .parse()
        .map_err(|e| format!("Hotkey ouvrir invalide: {e}"))?;
    let cancel_shortcut: Shortcut = cancel
        .parse()
        .map_err(|e| format!("Hotkey annuler invalide: {e}"))?;

    app.global_shortcut()
        .on_shortcut(open_shortcut, |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                show_main_window(app);
            }
        })
        .map_err(|e| e.to_string())?;

    app.global_shortcut()
        .on_shortcut(cancel_shortcut, |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                if let Some(state) = app.try_state::<AppState>() {
                    let _ = do_cancel(app, state.inner());
                }
            }
        })
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
fn rebind_hotkeys(app: AppHandle) -> Result<(), String> {
    register_hotkeys(&app)
}

fn ensure_widget_window(app: &AppHandle) -> tauri::Result<()> {
    if app.get_webview_window("widget").is_some() {
        return Ok(());
    }
    let widget = WebviewWindowBuilder::new(app, "widget", WebviewUrl::App("widget.html".into()))
        .title("Aquerty")
        .inner_size(220.0, 96.0)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .build()?;
    let _ = widget;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            get_schedule,
            get_settings,
            save_settings,
            get_license,
            activate_license,
            deactivate_license,
            schedule_power,
            cancel_schedule,
            clear_history,
            list_processes,
            parse_duration,
            set_widget_visible,
            rebind_hotkeys,
        ])
        .setup(|app| {
            setup_tray(app.handle())?;
            ensure_widget_window(app.handle())?;
            let _ = register_hotkeys(app.handle());

            let handle_tick = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_secs(1));
                if let Some(state) = handle_tick.try_state::<AppState>() {
                    tick(&handle_tick, state.inner());
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "widget" {
                    api.prevent_close();
                    let _ = window.hide();
                    return;
                }
                if let Some(state) = window.app_handle().try_state::<AppState>() {
                    if state.0.lock().settings.minimize_to_tray {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Aquerty Stop");
}
