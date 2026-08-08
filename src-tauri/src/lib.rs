mod conditions;
mod i18n;
mod license;
mod power;
mod settings;
mod wake;

pub use license::{
    default_annual_expiry as license_default_annual_expiry,
    generate_annual as license_generate_annual, generate_lifetime as license_generate_lifetime,
};

use chrono::{Datelike, Local, TimeZone, Utc};
use conditions::ConditionTracker;
use license::LicenseInfo;
use parking_lot::Mutex;
use power::{PowerAction, SmartConditions};
use serde::{Deserialize, Serialize};
use settings::{AppSettings, HistoryEntry, Locale, RecurringRule};
use std::time::Duration;
use sysinfo::System;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::webview::WebviewWindowBuilder;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tauri_plugin_notification::NotificationExt;
use wake::WakeTimer;

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
    pub in_grace: bool,
    pub grace_remaining_seconds: u64,
    pub from_recurring: bool,
    pub next_recurring_unix: Option<i64>,
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
    grace_ends_at_unix: Option<i64>,
    notified_grace: bool,
    from_recurring_id: Option<String>,
}

struct AppStateInner {
    settings: AppSettings,
    schedule: Option<ActiveSchedule>,
    tracker: ConditionTracker,
    system: System,
    last_condition_status: Option<power::ConditionStatus>,
    wake: WakeTimer,
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
            wake: WakeTimer::new(),
        }))
    }
}

fn is_pro(settings: &AppSettings) -> bool {
    license::info_from_key(settings.license_key.as_deref()).is_pro
}

fn locale_of(settings: &AppSettings) -> Locale {
    settings.locale.clone()
}

fn ensure_allowed(
    action: PowerAction,
    conditions: &SmartConditions,
    pro: bool,
    locale: &Locale,
) -> Result<(), String> {
    if action.requires_pro() && !pro {
        return Err(i18n::msg(locale, "pro_power"));
    }
    if conditions.requires_pro() && !pro {
        return Err(i18n::msg(locale, "pro_conditions"));
    }
    Ok(())
}

fn next_occurrence_unix(rule: &RecurringRule, after_unix: i64) -> Option<i64> {
    if !rule.enabled {
        return None;
    }
    if rule.hour > 23 || rule.minute > 59 {
        return None;
    }
    if !rule.days.iter().any(|d| *d) {
        return None;
    }

    let after = Local.timestamp_opt(after_unix, 0).single()?;
    for offset in 0..8 {
        let day = after.date_naive() + chrono::Duration::days(offset);
        let weekday = day.weekday().num_days_from_monday() as usize;
        if weekday >= 7 || !rule.days[weekday] {
            continue;
        }
        let candidate = day
            .and_hms_opt(rule.hour, rule.minute, 0)
            .and_then(|naive| Local.from_local_datetime(&naive).single())?;
        if candidate.timestamp() > after_unix {
            return Some(candidate.timestamp());
        }
    }
    None
}

fn find_next_recurring(settings: &AppSettings, after_unix: i64) -> Option<(RecurringRule, i64)> {
    let mut best: Option<(RecurringRule, i64)> = None;
    for rule in &settings.recurring {
        if let Some(ts) = next_occurrence_unix(rule, after_unix) {
            match &best {
                None => best = Some((rule.clone(), ts)),
                Some((_, bts)) if ts < *bts => best = Some((rule.clone(), ts)),
                _ => {}
            }
        }
    }
    best
}

fn sync_wake(inner: &AppStateInner) {
    let pro = is_pro(&inner.settings);
    if !pro || !inner.settings.wake_to_execute {
        inner.wake.clear();
        return;
    }
    if let Some(schedule) = &inner.schedule {
        let target = schedule
            .grace_ends_at_unix
            .unwrap_or(schedule.ends_at_unix);
        inner.wake.arm_at_unix(target);
    } else if let Some((_, ts)) = find_next_recurring(&inner.settings, Utc::now().timestamp()) {
        inner.wake.arm_at_unix(ts);
    } else {
        inner.wake.clear();
    }
}

