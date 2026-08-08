import { useEffect, useState, useTransition } from "react";
import { listen } from "@tauri-apps/api/event";
import { enable as enableAutostart, disable as disableAutostart } from "@tauri-apps/plugin-autostart";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  activateLicense,
  cancelSchedule,
  clearHistory,
  deactivateLicense,
  getLicense,
  getSchedule,
  getSettings,
  listProcesses,
  parseDuration,
  rebindHotkeys,
  saveSettings,
  schedulePower,
  setWidgetVisible,
} from "./api";
import { COMMERCE } from "./commerce";
import { actionLabel, dayLabel, t } from "./i18n";
import {
  ACCENT_OPTIONS,
  ACTION_IDS,
  PRO_ACTIONS,
  defaultRecurringRule,
  emptyConditions,
  formatCountdown,
  hasConditions,
  parseDurationClient,
  playBeep,
  type AlertPayload,
  type AppSettings,
  type LicenseInfo,
  type Locale,
  type PowerAction,
  type Profile,
  type RecurringRule,
  type ScheduleSnapshot,
  type SmartConditions,
} from "./types";
import "./App.css";

type Panel = "main" | "confirm" | "settings" | "license" | "history" | "processes";

function App() {
  const [panel, setPanel] = useState<Panel>("main");
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [license, setLicense] = useState<LicenseInfo | null>(null);
  const [schedule, setSchedule] = useState<ScheduleSnapshot | null>(null);
  const [durationInput, setDurationInput] = useState("30m");
  const [action, setAction] = useState<PowerAction>("shutdown");
  const [conditions, setConditions] = useState<SmartConditions>(emptyConditions());
  const [showConditions, setShowConditions] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [licenseInput, setLicenseInput] = useState("");
  const [processes, setProcesses] = useState<string[]>([]);
  const [processFilter, setProcessFilter] = useState("");
  const [newPreset, setNewPreset] = useState("");
  const [profileName, setProfileName] = useState("");
  const [updateMsg, setUpdateMsg] = useState<string | null>(null);
  const [, startTransition] = useTransition();
  const [pulse, setPulse] = useState(false);

  const isPro = license?.isPro ?? false;
  const locale: Locale = settings?.locale === "en" ? "en" : "fr";
  const parsedSeconds = parseDurationClient(durationInput);
  const active = schedule?.active ?? false;
  const inGrace = schedule?.inGrace ?? false;
  const remaining = schedule?.remainingSeconds ?? 0;
  const graceRemaining = schedule?.graceRemainingSeconds ?? 0;
  const lastMinute =
    active && !schedule?.waitingForConditions && !inGrace && remaining <= 60;

  useEffect(() => {
    const cleanups: Array<() => void> = [];

    (async () => {
      try {
        const [s, lic, sched] = await Promise.all([
          getSettings(),
          getLicense(),
          getSchedule(),
        ]);
        setSettings(s);
        setLicense(lic);
        setSchedule(sched);
        setDurationInput(s.lastDurationInput || "30m");
        setAction(s.lastAction || "shutdown");
        document.documentElement.style.setProperty(
          "--accent",
          lic.isPro ? s.accent || "#e2a84a" : "#e2a84a",
        );

        if (s.autoCheckUpdates !== false) {
          window.setTimeout(() => {
            void checkForUpdates({ silent: true });
          }, 1500);
        }

        cleanups.push(
          await listen<ScheduleSnapshot>("schedule-updated", (event) => {
            startTransition(() => setSchedule(event.payload));
          }),
        );
        cleanups.push(
          await listen<AppSettings>("settings-updated", (event) => {
            setSettings(event.payload);
          }),
        );
        cleanups.push(
          await listen<AlertPayload>("timer-alert", (event) => {
            if (event.payload.sound) playBeep(event.payload.stage);
          }),
        );
      } catch (e) {
        setError(String(e));
      }
    })();

    return () => cleanups.forEach((fn) => fn());
  }, []);

  useEffect(() => {
    if (!lastMinute) {
      setPulse(false);
      return;
    }
    setPulse(true);
    const id = window.setInterval(() => setPulse((p) => !p), 900);
    return () => window.clearInterval(id);
  }, [lastMinute]);

  useEffect(() => {
    document.documentElement.style.setProperty(
      "--accent",
      isPro ? settings?.accent || "#e2a84a" : "#e2a84a",
    );
  }, [isPro, settings?.accent]);

  async function persist(partial: Partial<AppSettings>) {
    if (!settings) return;
    if (partial.accent && !isPro) {
      setError(t(locale, "themePro"));
      setPanel("license");
      return;
    }
    if (partial.wakeToExecute && !isPro) {
      setError(t(locale, "wakePro"));
      setPanel("license");
      return;
    }
    if (partial.recurring && !isPro && partial.recurring.length > 1) {
      setError(t(locale, "recurringPro"));
      setPanel("license");
      return;
    }
    const next = { ...settings, ...partial };
    const saved = await saveSettings(next);
    setSettings(saved);
    if (partial.accent) {
      document.documentElement.style.setProperty("--accent", partial.accent);
    }
    if (partial.launchOnStartup !== undefined) {
      try {
        if (partial.launchOnStartup) await enableAutostart();
        else await disableAutostart();
      } catch {
        /* ignore */
      }
    }
    if (partial.hotkeyOpen !== undefined || partial.hotkeyCancel !== undefined) {
      try {
        await rebindHotkeys();
      } catch (e) {
        setError(String(e));
      }
    }
    if (partial.widgetEnabled !== undefined) {
      await setWidgetVisible(partial.widgetEnabled);
    }
  }

  function applyProfile(p: Profile) {
    setDurationInput(p.durationInput);
    setAction(p.action);
    setConditions({ ...emptyConditions(), ...p.conditions });
    setShowConditions(hasConditions(p.conditions));
    setError(null);
  }

  async function openConfirm() {
    setError(null);
    if (PRO_ACTIONS.includes(action) && !isPro) {
      setError(t(locale, "powerPro"));
      setPanel("license");
      return;
    }
    if (hasConditions(conditions) && !isPro) {
      setError(t(locale, "conditionsPro"));
      setPanel("license");
      return;
    }
    try {
      await parseDuration(durationInput);
      await persist({ lastAction: action, lastDurationInput: durationInput });
      setPanel("confirm");
    } catch (e) {
      setError(String(e));
    }
  }

  async function confirmSchedule() {
    setError(null);
    try {
      const seconds = await parseDuration(durationInput);
      const payload: SmartConditions = {
        cpu_below_percent: conditions.cpu_below_percent ?? null,
        cpu_for_seconds: conditions.cpu_for_seconds ?? 60,
        process_closed: conditions.process_closed?.trim() || null,
        idle_seconds: conditions.idle_seconds ?? null,
        target_unix: conditions.target_unix ?? null,
      };
      const snap = await schedulePower(
        action,
        seconds,
        hasConditions(conditions) ? payload : undefined,
      );
      setSchedule(snap);
      setPanel("main");
    } catch (e) {
      setError(String(e));
    }
  }

  async function onCancel() {
    setError(null);
    try {
      setSchedule(await cancelSchedule());
    } catch (e) {
      setError(String(e));
    }
  }

  async function loadProcesses() {
    try {
      setProcesses(await listProcesses());
    } catch {
      setProcesses([]);
    }
  }

  async function openProcessPicker() {
    setError(null);
    setPanel("processes");
    setProcessFilter("");
    await loadProcesses();
  }

  async function addPreset() {
    if (!settings || !newPreset.trim()) return;
    if (!isPro && settings.presets.length >= 4) {
      setError(t(locale, "freePresets"));
      setPanel("license");
      return;
    }
    if (parseDurationClient(newPreset) == null) {
      setError(t(locale, "invalidPreset"));
      return;
    }
    await persist({
      presets: [...settings.presets, newPreset.trim().toLowerCase()],
    });
    setNewPreset("");
  }

  async function saveCurrentAsProfile() {
    if (!settings || !profileName.trim()) return;
    if (!isPro && settings.profiles.length >= 3) {
      setError(t(locale, "freeProfiles"));
      setPanel("license");
      return;
    }
    const profile: Profile = {
      id: `p-${Date.now()}`,
      name: profileName.trim(),
      durationInput,
      action,
      conditions: { ...conditions },
    };
    await persist({ profiles: [...settings.profiles, profile] });
    setProfileName("");
  }

  async function removeProfile(id: string) {
    if (!settings) return;
    await persist({ profiles: settings.profiles.filter((p) => p.id !== id) });
  }

  function updateRecurring(next: RecurringRule[]) {
    void persist({ recurring: next });
  }

  function patchRule(id: string, patch: Partial<RecurringRule>) {
    if (!settings) return;
    updateRecurring(
      settings.recurring.map((r) => (r.id === id ? { ...r, ...patch } : r)),
    );
  }

  function addRecurringRule() {
    if (!settings) return;
    if (!isPro && settings.recurring.length >= 1) {
      setError(t(locale, "recurringPro"));
      setPanel("license");
      return;
    }
    updateRecurring([...settings.recurring, defaultRecurringRule(action)]);
  }

  function removeRecurringRule(id: string) {
    if (!settings) return;
    updateRecurring(settings.recurring.filter((r) => r.id !== id));
  }

  async function onActivateLicense() {
    setError(null);
    try {
      const lic = await activateLicense(licenseInput);
      setLicense(lic);
      setLicenseInput("");
      setPanel("main");
    } catch (e) {
      setError(String(e));
    }
  }

  async function checkForUpdates(opts?: { silent?: boolean }) {
    const silent = opts?.silent ?? false;
    if (!silent) setUpdateMsg(t(locale, "checking"));
    try {
      const update = await check();
      if (!update) {
        if (!silent) setUpdateMsg(t(locale, "upToDate"));
        return;
      }
      setUpdateMsg(
        `${t(locale, "updatePrefix")} ${update.version} : ${t(locale, "installing")}`,
      );
      await update.downloadAndInstall();
      setUpdateMsg(t(locale, "installedRestart"));
      await relaunch();
    } catch (e) {
      if (!silent) {
        setUpdateMsg(t(locale, "updateFail") + String(e));
      }
    }
  }

  const filteredProcesses = processes.filter((p) =>
    p.toLowerCase().includes(processFilter.toLowerCase()),
  );

  const brand = (
    <header className="brand">
      <img className="brand-mark" src="/icon.png" alt="" width={42} height={42} />
      <div>
        <p className="brand-name">Aquerty Stop</p>
        <p className="brand-tag">{isPro ? t(locale, "brandPro") : t(locale, "brandFree")}</p>
      </div>
    </header>
  );

  return (
    <div className={`app ${pulse ? "pulse" : ""} ${inGrace ? "in-grace" : ""}`}>
      <div className="atmosphere" aria-hidden />
      <main className="shell">
        {brand}
        {error && <p className="error">{error}</p>}

        {inGrace && (
          <div className="grace-banner enter" role="alert">
            <p className="eyebrow">{t(locale, "graceOverlay")}</p>
            <h1 className="countdown grace-count">
              {formatCountdown(graceRemaining)}
            </h1>
            <p className="hint">{t(locale, "graceCancelHint")}</p>
            <button className="btn danger" onClick={onCancel}>
              {t(locale, "cancel")}
            </button>
          </div>
        )}

        {panel === "main" && !inGrace && (
          <section className="hero enter main-panel">
            {active ? (
              <>
                <p className="eyebrow">
                  {schedule?.waitingForConditions
                    ? t(locale, "waitingConditions")
                    : schedule?.fromRecurring
                      ? t(locale, "recurringArmed")
                      : schedule?.actionLabel}
                </p>
                <h1 className="countdown">{formatCountdown(remaining)}</h1>
                {schedule?.conditionStatus && (
                  <p className="condition-line">{schedule.conditionStatus.summary}</p>
                )}
                <button className="btn danger" onClick={onCancel}>
                  {t(locale, "cancel")}
                </button>
                <nav className="footer-nav">
                  <button type="button" className="ghost" onClick={() => setPanel("settings")}>
                    {t(locale, "settings")}
                  </button>
                  <button type="button" className="ghost" onClick={() => setPanel("history")}>
                    {t(locale, "history")}
                  </button>
                  <button type="button" className="ghost" onClick={() => setPanel("license")}>
                    {t(locale, "license")}
                  </button>
                </nav>
              </>
            ) : (
              <>
                <div className="main-scroll">
                  <p className="eyebrow">{t(locale, "scheduleAction")}</p>

                  {(settings?.profiles?.length ?? 0) > 0 && (
                    <div className="profiles" role="group" aria-label={t(locale, "profiles")}>
                      {settings!.profiles.map((p) => (
                        <button
                          key={p.id}
                          type="button"
                          className="chip profile"
                          onClick={() => applyProfile(p)}
                          title={`${p.durationInput} · ${p.action}`}
                        >
                          {p.name}
                        </button>
                      ))}
                    </div>
                  )}

                  <div className="time-block">
                    <input
                      className="time-input"
                      value={durationInput}
                      onChange={(e) => setDurationInput(e.target.value)}
                      placeholder="1h20m"
                      spellCheck={false}
                      aria-label="Duration"
                    />
                    <p className="hint">
                      {parsedSeconds != null
                        ? `= ${formatCountdown(parsedSeconds)}`
                        : t(locale, "durationHint")}
                    </p>
                  </div>

                  <div className="presets" role="group" aria-label={t(locale, "presets")}>
                    {(settings?.presets ?? []).map((p) => (
                      <button
                        key={p}
                        type="button"
                        className={`chip ${durationInput === p ? "active" : ""}`}
                        onClick={() => setDurationInput(p)}
                      >
                        {p}
                      </button>
                    ))}
                  </div>

                  <div className="actions" role="group" aria-label="Action">
                    {ACTION_IDS.map((id) => (
                      <button
                        key={id}
                        type="button"
                        className={`action ${action === id ? "active" : ""}`}
                        onClick={() => setAction(id)}
                      >
                        {actionLabel(locale, id)}
                        {PRO_ACTIONS.includes(id) && !isPro ? (
                          <span className="pro-dot">Pro</span>
                        ) : null}
                      </button>
                    ))}
                  </div>

                  <div className="row tight">
                    <button type="button" className="linkish" onClick={openProcessPicker}>
                      {t(locale, "endOfProcess")}
                    </button>
                    <button
                      type="button"
                      className="linkish"
                      onClick={() => setShowConditions((v) => !v)}
                    >
                      {showConditions
                        ? t(locale, "hideConditions")
                        : t(locale, "conditions")}
                      {!isPro ? t(locale, "proSuffix") : ""}
                    </button>
                  </div>

                  {showConditions && (
                    <div className="conditions enter">
                      <label>
                        {t(locale, "cpuBelow")}
                        <input
                          type="number"
                          min={1}
                          max={100}
                          placeholder="%"
                          value={conditions.cpu_below_percent ?? ""}
                          onChange={(e) =>
                            setConditions((c) => ({
                              ...c,
                              cpu_below_percent: e.target.value
                                ? Number(e.target.value)
                                : null,
                            }))
                          }
                        />
                      </label>
                      <label>
                        {t(locale, "forSeconds")}
                        <input
                          type="number"
                          min={1}
                          placeholder="60"
                          value={conditions.cpu_for_seconds ?? ""}
                          onChange={(e) =>
                            setConditions((c) => ({
                              ...c,
                              cpu_for_seconds: e.target.value
                                ? Number(e.target.value)
                                : null,
                            }))
                          }
                        />
                      </label>
                      <label className="wide">
                        {t(locale, "whenProcessCloses")}
                        <input
                          placeholder="ex. chrome.exe"
                          value={conditions.process_closed ?? ""}
                          onChange={(e) =>
                            setConditions((c) => ({
                              ...c,
                              process_closed: e.target.value || null,
                            }))
                          }
                        />
                      </label>
                      <label>
                        {t(locale, "idleSeconds")}
                        <input
                          type="number"
                          min={1}
                          placeholder="300"
                          value={conditions.idle_seconds ?? ""}
                          onChange={(e) =>
                            setConditions((c) => ({
                              ...c,
                              idle_seconds: e.target.value
                                ? Number(e.target.value)
                                : null,
                            }))
                          }
                        />
                      </label>
                      <label className="wide">
                        {t(locale, "targetTime")}
                        <input
                          type="datetime-local"
                          onChange={(e) => {
                            const v = e.target.value;
                            setConditions((c) => ({
                              ...c,
                              target_unix: v
                                ? Math.floor(new Date(v).getTime() / 1000)
                                : null,
                            }));
                          }}
                        />
                      </label>
                    </div>
                  )}

                  <button className="btn primary" onClick={openConfirm}>
                    {t(locale, "continue")}
                  </button>
                </div>

                <nav className="footer-nav">
                  <button type="button" className="ghost" onClick={() => setPanel("settings")}>
                    {t(locale, "settings")}
                  </button>
                  <button type="button" className="ghost" onClick={() => setPanel("history")}>
                    {t(locale, "history")}
                  </button>
                  <button type="button" className="ghost" onClick={() => setPanel("license")}>
                    {t(locale, "license")}
                  </button>
                </nav>
              </>
            )}
          </section>
        )}

        {panel === "confirm" && (
          <section className="hero enter">
            <p className="eyebrow">{t(locale, "confirmation")}</p>
            <h2 className="confirm-title">
              {actionLabel(locale, action)} {t(locale, "in")}{" "}
              {formatCountdown(parsedSeconds ?? 0)}
            </h2>
            <p className="hint">
              {t(locale, "requestedTime")} <strong>{durationInput}</strong>
              {hasConditions(conditions) ? t(locale, "withConditions") : ""}
            </p>
            <div className="row">
              <button className="btn primary" onClick={confirmSchedule}>
                {t(locale, "confirm")}
              </button>
              <button className="btn" onClick={() => setPanel("main")}>
                {t(locale, "back")}
              </button>
            </div>
          </section>
        )}

        {panel === "processes" && (
          <section className="hero enter panel">
            <p className="eyebrow">{t(locale, "endOfProcessTitle")}</p>
            <h2 className="confirm-title">{t(locale, "whenAppCloses")}</h2>
            <input
              className="time-input small"
              value={processFilter}
              onChange={(e) => setProcessFilter(e.target.value)}
              placeholder={t(locale, "filter")}
            />
            <div className="process-list">
              {filteredProcesses.slice(0, 40).map((p) => (
                <button
                  key={p}
                  type="button"
                  className="process-item"
                  onClick={() => {
                    setConditions((c) => ({ ...c, process_closed: p }));
                    setShowConditions(true);
                    setPanel("main");
                  }}
                >
                  {p}
                </button>
              ))}
            </div>
            <button className="btn" onClick={() => setPanel("main")}>
              {t(locale, "back")}
            </button>
          </section>
        )}

        {panel === "history" && settings && (
          <section className="hero enter panel">
            <p className="eyebrow">{t(locale, "history")}</p>
            {settings.history.length === 0 ? (
              <p className="hint">{t(locale, "noHistory")}</p>
            ) : (
              <ul className="history-list">
                {settings.history.map((h, i) => (
                  <li key={`${h.atUnix}-${i}`}>
                    <strong>{h.actionLabel}</strong>
                    <span>
                      {h.durationLabel}
                      {h.cancelled ? t(locale, "cancelled") : ""}
                    </span>
                    <em>{new Date(h.atUnix * 1000).toLocaleString()}</em>
                  </li>
                ))}
              </ul>
            )}
            <div className="row">
              {settings.history.length > 0 && (
                <button
                  className="btn"
                  onClick={async () => setSettings(await clearHistory())}
                >
                  {t(locale, "clearHistory")}
                </button>
              )}
              <button className="btn" onClick={() => setPanel("main")}>
                {t(locale, "back")}
              </button>
            </div>
          </section>
        )}

        {panel === "settings" && settings && (
          <section className="hero enter panel settings-panel">
            <div className="settings-header">
              <p className="eyebrow">{t(locale, "settings")}</p>
              <h2 className="confirm-title settings-title">{t(locale, "preferences")}</h2>
            </div>

            <div className="settings-scroll">
              <div className="settings-section">
                <h3 className="settings-heading">{t(locale, "language")}</h3>
                <p className="settings-desc">{t(locale, "languageDesc")}</p>
                <div className="presets">
                  <button
                    type="button"
                    className={`chip ${locale === "fr" ? "active" : ""}`}
                    onClick={() => void persist({ locale: "fr" })}
                  >
                    {t(locale, "french")}
                  </button>
                  <button
                    type="button"
                    className={`chip ${locale === "en" ? "active" : ""}`}
                    onClick={() => void persist({ locale: "en" })}
                  >
                    {t(locale, "english")}
                  </button>
                </div>
              </div>

              <div className="settings-section">
                <h3 className="settings-heading">{t(locale, "general")}</h3>
                <p className="settings-desc">{t(locale, "generalDesc")}</p>
                <label className="toggle-row">
                  <span>
                    <strong>{t(locale, "minimizeTray")}</strong>
                    <small>{t(locale, "minimizeTrayHint")}</small>
                  </span>
                  <input
                    type="checkbox"
                    checked={settings.minimizeToTray}
                    onChange={(e) => void persist({ minimizeToTray: e.target.checked })}
                  />
                </label>
                <label className="toggle-row">
                  <span>
                    <strong>{t(locale, "launchStartup")}</strong>
                    <small>{t(locale, "launchStartupHint")}</small>
                  </span>
                  <input
                    type="checkbox"
                    checked={settings.launchOnStartup}
                    onChange={(e) => void persist({ launchOnStartup: e.target.checked })}
                  />
                </label>
                <label className="toggle-row">
                  <span>
                    <strong>{t(locale, "miniWidget")}</strong>
                    <small>{t(locale, "miniWidgetHint")}</small>
                  </span>
                  <input
                    type="checkbox"
                    checked={settings.widgetEnabled}
                    onChange={(e) => void persist({ widgetEnabled: e.target.checked })}
                  />
                </label>
              </div>

              <div className="settings-section">
                <h3 className="settings-heading">{t(locale, "alerts")}</h3>
                <p className="settings-desc">{t(locale, "alertsDesc")}</p>
                <label className="toggle-row">
                  <span>
                    <strong>{t(locale, "alertSounds")}</strong>
                    <small>{t(locale, "alertSoundsHint")}</small>
                  </span>
                  <input
                    type="checkbox"
                    checked={settings.soundEnabled}
                    onChange={(e) => void persist({ soundEnabled: e.target.checked })}
                  />
                </label>
                <label className="toggle-row">
                  <span>
                    <strong>{t(locale, "notify5m")}</strong>
                  </span>
                  <input
                    type="checkbox"
                    checked={settings.notifyAt5m}
                    onChange={(e) => void persist({ notifyAt5m: e.target.checked })}
                  />
                </label>
                <label className="toggle-row">
                  <span>
                    <strong>{t(locale, "notify1m")}</strong>
                  </span>
                  <input
                    type="checkbox"
                    checked={settings.notifyAt1m}
                    onChange={(e) => void persist({ notifyAt1m: e.target.checked })}
                  />
                </label>
              </div>

              <div className="settings-section">
                <h3 className="settings-heading">{t(locale, "grace")}</h3>
                <p className="settings-desc">{t(locale, "graceDesc")}</p>
                <label className="toggle-row">
                  <span>
                    <strong>{t(locale, "graceEnabled")}</strong>
                    <small>{t(locale, "graceEnabledHint")}</small>
                  </span>
                  <input
                    type="checkbox"
                    checked={settings.graceEnabled !== false}
                    onChange={(e) => void persist({ graceEnabled: e.target.checked })}
                  />
                </label>
                <label className="field">
                  {t(locale, "graceSeconds")}
                  <input
                    type="number"
                    min={1}
                    max={600}
                    value={settings.graceSeconds ?? 30}
                    onChange={(e) =>
                      void persist({
                        graceSeconds: Math.min(
                          600,
                          Math.max(1, Number(e.target.value) || 30),
                        ),
                      })
                    }
                  />
                </label>
              </div>

              <div className="settings-section">
                <h3 className="settings-heading">
                  {t(locale, "wake")} {!isPro ? <span className="pro-dot">Pro</span> : null}
                </h3>
                <p className="settings-desc">{t(locale, "wakeDesc")}</p>
                <label className={`toggle-row ${!isPro ? "locked" : ""}`}>
                  <span>
                    <strong>{t(locale, "wakeToggle")}</strong>
                    <small>{t(locale, "wakeHint")}</small>
                  </span>
                  <input
                    type="checkbox"
                    checked={!!settings.wakeToExecute && isPro}
                    onChange={(e) => {
                      if (!isPro) {
                        setError(t(locale, "wakePro"));
                        setPanel("license");
                        return;
                      }
                      void persist({ wakeToExecute: e.target.checked });
                    }}
                  />
                </label>
              </div>

              <div className="settings-section">
                <h3 className="settings-heading">{t(locale, "recurring")}</h3>
                <p className="settings-desc">
                  {t(locale, "recurringDesc")} {t(locale, "recurringFreeHint")}
                </p>
                {schedule?.nextRecurringUnix ? (
                  <p className="hint">
                    {t(locale, "nextAt")}{" "}
                    {new Date(schedule.nextRecurringUnix * 1000).toLocaleString()}
                  </p>
                ) : null}
                {(settings.recurring ?? []).map((rule) => (
                  <div key={rule.id} className="recurring-card">
                    <label className="toggle-row">
                      <span>
                        <strong>{t(locale, "enabled")}</strong>
                      </span>
                      <input
                        type="checkbox"
                        checked={rule.enabled}
                        onChange={(e) =>
                          patchRule(rule.id, { enabled: e.target.checked })
                        }
                      />
                    </label>
                    <label className="field">
                      {t(locale, "time")}
                      <input
                        type="time"
                        value={`${String(rule.hour).padStart(2, "0")}:${String(rule.minute).padStart(2, "0")}`}
                        onChange={(e) => {
                          const [h, m] = e.target.value.split(":").map(Number);
                          patchRule(rule.id, {
                            hour: h || 0,
                            minute: m || 0,
                          });
                        }}
                      />
                    </label>
                    <p className="settings-desc">{t(locale, "days")}</p>
                    <div className="day-row">
                      {rule.days.map((on, i) => (
                        <button
                          key={i}
                          type="button"
                          className={`chip day-chip ${on ? "active" : ""}`}
                          onClick={() => {
                            const days = [...rule.days];
                            days[i] = !days[i];
                            patchRule(rule.id, { days });
                          }}
                        >
                          {dayLabel(locale, i)}
                        </button>
                      ))}
                    </div>
                    <div className="actions compact">
                      {ACTION_IDS.map((id) => (
                        <button
                          key={id}
                          type="button"
                          className={`action ${rule.action === id ? "active" : ""}`}
                          onClick={() => {
                            if (PRO_ACTIONS.includes(id) && !isPro) {
                              setError(t(locale, "powerPro"));
                              setPanel("license");
                              return;
                            }
                            patchRule(rule.id, { action: id });
                          }}
                        >
                          {actionLabel(locale, id)}
                        </button>
                      ))}
                    </div>
                    <button
                      type="button"
                      className="linkish"
                      onClick={() => removeRecurringRule(rule.id)}
                    >
                      {t(locale, "remove")}
                    </button>
                  </div>
                ))}
                <button className="btn" type="button" onClick={addRecurringRule}>
                  {t(locale, "addRule")}
                  {!isPro && (settings.recurring?.length ?? 0) >= 1 ? (
                    <span className="pro-dot">Pro</span>
                  ) : null}
                </button>
              </div>

              <div className="settings-section">
                <h3 className="settings-heading">
                  {t(locale, "appearance")}{" "}
                  {!isPro ? <span className="pro-dot">Pro</span> : null}
                </h3>
                <p className="settings-desc">{t(locale, "appearanceDesc")}</p>
                <div className="presets accent-row">
                  {ACCENT_OPTIONS.map((a) => (
                    <button
                      key={a.id}
                      type="button"
                      className={`chip accent-chip ${settings.accent === a.id ? "active" : ""} ${!isPro ? "locked" : ""}`}
                      style={{ ["--chip-accent" as string]: a.id }}
                      onClick={() => {
                        if (!isPro) {
                          setError(t(locale, "themePro"));
                          setPanel("license");
                          return;
                        }
                        void persist({ accent: a.id });
                      }}
                    >
                      <span className="accent-dot" style={{ background: a.id }} />
                      {t(locale, a.labelKey)}
                    </button>
                  ))}
                </div>
              </div>

              <div className="settings-section">
                <h3 className="settings-heading">{t(locale, "hotkeys")}</h3>
                <p className="settings-desc">{t(locale, "hotkeysDesc")}</p>
                <label className="field">
                  {t(locale, "hotkeyOpen")}
                  <input
                    defaultValue={settings.hotkeyOpen}
                    key={`open-${settings.hotkeyOpen}`}
                    onBlur={(e) => {
                      const v = e.target.value.trim();
                      if (v && v !== settings.hotkeyOpen) void persist({ hotkeyOpen: v });
                    }}
                  />
                </label>
                <label className="field">
                  {t(locale, "hotkeyCancel")}
                  <input
                    defaultValue={settings.hotkeyCancel}
                    key={`cancel-${settings.hotkeyCancel}`}
                    onBlur={(e) => {
                      const v = e.target.value.trim();
                      if (v && v !== settings.hotkeyCancel)
                        void persist({ hotkeyCancel: v });
                    }}
                  />
                </label>
              </div>

              <div className="settings-section">
                <h3 className="settings-heading">{t(locale, "presets")}</h3>
                <p className="settings-desc">{t(locale, "presetsDesc")}</p>
                <div className="presets">
                  {settings.presets.map((p) => (
                    <button
                      key={p}
                      type="button"
                      className="chip"
                      title={t(locale, "remove")}
                      onClick={() =>
                        void persist({
                          presets: settings.presets.filter((x) => x !== p),
                        })
                      }
                    >
                      {p} ×
                    </button>
                  ))}
                </div>
                <div className="row add-row">
                  <input
                    value={newPreset}
                    onChange={(e) => setNewPreset(e.target.value)}
                    placeholder="45m"
                  />
                  <button className="btn" type="button" onClick={addPreset}>
                    {t(locale, "add")}
                  </button>
                </div>
              </div>

              <div className="settings-section">
                <h3 className="settings-heading">{t(locale, "profiles")}</h3>
                <p className="settings-desc">{t(locale, "profilesDesc")}</p>
                <div className="presets">
                  {settings.profiles.map((p) => (
                    <button
                      key={p.id}
                      type="button"
                      className="chip profile"
                      title={t(locale, "remove")}
                      onClick={() => void removeProfile(p.id)}
                    >
                      {p.name} ×
                    </button>
                  ))}
                </div>
                <div className="row add-row">
                  <input
                    value={profileName}
                    onChange={(e) => setProfileName(e.target.value)}
                    placeholder={t(locale, "profilePlaceholder")}
                  />
                  <button className="btn" type="button" onClick={saveCurrentAsProfile}>
                    {t(locale, "save")}
                  </button>
                </div>
              </div>

              <div className="settings-section">
                <h3 className="settings-heading">{t(locale, "updates")}</h3>
                <p className="settings-desc">{t(locale, "updatesDesc")}</p>
                <label className="toggle-row">
                  <span>
                    <strong>{t(locale, "checkOnLaunch")}</strong>
                    <small>{t(locale, "checkOnLaunchHint")}</small>
                  </span>
                  <input
                    type="checkbox"
                    checked={settings.autoCheckUpdates !== false}
                    onChange={(e) =>
                      void persist({ autoCheckUpdates: e.target.checked })
                    }
                  />
                </label>
                <button
                  className="btn"
                  type="button"
                  onClick={() => void checkForUpdates()}
                >
                  {t(locale, "checkNow")}
                </button>
                {updateMsg && <p className="hint">{updateMsg}</p>}
              </div>
            </div>

            <div className="settings-footer">
              <button className="btn" onClick={() => setPanel("main")}>
                {t(locale, "back")}
              </button>
            </div>
          </section>
        )}

        {panel === "license" && (
          <section className="hero enter panel">
            <p className="eyebrow">{t(locale, "license")}</p>
            <h2 className="confirm-title">{license?.message ?? "…"}</h2>
            <p className="hint">
              {t(locale, "freeLicense")}
              <br />
              {t(locale, "proLicense")}
            </p>
            {!isPro ? (
              <>
                <div className="row" style={{ flexWrap: "wrap", gap: 8 }}>
                  <button
                    className="btn primary"
                    type="button"
                    onClick={() => void openUrl(COMMERCE.gumroadLifetime)}
                  >
                    {t(locale, "buyLifetime")}
                  </button>
                  <button
                    className="btn"
                    type="button"
                    onClick={() => void openUrl(COMMERCE.gumroadAnnual)}
                  >
                    {t(locale, "buyAnnual")}
                  </button>
                </div>
                <p className="hint">{t(locale, "pasteKeyHint")}</p>
                <input
                  className="time-input small"
                  value={licenseInput}
                  onChange={(e) => setLicenseInput(e.target.value)}
                  placeholder="AQUERTY-…"
                  spellCheck={false}
                />
                <button className="btn primary" onClick={onActivateLicense}>
                  {t(locale, "activatePro")}
                </button>
              </>
            ) : (
              <button
                className="btn"
                onClick={async () => setLicense(await deactivateLicense())}
              >
                {t(locale, "deactivateLicense")}
              </button>
            )}
            <button className="btn" onClick={() => setPanel("main")}>
              {t(locale, "back")}
            </button>
          </section>
        )}
      </main>
    </div>
  );
}

export default App;
