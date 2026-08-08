use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PowerAction {
    Shutdown,
    Restart,
    Sleep,
    Hibernate,
    Lock,
}

impl PowerAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Shutdown => "Arrêt",
            Self::Restart => "Redémarrage",
            Self::Sleep => "Veille",
            Self::Hibernate => "Hibernation",
            Self::Lock => "Verrouillage",
        }
    }

    pub fn requires_pro(self) -> bool {
        matches!(self, Self::Sleep | Self::Hibernate | Self::Lock)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct SmartConditions {
    pub cpu_below_percent: Option<f32>,
    pub cpu_for_seconds: Option<u64>,
    pub process_closed: Option<String>,
    pub idle_seconds: Option<u64>,
    pub target_unix: Option<i64>,
}

impl SmartConditions {
    pub fn is_empty(&self) -> bool {
        self.cpu_below_percent.is_none()
            && self.process_closed.as_ref().map(|s| s.trim().is_empty()).unwrap_or(true)
            && self.idle_seconds.is_none()
            && self.target_unix.is_none()
    }

    pub fn requires_pro(&self) -> bool {
        !self.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ConditionStatus {
    pub cpu_percent: f32,
    pub cpu_ok_for_seconds: u64,
    pub process_running: Option<bool>,
    pub idle_seconds: u64,
    pub target_reached: bool,
    pub all_met: bool,
    pub summary: String,
}

#[cfg(windows)]
pub fn execute(action: PowerAction) -> Result<(), String> {
    match action {
        PowerAction::Shutdown => run_shutdown("/s"),
        PowerAction::Restart => run_shutdown("/r"),
        PowerAction::Sleep => suspend(false),
        PowerAction::Hibernate => suspend(true),
        PowerAction::Lock => lock_workstation(),
    }
}

#[cfg(not(windows))]
pub fn execute(action: PowerAction) -> Result<(), String> {
    Err(format!(
        "Action {:?} disponible uniquement sur Windows",
        action
    ))
}

#[cfg(windows)]
fn run_shutdown(flag: &str) -> Result<(), String> {
    let status = std::process::Command::new("shutdown.exe")
        .args([flag, "/t", "0", "/f"])
        .status()
        .map_err(|e| format!("Impossible de lancer shutdown.exe: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("shutdown.exe a échoué (code {status})"))
    }
}

#[cfg(windows)]
fn suspend(hibernate: bool) -> Result<(), String> {
    use windows::Win32::System::Power::SetSuspendState;

    // Ensure hibernate is enabled when requested
    if hibernate {
        let _ = std::process::Command::new("powercfg")
            .args(["/hibernate", "on"])
            .status();
    }

    let ok = unsafe { SetSuspendState(hibernate, false, false) };
    if ok.as_bool() {
        Ok(())
    } else {
        Err(if hibernate {
            "Hibernation impossible (désactivée ou non supportée)".into()
        } else {
            "Mise en veille impossible".into()
        })
    }
}

#[cfg(windows)]
fn lock_workstation() -> Result<(), String> {
    use windows::Win32::System::Shutdown::LockWorkStation;

    let ok = unsafe { LockWorkStation() };
    if ok.is_ok() {
        Ok(())
    } else {
        Err("Verrouillage impossible".into())
    }
}

#[cfg(windows)]
pub fn cancel_windows_shutdown() {
    let _ = std::process::Command::new("shutdown.exe")
        .args(["/a"])
        .status();
}

#[cfg(not(windows))]
pub fn cancel_windows_shutdown() {}
