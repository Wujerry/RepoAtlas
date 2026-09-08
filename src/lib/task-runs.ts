import { api, onTaskExited, onTaskLog, onTaskPersistenceFailed } from "./api";
import type { LogChunk, TaskRun } from "../types";

export const MAX_RECENT_RUNS = 20;
export const MAX_TERMINALS = 4;

type Listener = () => void;
type LogListener = (log: string) => void;

type TaskRunState = {
  runs: TaskRun[];
  recent: TaskRun[];
  logs: Record<string, string>;
  logOffsets: Record<string, number>;
  selectedId?: string;
  layout: "tiles" | "focus";
  projectNames: Record<string, string>;
  inputOwner?: string;
};

const listeners = new Set<Listener>();
const metadataListeners = new Set<Listener>();
const logListeners = new Map<string, Set<LogListener>>();
const pendingLogChunks = new Map<string, string[]>();
let logFlushTimer: ReturnType<typeof setTimeout> | undefined;
let state: TaskRunState = {
  runs: [],
  recent: [],
  logs: {},
  logOffsets: {},
  layout: "tiles",
  projectNames: {},
};
let started = false;

function emit() {
  listeners.forEach((listener) => listener());
}

function emitMetadata() {
  metadataListeners.forEach((listener) => listener());
  emit();
}

function notifyLog(runId: string) {
  const log = state.logs[runId] ?? "";
  logListeners.get(runId)?.forEach((listener) => listener(log));
}

function flushPendingLogs() {
  logFlushTimer = undefined;
  if (pendingLogChunks.size === 0) return;
  const logs = { ...state.logs };
  const logOffsets = { ...state.logOffsets };
  const changed: string[] = [];
  pendingLogChunks.forEach((chunks, runId) => {
    const appended = chunks.join("");
    logOffsets[runId] = (logOffsets[runId] ?? logs[runId]?.length ?? 0) + appended.length;
    logs[runId] = ((logs[runId] ?? "") + appended).slice(-200000);
    changed.push(runId);
  });
  pendingLogChunks.clear();
  state = { ...state, logs, logOffsets };
  emit();
  changed.forEach(notifyLog);
}

function enqueueLogChunk(chunk: LogChunk) {
  const chunks = pendingLogChunks.get(chunk.runId) ?? [];
  chunks.push(chunk.text);
  pendingLogChunks.set(chunk.runId, chunks);
  if (logFlushTimer === undefined) {
    logFlushTimer = setTimeout(flushPendingLogs, 50);
  }
}

function upsertRun(run: TaskRun) {
  const running = run.status === "running" || run.status === "starting";
  state = {
    ...state,
    runs: running
      ? [run, ...state.runs.filter((item) => item.id !== run.id)].slice(0, MAX_TERMINALS)
      : state.runs.filter((item) => item.id !== run.id),
    recent: running
      ? state.recent.filter((item) => item.id !== run.id)
      : [run, ...state.recent.filter((item) => item.id !== run.id)].slice(0, MAX_RECENT_RUNS),
    selectedId: state.selectedId && (running || state.recent.some((item) => item.id === state.selectedId) || state.runs.some((item) => item.id === state.selectedId))
      ? state.selectedId
      : run.id,
  };
  const retained = new Set([...state.runs, ...state.recent].map((item) => item.id));
  state = {
    ...state,
    logs: Object.fromEntries(Object.entries(state.logs).filter(([id]) => retained.has(id))),
    logOffsets: Object.fromEntries(Object.entries(state.logOffsets).filter(([id]) => retained.has(id))),
  };
}

export function getTaskRunSnapshot(): TaskRunState {
  return state;
}

export function subscribeTaskRuns(listener: Listener) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function getActiveTaskRunsSnapshot(): TaskRun[] {
  return state.runs;
}

export function subscribeActiveTaskRuns(listener: Listener) {
  metadataListeners.add(listener);
  return () => metadataListeners.delete(listener);
}

export function getTaskLog(runId: string): string {
  return state.logs[runId] ?? "";
}

export function subscribeTaskLog(runId: string, listener: LogListener) {
  const listenersForRun = logListeners.get(runId) ?? new Set<LogListener>();
  listenersForRun.add(listener);
  logListeners.set(runId, listenersForRun);
  return () => {
    listenersForRun.delete(listener);
    if (listenersForRun.size === 0) logListeners.delete(runId);
  };
}

export function selectTaskRun(id?: string) {
  state = { ...state, selectedId: id, layout: id ? "focus" : "tiles" };
  emit();
}

export function setTaskLayout(layout: "tiles" | "focus") {
  state = { ...state, layout, selectedId: layout === "tiles" ? undefined : state.selectedId };
  emit();
}

export function setInputOwner(runId?: string) {
  state = { ...state, inputOwner: runId };
  emit();
}

export async function refreshTaskRuns(options: { hydrateLogs?: boolean; refreshProjectNames?: boolean } = {}) {
  const refreshNames = options.refreshProjectNames || Object.keys(state.projectNames).length === 0;
  const [runs, projects] = await Promise.all([
    api.listActiveTaskRuns(),
    refreshNames ? api.listProjects({ includeArchived: true }) : Promise.resolve(undefined),
  ]);
  const names = projects
    ? Object.fromEntries(projects.map((project) => [project.id, project.displayName]))
    : state.projectNames;
  const tails = options.hydrateLogs
    ? await Promise.all(runs.map(async (run) => [run.id, await api.readTaskLog(run.id)] as const))
    : [];
  state = {
    ...state,
    runs,
    projectNames: names,
    logs: { ...state.logs, ...Object.fromEntries(tails) },
    logOffsets: { ...state.logOffsets, ...Object.fromEntries(tails.map(([id, log]) => [id, log.length])) },
    selectedId: state.selectedId && runs.some((run) => run.id === state.selectedId) ? state.selectedId : runs[0]?.id ?? state.selectedId,
  };
  emitMetadata();
  tails.forEach(([runId]) => notifyLog(runId));
}

export function rememberStartedRun(run: TaskRun) {
  upsertRun(run);
  state = { ...state, logs: { ...state.logs, [run.id]: state.logs[run.id] ?? "" } };
  emitMetadata();
  notifyLog(run.id);
}

export async function openTaskRun(runId: string) {
  const [run, output] = await Promise.all([api.getTaskRun(runId), api.readTaskLog(runId)]);
  upsertRun(run);
  state = {
    ...state,
    recent: [run, ...state.recent.filter((item) => item.id !== run.id)].slice(0, MAX_RECENT_RUNS),
    logs: { ...state.logs, [run.id]: output },
    logOffsets: { ...state.logOffsets, [run.id]: output.length },
    selectedId: run.id,
    layout: "focus",
  };
  emitMetadata();
  notifyLog(run.id);
}

export async function startTaskRunListeners() {
  if (started) return;
  started = true;
  await Promise.all([
    onTaskLog((chunk: LogChunk) => {
      enqueueLogChunk(chunk);
    }),
    onTaskExited(async (run) => {
      flushPendingLogs();
      upsertRun(run);
      try {
        const output = await api.readTaskLog(run.id);
        state = { ...state, logs: { ...state.logs, [run.id]: output }, logOffsets: { ...state.logOffsets, [run.id]: output.length } };
      } catch {
        /* keep streamed log */
      }
      emitMetadata();
      notifyLog(run.id);
    }),
    onTaskPersistenceFailed((failure) => {
      if (failure.run) upsertRun(failure.run);
      emitMetadata();
    }),
  ]);
  await refreshTaskRuns({ hydrateLogs: true, refreshProjectNames: true });
}
