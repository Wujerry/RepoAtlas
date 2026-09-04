import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { TaskWorkbench } from "../components/TaskWorkbench";
import { dictionaries, type MessageKey } from "../i18n";
import { selectTaskRun, setTaskLayout } from "../lib/task-runs";

const t = (key: MessageKey) => dictionaries.en[key];

const mockState = vi.hoisted(() => ({ empty: false }));
const getTaskRuntimeSnapshots = vi.hoisted(() => vi.fn(async () => [{
  runId: "run-1",
  capturedAt: "2026-09-01T00:01:00Z",
  cpuPercent: 12.5,
  peakCpuPercent: 14,
  memoryBytes: 64 * 1024 * 1024,
  peakMemoryBytes: 72 * 1024 * 1024,
  processCount: 3,
  ports: [5173],
  portInspectionAvailable: true,
  endpoints: ["http://localhost:5173/"],
  conflicts: [],
}]));
const openDevEndpoint = vi.hoisted(() => vi.fn(async () => undefined));

vi.mock("../lib/api", () => ({
  api: { getTaskRuntimeSnapshots, openDevEndpoint },
  onTaskRuntime: vi.fn(async () => () => undefined),
}));

vi.mock("../lib/task-runs", async () => {
  const actual = await vi.importActual<typeof import("../lib/task-runs")>("../lib/task-runs");
  const run = {
    id: "run-1",
    projectId: "project-1",
    taskId: "task-1",
    kind: "dev",
    executable: "pnpm",
    argv: ["dev"],
    cwd: "C:\\code\\atlas",
    shellMode: false,
    status: "running",
    exitCode: null,
    logPath: "log.txt",
    startedAt: "2026-09-01T00:00:00Z",
    finishedAt: null,
  };
  const snapshot = {
    runs: [run],
    recent: [],
    logs: { "run-1": "listening on 5173" },
    selectedId: undefined,
    layout: "tiles" as const,
    projectNames: { "project-1": "Atlas" },
    inputOwner: undefined,
  };
  const emptySnapshot = {
    runs: [],
    recent: [],
    logs: {},
    selectedId: undefined,
    layout: "tiles" as const,
    projectNames: {},
    inputOwner: undefined,
  };
  return {
    ...actual,
    getTaskRunSnapshot: () => (mockState.empty ? emptySnapshot : snapshot),
    subscribeTaskRuns: (listener: () => void) => {
      listener();
      return () => undefined;
    },
    refreshTaskRuns: vi.fn(async () => undefined),
    selectTaskRun: vi.fn(),
    setTaskLayout: vi.fn(),
  };
});

describe("TaskWorkbench", () => {
  afterEach(() => {
    mockState.empty = false;
    vi.clearAllMocks();
    openDevEndpoint.mockResolvedValue(undefined);
  });

  it("shows the run list, runtime observation, preview, and live output together", async () => {
    render(
      <TaskWorkbench
        open
        t={t}
        theme="light"
        nowTick={Date.parse("2026-09-01T00:01:00Z")}
        stoppingAll={false}
        onClose={vi.fn()}
        onStop={vi.fn()}
        onStopAll={vi.fn()}
        onJump={vi.fn()}
        onPreviewError={vi.fn()}
      />,
    );
    expect(screen.getByText("Task workbench")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Atlas/ })).toBeInTheDocument();
    expect(screen.getByText("listening on 5173")).toBeInTheDocument();
    expect(await screen.findByText("12.5%")).toBeInTheDocument();
    expect(screen.getByText("64.0 MB")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /http:\/\/localhost:5173/ })).toBeInTheDocument();
    expect(document.querySelector(".log-pane--light")).not.toBeNull();
  });

  it("can switch from tiled output to a focused run", () => {
    render(
      <TaskWorkbench
        open
        t={t}
        theme="dark"
        nowTick={Date.parse("2026-09-01T00:01:00Z")}
        stoppingAll={false}
        onClose={vi.fn()}
        onStop={vi.fn()}
        onStopAll={vi.fn()}
        onJump={vi.fn()}
        onPreviewError={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /Atlas/ }));
    expect(selectTaskRun).toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: t("tiledOutputs") }));
    expect(setTaskLayout).toHaveBeenCalledWith("tiles");
    fireEvent.click(screen.getByRole("button", { name: t("singleOutput") }));
    expect(selectTaskRun).toHaveBeenCalled();
  });

  it("reports preview launch failures instead of swallowing them", async () => {
    const onPreviewError = vi.fn();
    const error = new Error("browser unavailable");
    openDevEndpoint.mockRejectedValueOnce(error);
    render(
      <TaskWorkbench
        open
        t={t}
        theme="dark"
        nowTick={Date.parse("2026-09-01T00:01:00Z")}
        stoppingAll={false}
        onClose={vi.fn()}
        onStop={vi.fn()}
        onStopAll={vi.fn()}
        onJump={vi.fn()}
        onPreviewError={onPreviewError}
      />,
    );

    fireEvent.click(await screen.findByRole("button", { name: /http:\/\/localhost:5173/ }));
    await waitFor(() => expect(onPreviewError).toHaveBeenCalledWith(error));
  });

  it("closes the workbench when Escape is pressed", () => {
    const onClose = vi.fn();
    render(
      <TaskWorkbench
        open
        t={t}
        theme="dark"
        nowTick={Date.parse("2026-09-01T00:01:00Z")}
        stoppingAll={false}
        onClose={onClose}
        onStop={vi.fn()}
        onStopAll={vi.fn()}
        onJump={vi.fn()}
        onPreviewError={vi.fn()}
      />,
    );
    fireEvent.keyDown(screen.getByLabelText(t("activeTasks")), { key: "Escape" });
    expect(onClose).toHaveBeenCalled();
  });

  it("aligns pane actions on one row", () => {
    render(
      <TaskWorkbench
        open
        t={t}
        theme="dark"
        nowTick={Date.parse("2026-09-01T00:01:00Z")}
        stoppingAll={false}
        onClose={vi.fn()}
        onStop={vi.fn()}
        onStopAll={vi.fn()}
        onJump={vi.fn()}
        onPreviewError={vi.fn()}
      />,
    );
    const actions = document.querySelector(".task-terminal-pane-actions");
    expect(actions).not.toBeNull();
    expect(actions).toContainElement(screen.getByRole("button", { name: t("jumpToProject") }));
    expect(actions).toContainElement(screen.getByRole("button", { name: t("stop") }));
  });

  it("shows a designed empty state when no terminals are open", () => {
    mockState.empty = true;
    render(
      <TaskWorkbench
        open
        t={t}
        theme="dark"
        nowTick={Date.parse("2026-09-01T00:01:00Z")}
        stoppingAll={false}
        onClose={vi.fn()}
        onStop={vi.fn()}
        onStopAll={vi.fn()}
        onJump={vi.fn()}
        onPreviewError={vi.fn()}
      />,
    );
    expect(screen.getByText(t("taskWorkbenchEmptyTitle"))).toBeInTheDocument();
    expect(screen.getByText(t("taskWorkbenchEmptyBody"))).toBeInTheDocument();
    expect(screen.getByText("Esc")).toBeInTheDocument();
  });
});
