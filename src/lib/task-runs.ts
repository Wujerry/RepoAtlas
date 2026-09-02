import { api, onTaskExited, onTaskLog, onTaskPersistenceFailed } from "./api";
import type { LogChunk, TaskRun } from "../types";

export const MAX_RECENT_RUNS = 20;
export const MAX_TERMINALS = 4;

type Listener = () => void;

type TaskRunState = {
  runs: TaskRun[];
  recent: TaskRun[];
  logs: Record<string, string>;
  selectedId?: string;
  layout: "tiles" | "focus";
  projectNames: Record<string, string>;
  inputOwner?: string;
};

const listeners = new Set<Listener>();
let state: TaskRunState = {
  runs: [],
  recent: [],
  logs: {},
  layout: "tiles",
  projectNames: {},
};
let started = false;

function emit() {
  listeners.forEach((listener) => listener());
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
}

export function getTaskRunSnapshot(): TaskRunState {
  return state;
}

export function subscribeTaskRuns(listener: Listener) {
  listeners.add(listener);
  return () => listeners.delete(listener);
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

export async function refreshTaskRuns() {
  const [runs, projects] = await Promise.all([
    api.listActiveTaskRuns(),
    api.listProjects({ includeArchived: true }),
  ]);
  const names = Object.fromEntries(projects.map((project) => [project.id, project.displayName]));
  const tails = await Promise.all(runs.map(async (run) => [run.id, await api.readTaskLog(run.id)] as const));
  state = {
    ...state,
    runs,
    projectNames: names,
    logs: { ...state.logs, ...Object.fromEntries(tails) },
    selectedId: state.selectedId && runs.some((run) => run.id === state.selectedId) ? state.selectedId : runs[0]?.id ?? state.selectedId,
  };
  emit();
}

export function rememberStartedRun(run: TaskRun) {
  upsertRun(run);
  state = { ...state, logs: { ...state.logs, [run.id]: state.logs[run.id] ?? "" } };
  emit();
}

export async function startTaskRunListeners() {
  if (started) return;
  started = true;
  await Promise.all([
    onTaskLog((chunk: LogChunk) => {
      state = {
        ...state,
        logs: { ...state.logs, [chunk.runId]: ((state.logs[chunk.runId] ?? "") + chunk.text).slice(-200000) },
      };
      emit();
    }),
    onTaskExited(async (run) => {
      upsertRun(run);
      try {
        const output = await api.readTaskLog(run.id);
        state = { ...state, logs: { ...state.logs, [run.id]: output } };
      } catch {
        /* keep streamed log */
      }
      emit();
    }),
    onTaskPersistenceFailed((failure) => {
      if (failure.run) upsertRun(failure.run);
      emit();
    }),
  ]);
  await refreshTaskRuns();
}
