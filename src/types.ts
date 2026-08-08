export type PowerAction =
  | "shutdown"
  | "restart"
  | "sleep"
  | "hibernate"
  | "lock";

export interface SmartConditions {
  cpu_below_percent?: number | null;
  cpu_for_seconds?: number | null;
  process_closed?: string | null;
  idle_seconds?: number | null;
  target_unix?: number | null;
}

export interface ConditionStatus {
  cpu_percent: number;
  cpu_ok_for_seconds: number;
  process_running: boolean | null;
  idle_seconds: number;
  target_reached: boolean;
  all_met: boolean;
  summary: string;
}

export interface ScheduleSnapshot {
  active: boolean;
  action: PowerAction | null;
  actionLabel: string | null;
  remainingSeconds: number;
  totalSeconds: number;
  endsAtUnix: number | null;
  conditions: SmartConditions;
  conditionStatus: ConditionStatus | null;
  waitingForConditions: boolean;
}

export interface AppSettings {
  lastAction: PowerAction;
  lastDurationInput: string;
  presets: string[];
  minimizeToTray: boolean;
  launchOnStartup: boolean;
  notifyBeforeSeconds: number;
  licenseKey?: string | null;
}

export interface LicenseInfo {
  isPro: boolean;
  key: string | null;
  message: string;
}

export const ACTION_OPTIONS: {
  id: PowerAction;
  label: string;
  pro?: boolean;
}[] = [
  { id: "shutdown", label: "Arrêt" },
  { id: "restart", label: "Redémarrage" },
  { id: "sleep", label: "Veille", pro: true },
  { id: "hibernate", label: "Hibernation", pro: true },
  { id: "lock", label: "Verrouillage", pro: true },
];

export function formatCountdown(totalSeconds: number): string {
  const h = Math.floor(totalSeconds / 3600);
  const m = Math.floor((totalSeconds % 3600) / 60);
  const s = totalSeconds % 60;
  if (h > 0) {
    return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  }
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

/** Client-side mirror of Rust parser for instant feedback. */
export function parseDurationClient(input: string): number | null {
  const temps = input.trim().toLowerCase().replace(/\s+/g, "");
  if (!temps) return null;

  if (/^\d+$/.test(temps)) {
    const minutes = Number(temps);
    return minutes > 0 ? minutes * 60 : null;
  }

  if (!/^\d/.test(temps)) return null;
  const re = /^(?:(\d+)h)?(?:(\d+)m)?(?:(\d+)s)?$/;
  const match = temps.match(re);
  if (!match || (!match[1] && !match[2] && !match[3])) return null;

  const hours = Number(match[1] || 0);
  const minutes = Number(match[2] || 0);
  const seconds = Number(match[3] || 0);
  const total = hours * 3600 + minutes * 60 + seconds;
  return total > 0 ? total : null;
}