fn snapshot_from(inner: &AppStateInner) -> ScheduleSnapshot {
    let now = Utc::now().timestamp();
    let locale = locale_of(&inner.settings);
    let next_recurring_unix = find_next_recurring(&inner.settings, now).map(|(_, ts)| ts);

    match &inner.schedule {
        Some(s) => {
            let in_grace = s.grace_ends_at_unix.is_some();
            let grace_remaining = s
                .grace_ends_at_unix
                .map(|g| (g - now).max(0) as u64)
                .unwrap_or(0);
            let remaining = if s.waiting_for_conditions || in_grace {
                0
            } else {
                (s.ends_at_unix - now).max(0) as u64
            };
            ScheduleSnapshot {
                active: true,
                action: Some(s.action),
                action_label: Some(i18n::action_label(&locale, s.action).into()),
                remaining_seconds: remaining,
                total_seconds: s.total_seconds,
                ends_at_unix: Some(s.ends_at_unix),
                conditions: s.conditions.clone(),
                condition_status: inner.last_condition_status.clone(),
                waiting_for_conditions: s.waiting_for_conditions,
                in_grace,
                grace_remaining_seconds: grace_remaining,
                from_recurring: s.from_recurring_id.is_some(),
                next_recurring_unix,
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
            in_grace: false,
            grace_remaining_seconds: 0,
            from_recurring: false,
            next_recurring_unix,
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

fn arm_recurring_if_idle(inner: &mut AppStateInner) {
    arm_recurring_after(inner, Utc::now().timestamp());
}

fn arm_recurring_after(inner: &mut AppStateInner, after_unix: i64) {
    if inner.schedule.is_some() {
        return;
    }
    let Some((rule, ends_at)) = find_next_recurring(&inner.settings, after_unix) else {
        sync_wake(inner);
        return;
    };
    let now = Utc::now().timestamp();
    let total = (ends_at - now).max(1) as u64;
    inner.tracker = ConditionTracker::default();
    inner.last_condition_status = None;
    inner.schedule = Some(ActiveSchedule {
        action: rule.action,
        total_seconds: total,
        ends_at_unix: ends_at,
        conditions: SmartConditions::default(),
        notified_5m: false,
        notified_1m: false,
        notified_soon: false,
        waiting_for_conditions: false,
        grace_ends_at_unix: None,
        notified_grace: false,
        from_recurring_id: Some(rule.id),
    });
    sync_wake(inner);
}

fn do_cancel(app: &AppHandle, state: &AppState) -> ScheduleSnapshot {
    power::cancel_windows_shutdown();
    let mut inner = state.0.lock();
    let locale = locale_of(&inner.settings);
    let mut skip_recurring_after: Option<i64> = None;
    if let Some(schedule) = inner.schedule.take() {
        if schedule.from_recurring_id.is_some() {
            skip_recurring_after = Some(schedule.ends_at_unix);
        }
        settings::push_history(
            &mut inner.settings,
            HistoryEntry {
                at_unix: Utc::now().timestamp(),
                action: schedule.action,
                action_label: i18n::action_label(&locale, schedule.action).into(),
                duration_seconds: schedule.total_seconds,
                duration_label: format_duration_label(schedule.total_seconds),
                cancelled: true,
            },
        );
        let _ = settings::save(&inner.settings);
    }
    inner.last_condition_status = None;
    inner.tracker = ConditionTracker::default();
    let after = skip_recurring_after.unwrap_or_else(|| Utc::now().timestamp());
    arm_recurring_after(&mut inner, after);
    let snap = snapshot_from(&inner);
    sync_widget(app, &inner.settings, &snap);
    let _ = app.emit("schedule-updated", &snap);
    let _ = app.emit("settings-updated", &inner.settings);
    snap
}

fn validate_recurring(settings: &mut AppSettings, pro: bool, locale: &Locale) -> Result<(), String> {
    for rule in &mut settings.recurring {
        if rule.hour > 23 {
            rule.hour = 23;
        }
        if rule.minute > 59 {
            rule.minute = 59;
        }
    }
    if !pro && settings.recurring.len() > 1 {
        return Err(i18n::msg(locale, "pro_recurring"));
    }
    if !pro {
        settings.wake_to_execute = false;
    }
    Ok(())
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
    if new_settings.history.is_empty() && !inner.settings.history.is_empty() {
        new_settings.history = inner.settings.history.clone();
    }
    let pro = is_pro(&new_settings);
    let locale = locale_of(&new_settings);
    if !pro && new_settings.presets.len() > 4 {
        new_settings.presets.truncate(4);
    }
    if !pro && new_settings.profiles.len() > 3 {
        new_settings.profiles.truncate(3);
    }
    validate_recurring(&mut new_settings, pro, &locale)?;
    if new_settings.grace_seconds == 0 {
        new_settings.grace_seconds = 1;
    }
    if new_settings.grace_seconds > 600 {
        new_settings.grace_seconds = 600;
    }

    let recurring_changed = new_settings.recurring != inner.settings.recurring
        || new_settings.wake_to_execute != inner.settings.wake_to_execute;

    settings::save(&new_settings)?;
    inner.settings = new_settings.clone();

    if recurring_changed {
        let only_recurring = inner
            .schedule
            .as_ref()
            .map(|s| s.from_recurring_id.is_some())
            .unwrap_or(true);
        if only_recurring {
            // Drop armed recurring so it re-arms with new rules
            if inner
                .schedule
                .as_ref()
                .map(|s| s.from_recurring_id.is_some())
                .unwrap_or(false)
            {
                inner.schedule = None;
            }
            arm_recurring_if_idle(&mut inner);
        } else {
            sync_wake(&inner);
        }
    } else {
        sync_wake(&inner);
    }

    let snap = snapshot_from(&inner);
    sync_widget(&app, &inner.settings, &snap);
    let _ = app.emit("schedule-updated", &snap);
    let _ = app.emit("settings-updated", &new_settings);
    Ok(new_settings)
}

#[tauri::command]
fn get_license(app: AppHandle, state: State<'_, AppState>) -> LicenseInfo {
    let mut inner = state.0.lock();
    let info = license::info_from_key(inner.settings.license_key.as_deref());
    // Revoke stored demo / expired / invalid keys (e.g. after 1.1.2)
    if inner.settings.license_key.is_some() && !info.is_pro {
        inner.settings.license_key = None;
        if inner.settings.recurring.len() > 1 {
            inner.settings.recurring.truncate(1);
        }
        inner.settings.wake_to_execute = false;
        let _ = settings::save(&inner.settings);
        let _ = app.emit("settings-updated", &inner.settings);
        return license::info_from_key(None);
    }
    info
}

#[tauri::command]
fn activate_license(state: State<'_, AppState>, key: String) -> Result<LicenseInfo, String> {
    let locale = locale_of(&state.0.lock().settings);
    if !license::validate_key(&key) {
        return Err(i18n::msg(&locale, "invalid_license"));
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
    // Free: keep at most one recurring rule, disable wake
    if inner.settings.recurring.len() > 1 {
        inner.settings.recurring.truncate(1);
    }
    inner.settings.wake_to_execute = false;
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
    let mut inner = state.0.lock();
    let locale = locale_of(&inner.settings);
    if request.seconds == 0 && conditions.is_empty() {
        return Err(i18n::msg(&locale, "delay_or_condition"));
    }

    let pro = is_pro(&inner.settings);
    ensure_allowed(request.action, &conditions, pro, &locale)?;

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
        grace_ends_at_unix: None,
        notified_grace: false,
        from_recurring_id: None,
    });
    inner.settings.last_action = request.action;
    let _ = settings::save(&inner.settings);
    sync_wake(&inner);

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
fn parse_duration(state: State<'_, AppState>, input: String) -> Result<u64, String> {
    let locale = locale_of(&state.0.lock().settings);
    parse_duration_inner(&input, &locale)
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

fn parse_duration_inner(input: &str, locale: &Locale) -> Result<u64, String> {
    let temps = input.trim().to_lowercase().replace(' ', "");
    if temps.is_empty() {
        return Err(i18n::msg(locale, "empty_time"));
    }

    if temps.chars().all(|c| c.is_ascii_digit()) {
        let minutes: u64 = temps
            .parse()
            .map_err(|_| i18n::msg(locale, "invalid_number"))?;
        let total = minutes.saturating_mul(60);
        if total == 0 {
            return Err(i18n::msg(locale, "time_gt_zero"));
        }
        return Ok(total);
    }

    let re = regex_lite_duration(&temps, locale)?;
    if re == 0 {
        return Err(i18n::msg(locale, "time_gt_zero"));
    }
    Ok(re)
}

fn regex_lite_duration(temps: &str, locale: &Locale) -> Result<u64, String> {
    if !temps
        .chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false)
    {
        return Err(i18n::msg(locale, "invalid_format"));
    }

    let mut rest = temps;
    let mut hours: u64 = 0;
    let mut minutes: u64 = 0;
    let mut seconds: u64 = 0;
    let mut saw_unit = false;

    if let Some(idx) = rest.find('h') {
        let (num, after) = rest.split_at(idx);
        if num.is_empty() || !num.chars().all(|c| c.is_ascii_digit()) {
            return Err(i18n::msg(locale, "invalid_format"));
        }
        hours = num
            .parse()
            .map_err(|_| i18n::msg(locale, "invalid_number"))?;
        rest = &after[1..];
        saw_unit = true;
    }

    if let Some(idx) = rest.find('m') {
        let (num, after) = rest.split_at(idx);
        if num.is_empty() || !num.chars().all(|c| c.is_ascii_digit()) {
            return Err(i18n::msg(locale, "invalid_format"));
        }
        minutes = num
            .parse()
            .map_err(|_| i18n::msg(locale, "invalid_number"))?;
        rest = &after[1..];
        saw_unit = true;
    }

    if let Some(idx) = rest.find('s') {
        let (num, after) = rest.split_at(idx);
        if num.is_empty() || !num.chars().all(|c| c.is_ascii_digit()) {
            return Err(i18n::msg(locale, "invalid_format"));
        }
        seconds = num
            .parse()
            .map_err(|_| i18n::msg(locale, "invalid_number"))?;
        rest = &after[1..];
        saw_unit = true;
    }

    if !rest.is_empty() || !saw_unit {
        return Err(i18n::msg(locale, "invalid_format"));
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
    let mut fire_action: Option<(PowerAction, u64, Option<String>)> = None;
    let mut alerts: Vec<(String, u64, bool, String)> = Vec::new();
    let snap: ScheduleSnapshot;
    let settings_snapshot: AppSettings;

    {
        let mut inner = state.0.lock();
        if inner.schedule.is_none() {
            arm_recurring_if_idle(&mut inner);
        }

        let Some(schedule) = inner.schedule.clone() else {
            let snap_idle = snapshot_from(&inner);
            sync_widget(app, &inner.settings, &snap_idle);
            let _ = app.emit("schedule-updated", &snap_idle);
            return;
        };

        let now = Utc::now().timestamp();
        let notify_before = inner.settings.notify_before_seconds.max(1);
        let conditions = schedule.conditions.clone();
        let sound = inner.settings.sound_enabled;
        let notify_5m = inner.settings.notify_at_5m;
        let notify_1m = inner.settings.notify_at_1m;
        let grace_enabled = inner.settings.grace_enabled;
        let grace_seconds = inner.settings.grace_seconds.max(1);
        let locale = locale_of(&inner.settings);

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
        let in_grace = schedule.grace_ends_at_unix.is_some();

        if !in_grace
            && notify_5m
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
                i18n::msg(&locale, "alert_5m"),
            ));
        }

        if !in_grace
            && notify_1m
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
                i18n::msg(&locale, "alert_1m"),
            ));
        } else if !in_grace
            && !schedule.notified_soon
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
                i18n::msg(&locale, "alert_soon"),
            ));
        }

        if timer_done && !conditions_ok {
            if let Some(s) = inner.schedule.as_mut() {
                s.waiting_for_conditions = true;
            }
        }

        if timer_done && conditions_ok {
            if let Some(grace_end) = schedule.grace_ends_at_unix {
                if now >= grace_end {
                    fire_action = Some((
                        schedule.action,
                        schedule.total_seconds,
                        schedule.from_recurring_id.clone(),
                    ));
                    inner.schedule = None;
                }
            } else if grace_enabled {
                if let Some(s) = inner.schedule.as_mut() {
                    s.grace_ends_at_unix = Some(now + grace_seconds as i64);
                    s.waiting_for_conditions = false;
                    if !s.notified_grace {
                        s.notified_grace = true;
                        alerts.push((
                            "grace".into(),
                            grace_seconds,
                            sound,
                            i18n::msg(&locale, "alert_grace"),
                        ));
                    }
                }
                sync_wake(&inner);
            } else {
                fire_action = Some((
                    schedule.action,
                    schedule.total_seconds,
                    schedule.from_recurring_id.clone(),
                ));
                inner.schedule = None;
            }
        }

        snap = snapshot_from(&inner);
        settings_snapshot = inner.settings.clone();
    }

    let locale = locale_of(&settings_snapshot);
    let title = if snap.active {
        if snap.in_grace {
            i18n::msg(&locale, "grace_title")
        } else if snap.waiting_for_conditions {
            i18n::msg(&locale, "waiting")
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
        let label = if snap.in_grace {
            format_duration_label(snap.grace_remaining_seconds)
        } else {
            format_duration_label(snap.remaining_seconds)
        };
        let _ = widget.set_title(&label);
    }
    sync_widget(app, &settings_snapshot, &snap);
    let _ = app.emit("schedule-updated", &snap);

    for (stage, remaining, sound, body) in alerts {
        emit_alert(app, &stage, remaining, sound, &body);
    }

    if let Some((action, total_seconds, recurring_id)) = fire_action {
        {
            let mut inner = state.0.lock();
            let loc = locale_of(&inner.settings);
            let fired_ends = Utc::now().timestamp();
            settings::push_history(
                &mut inner.settings,
                HistoryEntry {
                    at_unix: fired_ends,
                    action,
                    action_label: i18n::action_label(&loc, action).into(),
                    duration_seconds: total_seconds,
                    duration_label: format_duration_label(total_seconds),
                    cancelled: false,
                },
            );
            let _ = settings::save(&inner.settings);
            let _ = app.emit("settings-updated", &inner.settings);
            if recurring_id.is_some() {
                arm_recurring_after(&mut inner, fired_ends);
            } else {
                arm_recurring_if_idle(&mut inner);
            }
            let snap_after = snapshot_from(&inner);
            sync_widget(app, &inner.settings, &snap_after);
            let _ = app.emit("schedule-updated", &snap_after);
        }
        let _ = app
            .notification()
            .builder()
            .title("Aquerty Stop")
            .body(i18n::action_in_progress(&locale, action))
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

            {
                let state = app.state::<AppState>();
                let mut inner = state.0.lock();
                arm_recurring_if_idle(&mut inner);
            }

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
