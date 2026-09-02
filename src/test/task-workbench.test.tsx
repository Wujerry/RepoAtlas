import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { TaskWorkbench } from "../components/TaskWorkbench";
import { dictionaries, type MessageKey } from "../i18n";
import { selectTaskRun, setTaskLayout } from "../lib/task-runs";

const t = (key: MessageKey) => dictionaries.en[key];

const mockState = vi.hoisted(() => ({ empty: false }));

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
  });

  it("shows the run list and live output together", () => {
    render(
      <TaskWorkbench
        open
        t={t}
        nowTick={Date.parse("2026-09-01T00:01:00Z")}
        stoppingAll={false}
        onClose={vi.fn()}
        onStop={vi.fn()}
        onStopAll={vi.fn()}
        onJump={vi.fn()}
      />,
    );
    expect(screen.getByText("Task workbench")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Atlas/ })).toBeInTheDocument();
    expect(screen.getByText("listening on 5173")).toBeInTheDocument();
  });

  it("can switch from tiled output to a focused run", () => {
    render(
      <TaskWorkbench
        open
        t={t}
        nowTick={Date.parse("2026-09-01T00:01:00Z")}
        stoppingAll={false}
        onClose={vi.fn()}
        onStop={vi.fn()}
        onStopAll={vi.fn()}
        onJump={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /Atlas/ }));
    expect(selectTaskRun).toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: t("allOutputs") }));
    expect(setTaskLayout).toHaveBeenCalledWith("tiles");
  });

  it("closes the workbench when Escape is pressed", () => {
    const onClose = vi.fn();
    render(
      <TaskWorkbench
        open
        t={t}
        nowTick={Date.parse("2026-09-01T00:01:00Z")}
        stoppingAll={false}
        onClose={onClose}
        onStop={vi.fn()}
        onStopAll={vi.fn()}
        onJump={vi.fn()}
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
        nowTick={Date.parse("2026-09-01T00:01:00Z")}
        stoppingAll={false}
        onClose={vi.fn()}
        onStop={vi.fn()}
        onStopAll={vi.fn()}
        onJump={vi.fn()}
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
        nowTick={Date.parse("2026-09-01T00:01:00Z")}
        stoppingAll={false}
        onClose={vi.fn()}
        onStop={vi.fn()}
        onStopAll={vi.fn()}
        onJump={vi.fn()}
      />,
    );
    expect(screen.getByText(t("taskWorkbenchEmptyTitle"))).toBeInTheDocument();
    expect(screen.getByText(t("taskWorkbenchEmptyBody"))).toBeInTheDocument();
    expect(screen.getByText("Esc")).toBeInTheDocument();
  });
});
