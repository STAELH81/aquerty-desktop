use sysinfo::{ProcessesToUpdate, System};

use crate::power::SmartConditions;

#[cfg(windows)]
pub fn idle_seconds() -> u64 {
    use std::mem::size_of;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

    unsafe {
        let mut info = LASTINPUTINFO {
            cbSize: size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if GetLastInputInfo(&mut info).as_bool() {
            let tick = windows::Win32::System::SystemInformation::GetTickCount();
            let idle_ms = tick.wrapping_sub(info.dwTime);
            return (idle_ms / 1000) as u64;
        }
    }
    0
}

#[cfg(not(windows))]
pub fn idle_seconds() -> u64 {
    0
}

pub fn cpu_usage_percent(sys: &mut System) -> f32 {
    sys.refresh_cpu_usage();
    sys.global_cpu_usage()
}

pub fn is_process_running(sys: &mut System, name: &str) -> bool {
    let needle = name.trim().to_lowercase();
    if needle.is_empty() {
        return false;
    }
    sys.refresh_processes(ProcessesToUpdate::All, true);
    sys.processes().values().any(|p| {
        let pname = p.name().to_string_lossy().to_lowercase();
        pname == needle || pname == format!("{needle}.exe") || pname.contains(&needle)
    })
}

pub fn list_process_names(sys: &mut System, limit: usize) -> Vec<String> {
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let mut names: Vec<String> = sys
        .processes()
        .values()
        .map(|p| p.name().to_string_lossy().to_string())
        .collect();
    names.sort();
    names.dedup();
    names.into_iter().take(limit).collect()
}

pub struct ConditionTracker {
    pub cpu_ok_for_seconds: u64,
}

impl Default for ConditionTracker {
    fn default() -> Self {
        Self {
            cpu_ok_for_seconds: 0,
        }
    }
}

impl ConditionTracker {
    pub fn evaluate(
        &mut self,
        sys: &mut System,
        conditions: &SmartConditions,
        now_unix: i64,
    ) -> crate::power::ConditionStatus {
        let cpu = cpu_usage_percent(sys);
        let idle = idle_seconds();

        let cpu_threshold = conditions.cpu_below_percent.unwrap_or(100.0);
        let cpu_needed = conditions.cpu_for_seconds.unwrap_or(0);
        let cpu_gate_enabled = conditions.cpu_below_percent.is_some();

        if cpu_gate_enabled {
            if cpu <= cpu_threshold {
                self.cpu_ok_for_seconds = self.cpu_ok_for_seconds.saturating_add(1);
            } else {
                self.cpu_ok_for_seconds = 0;
            }
        } else {
            self.cpu_ok_for_seconds = 0;
        }

        let process_running = conditions
            .process_closed
            .as_ref()
            .filter(|s| !s.trim().is_empty())
            .map(|name| is_process_running(sys, name));

        let target_reached = conditions
            .target_unix
            .map(|t| now_unix >= t)
            .unwrap_or(true);

        let cpu_met = !cpu_gate_enabled || self.cpu_ok_for_seconds >= cpu_needed.max(1);
        let process_met = match process_running {
            Some(running) => !running,
            None => true,
        };
        let idle_met = match conditions.idle_seconds {
            Some(need) => idle >= need,
            None => true,
        };

        let all_met = cpu_met && process_met && idle_met && target_reached;

        let mut parts = Vec::new();
        if cpu_gate_enabled {
            parts.push(format!(
                "CPU {cpu:.0}% (ok {}s/{cpu_needed}s)",
                self.cpu_ok_for_seconds
            ));
        }
        if let Some(running) = process_running {
            let name = conditions.process_closed.clone().unwrap_or_default();
            parts.push(if running {
                format!("{name} encore ouvert")
            } else {
                format!("{name} fermé")
            });
        }
        if let Some(need) = conditions.idle_seconds {
            parts.push(format!("Inactivité {idle}s/{need}s"));
        }
        if conditions.target_unix.is_some() {
            parts.push(if target_reached {
                "Heure cible atteinte".into()
            } else {
                "En attente de l'heure cible".into()
            });
        }
        if parts.is_empty() {
            parts.push("Aucune condition".into());
        }

        crate::power::ConditionStatus {
            cpu_percent: cpu,
            cpu_ok_for_seconds: self.cpu_ok_for_seconds,
            process_running,
            idle_seconds: idle,
            target_reached,
            all_met,
            summary: parts.join(" · "),
        }
    }
}
