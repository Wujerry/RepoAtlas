import { beforeEach, describe, expect, it, vi } from "vitest";
import type { LogChunk, TaskRun } from "../types";

const handlers = vi.hoisted(() => ({
  log: undefined as ((chunk: LogChunk) => void) | undefined,
  exit: undefined as ((run: TaskRun) => void) | undefined,
}));

vi.mock("../lib/api", () => ({
  api: {
    listActiveTaskRuns: vi.fn(async () => []),
    listProjects: vi.fn(async () => []),
    readTaskLog: vi.fn(async () => ""),
    getTaskRun: vi.fn(),
  },
  onTaskLog: vi.fn(async (handler: (chunk: LogChunk) => void) => {
    handlers.log = handler;
    return () => undefined;
  }),
  onTaskExited: vi.fn(async (handler: (run: TaskRun) => void) => {
    handlers.exit = handler;
    return () => undefined;
  }),
  onTaskPersistenceFailed: vi.fn(async () => () => undefined),
}));

import {
  getTaskRunSnapshot,
  startTaskRunListeners,
  subscribeActiveTaskRuns,
  subscribeTaskRuns,
  rememberStartedRun,
  refreshTaskRuns,
  openTaskRun,
} from "../lib/task-runs";
import { api } from "../lib/api";

function taskRun(id: string): TaskRun {
  return { id, projectId: "project", taskId: null, kind: "build", executable: "pnpm", argv: ["build"], cwd: "C:/project", shellMode: false, status: "running", exitCode: null, logPath: "build.log", startedAt: "2026-09-15T00:00:00Z", finishedAt: null };
}

it("publishes completion before the final log read and rejects a late start response", async () => {
  await startTaskRunListeners();
  const run = taskRun("fast-build");
  rememberStartedRun(run);
  let resolveLog!: (text: string) => void;
  vi.mocked(api.readTaskLog).mockImplementationOnce(() => new Promise((resolve) => { resolveLog = resolve; }));
  const listener = vi.fn();
  const unsubscribe = subscribeActiveTaskRuns(listener);
  const completion = handlers.exit?.({ ...run, status: "succeeded", exitCode: 0 });
  expect(listener).toHaveBeenCalled();
  expect(getTaskRunSnapshot().runs.some((item) => item.id === run.id)).toBe(false);
  rememberStartedRun(run);
  expect(getTaskRunSnapshot().runs.some((item) => item.id === run.id)).toBe(false);
  resolveLog("done");
  await completion;
  expect(getTaskRunSnapshot().recent.find((item) => item.id === run.id)?.status).toBe("succeeded");
  unsubscribe();
});

it("does not restore a completed task from an older refresh", async () => {
  await startTaskRunListeners();
  const run = taskRun("refresh-race");
  rememberStartedRun(run);
  let resolveRuns!: (runs: TaskRun[]) => void;
  vi.mocked(api.listActiveTaskRuns).mockImplementationOnce(() => new Promise<TaskRun[]>((resolve) => { resolveRuns = resolve; }));
  const refresh = refreshTaskRuns();
  await handlers.exit?.({ ...run, status: "failed", exitCode: 7 });
  resolveRuns([run]);
  await refresh;
  expect(getTaskRunSnapshot().runs.some((item) => item.id === run.id)).toBe(false);
});

it("keeps an inspected running task active during refresh", async () => {
  const run = taskRun("inspected-running");
  vi.mocked(api.getTaskRun).mockResolvedValueOnce(run);
  await openTaskRun(run.id);
  vi.mocked(api.listActiveTaskRuns).mockResolvedValueOnce([run]);
  await refreshTaskRuns();
  expect(getTaskRunSnapshot().runs).toContainEqual(run);
});

describe("task run event store", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  it("batches output chunks without notifying metadata subscribers", async () => {
    await startTaskRunListeners();
    const fullListener = vi.fn();
    const metadataListener = vi.fn();
    const unsubscribeFull = subscribeTaskRuns(fullListener);
    const unsubscribeMetadata = subscribeActiveTaskRuns(metadataListener);

    for (let index = 0; index < 100; index += 1) {
      handlers.log?.({ runId: "run-1", stream: "pty", text: "x" });
    }
    expect(fullListener).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(50);

    expect(fullListener).toHaveBeenCalledTimes(1);
    expect(metadataListener).not.toHaveBeenCalled();
    expect(getTaskRunSnapshot().logs["run-1"]).toHaveLength(100);
    unsubscribeFull();
    unsubscribeMetadata();
    vi.useRealTimers();
  });
});

it("advances the cumulative offset when a capped log has identical content", async () => {
  vi.useFakeTimers();
  await startTaskRunListeners();
  handlers.log?.({ runId: "capped", stream: "pty", text: "x".repeat(200000) });
  await vi.advanceTimersByTimeAsync(50);
  handlers.log?.({ runId: "capped", stream: "pty", text: "xxx" });
  await vi.advanceTimersByTimeAsync(50);
  expect(getTaskRunSnapshot().logs.capped).toHaveLength(200000);
  expect(getTaskRunSnapshot().logOffsets.capped).toBe(200003);
  vi.useRealTimers();
});
