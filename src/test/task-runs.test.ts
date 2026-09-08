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
} from "../lib/task-runs";

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
