import React, { useEffect, useState } from "react";
import ReactDOM from "react-dom/client";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { formatCountdown, type ScheduleSnapshot } from "./types";
import { getSchedule } from "./api";

function Widget() {
  const [snap, setSnap] = useState<ScheduleSnapshot | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      setSnap(await getSchedule());
      unlisten = await listen<ScheduleSnapshot>("schedule-updated", (e) => {
        setSnap(e.payload);
      });
    })();
    return () => unlisten?.();
  }, []);

  const label = snap?.waitingForConditions
    ? "En attente"
    : snap?.actionLabel || "Aquerty";
  const time = formatCountdown(snap?.remainingSeconds ?? 0);

  return (
    <div
      className="wrap"
      onMouseDown={() => {
        void getCurrentWindow().startDragging();
      }}
    >
      <div className="label">{snap?.active ? label : "Inactif"}</div>
      <div className="time">{snap?.active ? time : "--:--"}</div>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Widget />
  </React.StrictMode>,
);
