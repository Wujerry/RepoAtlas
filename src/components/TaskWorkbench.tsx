import { Play, Square, TerminalWindow, X } from "@phosphor-icons/react";
import { useEffect, useMemo, useState, useSyncExternalStore } from "react";
import { api, onTaskRuntime } from "../lib/api";
import type { MessageKey } from "../i18n";
import { formatCommand, statusKey } from "./DashboardShared";
import { Button } from "./ui/button";
import { TaskTerminal } from "./TaskTerminal";
import { getTaskRunSnapshot, MAX_TERMINALS, refreshTaskRuns, selectTaskRun, setTaskLayout, subscribeTaskRuns } from "../lib/task-runs";
import type { TaskRun, TaskRuntimeSnapshot } from "../types";

function formatBytes(value: number): string {
  if (value < 1024 * 1024) return `${Math.round(value / 1024)} KB`;
  if (value < 1024 * 1024 * 1024) return `${(value / 1024 / 1024).toFixed(1)} MB`;
  return `${(value / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

function elapsedLabel(startedAt: string | undefined, now: number): string {
  if (!startedAt) return "";
  const started = new Date(startedAt).getTime();
  if (!Number.isFinite(started)) return "";
  const totalSeconds = Math.max(0, Math.floor((now - started) / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  const mm = String(minutes).padStart(2, "0");
  const ss = String(seconds).padStart(2, "0");
  return hours > 0 ? `${hours}:${mm}:${ss}` : `${minutes}:${ss}`;
}

export function TaskWorkbench({
  open,
  t,
  theme,
  nowTick,
  stoppingRunId,
  stoppingAll,
  onClose,
  onStop,
  onStopAll,
  onJump,
  onPreviewError,
}: {
  open: boolean;
  t: (key: MessageKey) => string;
  theme: "dark" | "light";
  nowTick: number;
  stoppingRunId?: string;
  stoppingAll: boolean;
  onClose: () => void;
  onStop: (runId: string) => void;
  onStopAll: () => void;
  onJump: (projectId: string) => void;
  onPreviewError: (error: unknown) => void;
}) {
  const snapshot = useSyncExternalStore(subscribeTaskRuns, getTaskRunSnapshot, getTaskRunSnapshot);
  const [runtime, setRuntime] = useState<Record<string, TaskRuntimeSnapshot>>({});
  const [openingEndpoint, setOpeningEndpoint] = useState<string>();
  const visible = snapshot.layout === "focus" && snapshot.selectedId
    ? [snapshot.runs.find((run) => run.id === snapshot.selectedId) ?? snapshot.recent.find((run) => run.id === snapshot.selectedId)].filter(Boolean) as TaskRun[]
    : snapshot.runs.slice(0, MAX_TERMINALS);

  useEffect(() => {
    if (!open) return;
    void refreshTaskRuns({ hydrateLogs: true, refreshProjectNames: true });
  }, [open]);

  useEffect(() => {
    const unlisten = onTaskRuntime((next) => setRuntime((current) => ({ ...current, [next.runId]: next })));
    return () => { void unlisten.then((dispose) => dispose()); };
  }, []);

  useEffect(() => {
    if (!open || snapshot.runs.length === 0) return;
    let active = true;
    void api.getTaskRuntimeSnapshots(snapshot.runs.map((run) => run.id)).then((snapshots) => {
      if (!active) return;
      setRuntime((current) => ({
        ...current,
        ...Object.fromEntries(snapshots.map((snapshot) => [snapshot.runId, snapshot])),
      }));
    }).catch(() => undefined);
    return () => { active = false; };
  }, [open, snapshot.runs]);

  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.defaultPrevented) return;
      const target = event.target;
      if (target instanceof Element && target.closest('[role="dialog"], [role="alertdialog"], [role="menu"]')) return;
      event.preventDefault();
      onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose, open]);

  const gridClass = useMemo(() => {
    if (visible.length <= 1) return "is-one";
    if (visible.length === 2) return "is-two";
    if (visible.length === 3) return "is-three";
    return "is-four";
  }, [visible.length]);

  async function openPreview(runId: string, endpoint: string) {
    const key = `${runId}:${endpoint}`;
    setOpeningEndpoint(key);
    try {
      await api.openDevEndpoint(runId, endpoint);
    } catch (error) {
      onPreviewError(error);
    } finally {
      setOpeningEndpoint((current) => current === key ? undefined : current);
    }
  }

  if (!open) return null;

  return (
    <section className="task-workbench" aria-label={t("activeTasks")} aria-keyshortcuts={"Escape Control+`"}>
      <aside className="task-workbench-list">
        <header>
          <div>
            <p className="eyebrow">{t("activeTasks")}</p>
            <h2>{t("taskWorkbench")}</h2>
          </div>
          <Button size="icon" variant="quiet" aria-label={t("close")} title={`${t("close")} (Ctrl + \`)`} onClick={onClose}><X /></Button>
        </header>
        <div className="task-workbench-actions">
          <div className="task-layout-toggle" role="group" aria-label={t("outputLayout")}>
            <Button variant="quiet" aria-pressed={snapshot.layout === "focus"} onClick={() => selectTaskRun(snapshot.selectedId ?? snapshot.runs[0]?.id ?? snapshot.recent[0]?.id)}>{t("singleOutput")}</Button>
            <Button variant="quiet" aria-pressed={snapshot.layout === "tiles"} onClick={() => setTaskLayout("tiles")}>{t("tiledOutputs")}</Button>
          </div>
          {snapshot.runs.length > 0 && <Button variant="danger" loading={stoppingAll} onClick={onStopAll}>{t("stopAll")}</Button>}
        </div>
        <section>
          <h3>{t("running")}</h3>
          {snapshot.runs.length === 0 ? <p className="muted-copy">{t("noActiveTasks")}</p> : snapshot.runs.map((run) => (
            <button key={run.id} className={snapshot.selectedId === run.id && snapshot.layout === "focus" ? "active" : ""} onClick={() => selectTaskRun(run.id)}>
              <span className={"run-dot run-" + run.status} />
              <strong>{snapshot.projectNames[run.projectId] ?? run.projectId} · {run.kind}</strong>
              <span>{t(statusKey(run.status))} · {elapsedLabel(run.startedAt, nowTick)}</span>
            </button>
          ))}
        </section>
        <section>
          <h3>{t("recentActivity")}</h3>
          {snapshot.recent.length === 0 ? <p className="muted-copy">{t("noRunHistory")}</p> : snapshot.recent.map((run) => (
            <button key={run.id} className={snapshot.selectedId === run.id && snapshot.layout === "focus" ? "active" : ""} onClick={() => selectTaskRun(run.id)}>
              <span className={"run-dot run-" + run.status} />
              <strong>{snapshot.projectNames[run.projectId] ?? run.projectId} · {run.kind}</strong>
              <span>{t(statusKey(run.status))}</span>
            </button>
          ))}
        </section>
      </aside>
      <div className={"task-workbench-grid " + gridClass}>
          {visible.map((run) => (
            <article
              key={run.id}
              className={"task-terminal-pane" + (snapshot.selectedId === run.id ? " is-focused" : "")}
            >
              <header>
                <div>
                  <strong>{run.kind}</strong>
                  <span>{snapshot.projectNames[run.projectId] ?? run.projectId}</span>
                  <code>{formatCommand(run.executable, run.argv)}</code>
                </div>
                <div className="task-terminal-pane-actions">
                  <Button variant="quiet" onClick={() => onJump(run.projectId)}>{t("jumpToProject")}</Button>
                  {(run.status === "running" || run.status === "starting") && <Button variant="danger" loading={stoppingRunId === run.id} onClick={() => onStop(run.id)}><Square weight="fill" />{t("stop")}</Button>}
                </div>
              </header>
              {runtime[run.id] && <div className="task-runtime-strip" aria-label={t("runtimeMonitor")}>
                <span className="runtime-cpu"><b>{t("cpuUsage")}</b>{runtime[run.id].cpuPercent.toFixed(1)}%</span>
                <span className="runtime-cpu"><b>{t("peakCpuUsage")}</b>{runtime[run.id].peakCpuPercent.toFixed(1)}%</span>
                <span className="runtime-memory"><b>{t("memoryUsage")}</b>{formatBytes(runtime[run.id].memoryBytes)}</span>
                <span className="runtime-memory"><b>{t("peakMemoryUsage")}</b>{formatBytes(runtime[run.id].peakMemoryBytes)}</span>
                <span className="runtime-process"><b>{t("processCount")}</b>{runtime[run.id].processCount}</span>
                <span className="runtime-ports"><b>{t("listeningPorts")}</b>{!runtime[run.id].portInspectionAvailable ? t("portInspectionUnavailable") : runtime[run.id].ports.length ? runtime[run.id].ports.join(", ") : t("noListeningPorts")}</span>
                {runtime[run.id].conflicts.length > 0 && <span className="runtime-conflict"><b>{t("portConflicts")}</b>{runtime[run.id].conflicts.map((item) => item.port).join(", ")}</span>}
                {runtime[run.id].endpoints.map((endpoint) => <Button key={endpoint} size="sm" variant="quiet" loading={openingEndpoint === `${run.id}:${endpoint}`} onClick={() => void openPreview(run.id, endpoint)}>{t("openPreview")} · {endpoint}</Button>)}
              </div>}
              <TaskTerminal run={run} log={snapshot.logs[run.id] ?? ""} interactive={snapshot.inputOwner === run.id || visible.length === 1} theme={theme} />
            </article>
          ))}
        {visible.length === 0 && (
          <div className="task-workbench-empty">
            <span className="task-workbench-empty-mark" aria-hidden="true">
              <TerminalWindow weight="duotone" />
              <Play weight="fill" />
            </span>
            <h3>{t("taskWorkbenchEmptyTitle")}</h3>
            <p>{t("taskWorkbenchEmptyBody")}</p>
            <span className="task-workbench-empty-hint"><kbd>Esc</kbd>{t("taskWorkbenchEscHint")}</span>
          </div>
        )}
      </div>
    </section>
  );
}
