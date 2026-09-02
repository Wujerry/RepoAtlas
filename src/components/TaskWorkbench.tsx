import { AnimatePresence, motion } from "framer-motion";
import { Play, Square, TerminalWindow, X } from "@phosphor-icons/react";
import { useEffect, useMemo, useSyncExternalStore } from "react";
import type { MessageKey } from "../i18n";
import { formatCommand, statusKey } from "./DashboardShared";
import { Button } from "./ui/button";
import { TaskTerminal } from "./TaskTerminal";
import { getTaskRunSnapshot, MAX_TERMINALS, refreshTaskRuns, selectTaskRun, setTaskLayout, subscribeTaskRuns } from "../lib/task-runs";
import { fadeMotion, MOTION, MOTION_EASE } from "../lib/motion";
import type { TaskRun } from "../types";

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
  nowTick,
  stoppingRunId,
  stoppingAll,
  onClose,
  onStop,
  onStopAll,
  onJump,
}: {
  open: boolean;
  t: (key: MessageKey) => string;
  nowTick: number;
  stoppingRunId?: string;
  stoppingAll: boolean;
  onClose: () => void;
  onStop: (runId: string) => void;
  onStopAll: () => void;
  onJump: (projectId: string) => void;
}) {
  const snapshot = useSyncExternalStore(subscribeTaskRuns, getTaskRunSnapshot, getTaskRunSnapshot);
  const visible = snapshot.layout === "focus" && snapshot.selectedId
    ? [snapshot.runs.find((run) => run.id === snapshot.selectedId) ?? snapshot.recent.find((run) => run.id === snapshot.selectedId)].filter(Boolean) as TaskRun[]
    : snapshot.runs.slice(0, MAX_TERMINALS);

  useEffect(() => {
    if (!open) return;
    void refreshTaskRuns();
  }, [open]);

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

  if (!open) return null;

  return (
    <motion.section className="task-workbench" {...fadeMotion} aria-label={t("activeTasks")} aria-keyshortcuts="Escape">
      <aside className="task-workbench-list">
        <header>
          <div>
            <p className="eyebrow">{t("activeTasks")}</p>
            <h2>{t("taskWorkbench")}</h2>
          </div>
          <Button size="icon" variant="quiet" aria-label={t("close")} title={`${t("close")} (Esc)`} onClick={onClose}><X /></Button>
        </header>
        <div className="task-workbench-actions">
          <Button variant="quiet" onClick={() => setTaskLayout("tiles")}>{t("allOutputs")}</Button>
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
        <AnimatePresence initial={false}>
          {visible.map((run) => (
            <motion.article
              key={run.id}
              className={"task-terminal-pane" + (snapshot.selectedId === run.id ? " is-focused" : "")}
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: MOTION.press, ease: MOTION_EASE }}
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
              <TaskTerminal run={run} log={snapshot.logs[run.id] ?? ""} interactive={snapshot.inputOwner === run.id || visible.length === 1} />
            </motion.article>
          ))}
        </AnimatePresence>
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
    </motion.section>
  );
}
