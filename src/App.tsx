import { useEffect, useState, useTransition } from "react";
import { listen } from "@tauri-apps/api/event";
import { enable as enableAutostart, disable as disableAutostart } from "@tauri-apps/plugin-autostart";
import {
  activateLicense,
  cancelSchedule,
  deactivateLicense,
  getLicense,
  getSchedule,
  getSettings,
  listProcesses,
  parseDuration,
  saveSettings,
  schedulePower,
} from "./api";
import {
  ACTION_OPTIONS,
  formatCountdown,
  parseDurationClient,
  type AppSettings,
  type LicenseInfo,
  type PowerAction,
  type ScheduleSnapshot,
  type SmartConditions,
} from "./types";
import "./App.css";

type Panel = "main" | "confirm" | "settings" | "license";

const emptyConditions = (): SmartConditions => ({
  cpu_below_percent: null,
  cpu_for_seconds: null,
  process_closed: null,
  idle_seconds: null,
  target_unix: null,
});

function hasConditions(c: SmartConditions): boolean {
  return Boolean(
    c.cpu_below_percent != null ||
      (c.process_closed && c.process_closed.trim()) ||
      c.idle_seconds != null ||
      c.target_unix != null,
  );
}

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
  const [newPreset, setNewPreset] = useState("");
  const [, startTransition] = useTransition();
  const [pulse, setPulse] = useState(false);

  const isPro = license?.isPro ?? false;
  const parsedSeconds = parseDurationClient(durationInput);
  const active = schedule?.active ?? false;
  const remaining = schedule?.remainingSeconds ?? 0;
  const lastMinute = active && !schedule?.waitingForConditions && remaining <= 60;

  useEffect(() => {
    let unlisten: (() => void) | undefined;

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
        unlisten = await listen<ScheduleSnapshot>("schedule-updated", (event) => {
          startTransition(() => setSchedule(event.payload));
        });
      } catch (e) {
        setError(String(e));
      }
    })();

    return () => {
      unlisten?.();
    };
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

  async function refreshLicense() {
    setLicense(await getLicense());
  }

  async function persist(partial: Partial<AppSettings>) {
    if (!settings) return;
    const next = { ...settings, ...partial };
    const saved = await saveSettings(next);
    setSettings(saved);

    if (partial.launchOnStartup !== undefined) {
      try {
        if (partial.launchOnStartup) await enableAutostart();
        else await disableAutostart();
      } catch {
        /* autostart may fail without permissions */
      }
    }
  }

  async function openConfirm() {
    setError(null);
    const opt = ACTION_OPTIONS.find((a) => a.id === action);
    if (opt?.pro && !isPro) {
      setError("Veille, hibernation et verrouillage sont réservés à Pro.");
      setPanel("license");
      return;
    }
    if (hasConditions(conditions) && !isPro) {
      setError("Les conditions intelligentes sont réservées à Pro.");
      setPanel("license");
      return;
    }
    try {
      const seconds = await parseDuration(durationInput);
      if (seconds <= 0 && !hasConditions(conditions)) {
        setError("Le temps doit être supérieur à 0.");
        return;
      }
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

  async function addPreset() {
    if (!settings || !newPreset.trim()) return;
    if (!isPro && settings.presets.length >= 4) {
      setError("Version gratuite : 4 presets max. Passez Pro pour en ajouter.");
      setPanel("license");
      return;
    }
    if (parseDurationClient(newPreset) == null) {
      setError("Preset invalide.");
      return;
    }
    const presets = [...settings.presets, newPreset.trim().toLowerCase()];
    await persist({ presets });
    setNewPreset("");
  }

  async function onActivateLicense() {
    setError(null);
    try {
      const info = await activateLicense(licenseInput);
      setLicense(info);
      setLicenseInput("");
      setPanel("main");
    } catch (e) {
      setError(String(e));
    }
  }

  async function onDeactivateLicense() {
    setLicense(await deactivateLicense());
  }

  const brand = (
    <header className="brand">
      <div className="brand-mark" aria-hidden />
      <div>
        <p className="brand-name">Aquerty Stop</p>
        <p className="brand-tag">{isPro ? "Pro" : "Free"}</p>
      </div>
    </header>
  );

  return (
    <div className={`app ${pulse ? "pulse" : ""}`}>
      <div className="atmosphere" aria-hidden />
      <main className="shell">
        {brand}

        {error && <p className="error">{error}</p>}

        {panel === "main" && (
          <section className="hero enter">
            {active ? (
              <>
                <p className="eyebrow">
                  {schedule?.waitingForConditions
                    ? "En attente des conditions"
                    : schedule?.actionLabel}
                </p>
                <h1 className="countdown">{formatCountdown(remaining)}</h1>
                {schedule?.conditionStatus && (
                  <p className="condition-line">{schedule.conditionStatus.summary}</p>
                )}
                <button className="btn danger" onClick={onCancel}>
                  Annuler
                </button>
              </>
            ) : (
              <>
                <p className="eyebrow">Programmer une action</p>
                <div className="time-block">
                  <input
                    className="time-input"
                    value={durationInput}
                    onChange={(e) => setDurationInput(e.target.value)}
                    placeholder="1h20m"
                    spellCheck={false}
                    aria-label="Durée"
                  />
                  <p className="hint">
                    {parsedSeconds != null
                      ? `= ${formatCountdown(parsedSeconds)}`
                      : "Formats : 30s · 45m · 2h · 1h20m"}
                  </p>
                </div>

                <div className="presets" role="group" aria-label="Presets">
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
                  {ACTION_OPTIONS.map((opt) => (
                    <button
                      key={opt.id}
                      type="button"
                      className={`action ${action === opt.id ? "active" : ""}`}
                      onClick={() => setAction(opt.id)}
                    >
                      {opt.label}
                      {opt.pro && !isPro ? <span className="pro-dot">Pro</span> : null}
                    </button>
                  ))}
                </div>

                <button
                  type="button"
                  className="linkish"
                  onClick={() => {
                    setShowConditions((v) => !v);
                    if (!showConditions) void loadProcesses();
                  }}
                >
                  {showConditions ? "Masquer les conditions" : "Conditions intelligentes"}
                  {!isPro ? " (Pro)" : ""}
                </button>

                {showConditions && (
                  <div className="conditions enter">
                    <label>
                      CPU sous
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
                      pendant (s)
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
                      Quand le processus se ferme
                      <input
                        list="process-list"
                        placeholder="ex. chrome.exe"
                        value={conditions.process_closed ?? ""}
                        onChange={(e) =>
                          setConditions((c) => ({
                            ...c,
                            process_closed: e.target.value || null,
                          }))
                        }
                      />
                      <datalist id="process-list">
                        {processes.map((p) => (
                          <option key={p} value={p} />
                        ))}
                      </datalist>
                    </label>
                    <label>
                      Inactivité (s)
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
                      Heure cible
                      <input
                        type="datetime-local"
                        onChange={(e) => {
                          const v = e.target.value;
                          setConditions((c) => ({
                            ...c,
                            target_unix: v ? Math.floor(new Date(v).getTime() / 1000) : null,
                          }));
                        }}
                      />
                    </label>
                  </div>
                )}

                <button className="btn primary" onClick={openConfirm}>
                  Continuer
                </button>
              </>
            )}

            <nav className="footer-nav">
              <button type="button" className="ghost" onClick={() => setPanel("settings")}>
                Réglages
              </button>
              <button type="button" className="ghost" onClick={() => setPanel("license")}>
                Licence
              </button>
            </nav>
          </section>
        )}

        {panel === "confirm" && (
          <section className="hero enter">
            <p className="eyebrow">Confirmation</p>
            <h2 className="confirm-title">
              {ACTION_OPTIONS.find((a) => a.id === action)?.label} dans{" "}
              {formatCountdown(parsedSeconds ?? 0)}
            </h2>
            <p className="hint">
              Temps demandé : <strong>{durationInput}</strong>
              {hasConditions(conditions) ? " · avec conditions" : ""}
            </p>
            <div className="row">
              <button className="btn primary" onClick={confirmSchedule}>
                Confirmer
              </button>
              <button className="btn" onClick={() => setPanel("main")}>
                Retour
              </button>
            </div>
          </section>
        )}

        {panel === "settings" && settings && (
          <section className="hero enter panel">
            <p className="eyebrow">Réglages</p>
            <label className="toggle">
              <input
                type="checkbox"
                checked={settings.minimizeToTray}
                onChange={(e) => void persist({ minimizeToTray: e.target.checked })}
              />
              Réduire dans le tray à la fermeture
            </label>
            <label className="toggle">
              <input
                type="checkbox"
                checked={settings.launchOnStartup}
                onChange={(e) => void persist({ launchOnStartup: e.target.checked })}
              />
              Lancer au démarrage de Windows
            </label>
            <label>
              Notifier avant (secondes)
              <input
                type="number"
                min={5}
                max={600}
                value={settings.notifyBeforeSeconds}
                onChange={(e) =>
                  void persist({ notifyBeforeSeconds: Number(e.target.value) || 60 })
                }
              />
            </label>

            <div className="preset-edit">
              <p className="hint">Presets</p>
              <div className="presets">
                {settings.presets.map((p) => (
                  <button
                    key={p}
                    type="button"
                    className="chip"
                    onClick={() =>
                      void persist({
                        presets: settings.presets.filter((x) => x !== p),
                      })
                    }
                    title="Supprimer"
                  >
                    {p} ×
                  </button>
                ))}
              </div>
              <div className="row">
                <input
                  value={newPreset}
                  onChange={(e) => setNewPreset(e.target.value)}
                  placeholder="45m"
                />
                <button className="btn" type="button" onClick={addPreset}>
                  Ajouter
                </button>
              </div>
            </div>

            <button className="btn" onClick={() => setPanel("main")}>
              Retour
            </button>
          </section>
        )}

        {panel === "license" && (
          <section className="hero enter panel">
            <p className="eyebrow">Licence</p>
            <h2 className="confirm-title">{license?.message ?? "…"}</h2>
            <p className="hint">
              Gratuit : arrêt, redémarrage, 4 presets.
              <br />
              Pro : veille, hibernation, verrouillage, conditions, presets illimités.
            </p>
            {!isPro ? (
              <>
                <input
                  className="time-input small"
                  value={licenseInput}
                  onChange={(e) => setLicenseInput(e.target.value)}
                  placeholder="AQUERTY-…"
                  spellCheck={false}
                />
                <p className="hint">Demo : AQUERTY-PRO-DEMO-2026</p>
                <button className="btn primary" onClick={onActivateLicense}>
                  Activer Pro
                </button>
              </>
            ) : (
              <button className="btn" onClick={onDeactivateLicense}>
                Désactiver la licence
              </button>
            )}
            <button className="btn" onClick={() => { void refreshLicense(); setPanel("main"); }}>
              Retour
            </button>
          </section>
        )}
      </main>
    </div>
  );
}

export default App;
