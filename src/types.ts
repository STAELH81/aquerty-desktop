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

export interface Profile {
  id: string;
  name: string;
  durationInput: string;
  action: PowerAction;
  conditions: SmartConditions;
}

export interface HistoryEntry {
  atUnix: number;
  action: PowerAction;
  actionLabel: string;
  durationSeconds: number;
  durationLabel: string;
  cancelled: boolean;
}

export interface AppSettings {
  lastAction: PowerAction;
  lastDurationInput: string;
  presets: string[];
  minimizeToTray: boolean;
  launchOnStartup: boolean;
  notifyBeforeSeconds: number;
  licenseKey?: string | null;
  profiles: Profile[];
  history: HistoryEntry[];
  soundEnabled: boolean;
  notifyAt5m: boolean;
  notifyAt1m: boolean;
  widgetEnabled: boolean;
  accent: string;
  hotkeyOpen: string;
  hotkeyCancel: string;
}

export interface LicenseInfo {
  isPro: boolean;
  key: string | null;
  message: string;
}

export interface AlertPayload {
  stage: string;
  remainingSeconds: number;
  sound: boolean;
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

export const ACCENT_OPTIONS = [
  { id: "#e2a84a", label: "Ambre" },
  { id: "#5ec2a0", label: "Menthe" },
  { id: "#6aa8e8", label: "Azur" },
  { id: "#d96a5b", label: "Corail" },
  { id: "#c4a1e8", label: "Lilas" },
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

export function emptyConditions(): SmartConditions {
  return {
    cpu_below_percent: null,
    cpu_for_seconds: null,
    process_closed: null,
    idle_seconds: null,
    target_unix: null,
  };
}

export function hasConditions(c: SmartConditions): boolean {
  return Boolean(
    c.cpu_below_percent != null ||
      (c.process_closed && c.process_closed.trim()) ||
      c.idle_seconds != null ||
      c.target_unix != null,
  );
}

export function playBeep(stage: string) {
  try {
    const ctx = new AudioContext();
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.type = "sine";
    osc.frequency.value = stage === "fire" ? 660 : stage === "1m" ? 520 : 440;
    gain.gain.value = 0.05;
    osc.connect(gain);
    gain.connect(ctx.destination);
    osc.start();
    gain.gain.exponentialRampToValueAtTime(0.0001, ctx.currentTime + 0.35);
    osc.stop(ctx.currentTime + 0.4);
    window.setTimeout(() => void ctx.close(), 500);
  } catch {
    /* ignore */
  }
}
