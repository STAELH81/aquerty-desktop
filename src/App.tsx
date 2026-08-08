import { useEffect, useState, useTransition } from "react";
import { listen } from "@tauri-apps/api/event";
import { enable as enableAutostart, disable as disableAutostart } from "@tauri-apps/plugin-autostart";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
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
import {
  ACCENT_OPTIONS,
  ACTION_OPTIONS,
  emptyConditions,
  formatCountdown,
  hasConditions,
  parseDurationClient,
  playBeep,
  type AlertPayload,
  type AppSettings,
  type LicenseInfo,
  type PowerAction,
  type Profile,
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
  const parsedSeconds = parseDurationClient(durationInput);
  const active = schedule?.active ?? false;
  const remaining = schedule?.remainingSeconds ?? 0;
  const lastMinute = active && !schedule?.waitingForConditions && remaining <= 60;

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
        document.documentElement.style.setProperty("--accent", s.accent || "#e2a84a");

        cleanups.push(
          await listen<ScheduleSnapshot>("schedule-updated", (event) => {
            startTransition(() => setSchedule(event.payload));
          }),
        );
        cleanups.push(
          await listen<AppSettings>("settings-updated", (event) => {
            setSettings(event.payload);
            document.documentElement.style.setProperty(
              "--accent",
              event.payload.accent || "#e2a84a",
            );
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

  async function persist(partial: Partial<AppSettings>) {
    if (!settings) return;
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
      setError("Version gratuite : 4 presets max.");
      setPanel("license");
      return;
    }
    if (parseDurationClient(newPreset) == null) {
      setError("Preset invalide.");
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
      setError("Version gratuite : 3 profils max.");
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

  async function onActivateLicense() {
    setError(null);
    try {
      setLicense(await activateLicense(licenseInput));
      setLicenseInput("");
      setPanel("main");
    } catch (e) {
      setError(String(e));
    }
  }

  async function checkForUpdates() {
    setUpdateMsg("Vérification…");
    try {
      const update = await check();
      if (!update) {
        setUpdateMsg("Déjà à jour.");
        return;
      }
      setUpdateMsg(`Mise à jour ${update.version}…`);
      await update.downloadAndInstall();
      setUpdateMsg("Installée — redémarrage…");
      await relaunch();
    } catch (e) {
      setUpdateMsg(
        "Updater pas encore branché (clé de signature / release). " + String(e),
      );
    }
  }

  const filteredProcesses = processes.filter((p) =>
    p.toLowerCase().includes(processFilter.toLowerCase()),
  );

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

                {(settings?.profiles?.length ?? 0) > 0 && (
                  <div className="profiles" role="group" aria-label="Profils">
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

                <div className="row tight">
                  <button type="button" className="linkish" onClick={openProcessPicker}>
                    Fin de process…
                  </button>
                  <button
                    type="button"
                    className="linkish"
                    onClick={() => setShowConditions((v) => !v)}
                  >
                    {showConditions ? "Masquer conditions" : "Conditions"}
                    {!isPro ? " (Pro)" : ""}
                  </button>
                </div>

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
                  Continuer
                </button>
              </>
            )}

            <nav className="footer-nav">
              <button type="button" className="ghost" onClick={() => setPanel("settings")}>
                Réglages
              </button>
              <button type="button" className="ghost" onClick={() => setPanel("history")}>
                Historique
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

        {panel === "processes" && (
          <section className="hero enter panel">
            <p className="eyebrow">Fin de process</p>
            <h2 className="confirm-title">Quand cette app se ferme</h2>
            <input
              className="time-input small"
              value={processFilter}
              onChange={(e) => setProcessFilter(e.target.value)}
              placeholder="Filtrer…"
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
              Retour
            </button>
          </section>
        )}

        {panel === "history" && settings && (
          <section className="hero enter panel">
            <p className="eyebrow">Historique</p>
            {settings.history.length === 0 ? (
              <p className="hint">Aucune action pour l’instant.</p>
            ) : (
              <ul className="history-list">
                {settings.history.map((h, i) => (
                  <li key={`${h.atUnix}-${i}`}>
                    <strong>{h.actionLabel}</strong>
                    <span>
                      {h.durationLabel}
                      {h.cancelled ? " · annulé" : ""}
                    </span>
                    <em>{new Date(h.atUnix * 1000).toLocaleString()}</em>
                  </li>
                ))}
              </ul>
            )}
            <div className="row">
              <button
                className="btn"
                onClick={async () => setSettings(await clearHistory())}
              >
                Vider
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
            <label className="toggle">
              <input
                type="checkbox"
                checked={settings.widgetEnabled}
                onChange={(e) => void persist({ widgetEnabled: e.target.checked })}
              />
              Mini-widget countdown (si timer actif)
            </label>
            <label className="toggle">
              <input
                type="checkbox"
                checked={settings.soundEnabled}
                onChange={(e) => void persist({ soundEnabled: e.target.checked })}
              />
              Sons d’alerte
            </label>
            <label className="toggle">
              <input
                type="checkbox"
                checked={settings.notifyAt5m}
                onChange={(e) => void persist({ notifyAt5m: e.target.checked })}
              />
              Notif à 5 minutes
            </label>
            <label className="toggle">
              <input
                type="checkbox"
                checked={settings.notifyAt1m}
                onChange={(e) => void persist({ notifyAt1m: e.target.checked })}
              />
              Notif à 1 minute
            </label>

            <p className="hint">Accent</p>
            <div className="presets">
              {ACCENT_OPTIONS.map((a) => (
                <button
                  key={a.id}
                  type="button"
                  className={`chip ${settings.accent === a.id ? "active" : ""}`}
                  style={{ borderColor: a.id }}
                  onClick={() => void persist({ accent: a.id })}
                >
                  {a.label}
                </button>
              ))}
            </div>

            <label>
              Hotkey ouvrir
              <input
                value={settings.hotkeyOpen}
                onChange={(e) => void persist({ hotkeyOpen: e.target.value })}
              />
            </label>
            <label>
              Hotkey annuler
              <input
                value={settings.hotkeyCancel}
                onChange={(e) => void persist({ hotkeyCancel: e.target.value })}
              />
            </label>

            <div className="preset-edit">
              <p className="hint">Presets durée</p>
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

            <div className="preset-edit">
              <p className="hint">Sauver le setup actuel en profil</p>
              <div className="row">
                <input
                  value={profileName}
                  onChange={(e) => setProfileName(e.target.value)}
                  placeholder="Fin de série"
                />
                <button className="btn" type="button" onClick={saveCurrentAsProfile}>
                  Sauver
                </button>
              </div>
              <div className="presets">
                {settings.profiles.map((p) => (
                  <button
                    key={p.id}
                    type="button"
                    className="chip"
                    onClick={() => void removeProfile(p.id)}
                  >
                    {p.name} ×
                  </button>
                ))}
              </div>
            </div>

            <button className="btn" type="button" onClick={checkForUpdates}>
              Vérifier les mises à jour
            </button>
            {updateMsg && <p className="hint">{updateMsg}</p>}

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
              Gratuit : arrêt, redémarrage, 4 presets, 3 profils.
              <br />
              Pro : veille, hibernation, verrouillage, conditions, plus de profils.
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
              <button
                className="btn"
                onClick={async () => setLicense(await deactivateLicense())}
              >
                Désactiver la licence
              </button>
            )}
            <button className="btn" onClick={() => setPanel("main")}>
              Retour
            </button>
          </section>
        )}
      </main>
    </div>
  );
}

export default App;
