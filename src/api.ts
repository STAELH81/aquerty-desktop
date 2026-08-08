import { invoke } from "@tauri-apps/api/core";
import type {
  AppSettings,
  LicenseInfo,
  PowerAction,
  ScheduleSnapshot,
  SmartConditions,
} from "./types";

export function getSchedule() {
  return invoke<ScheduleSnapshot>("get_schedule");
}

export function getSettings() {
  return invoke<AppSettings>("get_settings");
}

export function saveSettings(settings: AppSettings) {
  return invoke<AppSettings>("save_settings", { newSettings: settings });
}

export function getLicense() {
  return invoke<LicenseInfo>("get_license");
}

export function activateLicense(key: string) {
  return invoke<LicenseInfo>("activate_license", { key });
}

export function deactivateLicense() {
  return invoke<LicenseInfo>("deactivate_license");
}

export function schedulePower(
  action: PowerAction,
  seconds: number,
  conditions?: SmartConditions,
) {
  return invoke<ScheduleSnapshot>("schedule_power", {
    request: { action, seconds, conditions },
  });
}

export function cancelSchedule() {
  return invoke<ScheduleSnapshot>("cancel_schedule");
}

export function listProcesses() {
  return invoke<string[]>("list_processes");
}

export function parseDuration(input: string) {
  return invoke<number>("parse_duration", { input });
}
