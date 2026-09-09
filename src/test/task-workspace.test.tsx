import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { dictionaries, type MessageKey } from "../i18n";
import type { TaskDefinition, TaskRun } from "../types";
import { describe, expect, it, vi } from "vitest";
import { TaskWorkspace } from "../components/Dashboard";

const t = (key: MessageKey) => dictionaries.en[key];

function task(id: string): TaskDefinition {
  return { id, kind: "dev", name: "dev", description: "Start the local Vite server", executable: "pnpm", argv: ["run", "dev"], cwd: null, inferred: true, shellMode: false };
}

function run(status: string): TaskRun {
  return {
    id: "run-1",
    projectId: "project-1",
    taskId: "task-1",
    kind: "dev",
    executable: "pnpm",
    argv: ["run", "dev"],
    cwd: "C:\\code\\atlas",
    shellMode: false,
    status,
    exitCode: status === "running" ? null : 0,
    logPath: "log.txt",
    startedAt: "2026-08-24T00:00:00Z",
    finishedAt: status === "running" ? null : "2026-08-24T00:01:00Z",
  };
}

function renderWorkspace(overrides: Partial<React.ComponentProps<typeof TaskWorkspace>> = {}) {
  return render(
    <TaskWorkspace
      tasks={[task("task-1")]}
      runs={[]}
      loading={false}
      log=""
      t={t}
      notify={vi.fn()}
      onSaveTasks={vi.fn()}
      onRun={vi.fn()}
      onStopRun={vi.fn()}
      onClearLog={vi.fn()}
      onHistory={vi.fn()}
      {...overrides}
    />,
  );
}

describe("TaskWorkspace console", () => {
  it("stays collapsed until a task is running", () => {
    renderWorkspace();
    expect(screen.getByLabelText("Task console")).toHaveClass("is-collapsed");
    expect(screen.queryByText("listening on 5173")).not.toBeInTheDocument();
  });

  it("edits a task in place even when it is deep in a long task list", async () => {
    const onSaveTasks = vi.fn().mockResolvedValue(undefined);
    const tasks = Array.from({ length: 14 }, (_, index) => ({ ...task(`task-${index}`), name: `dev-${index}` }));
    renderWorkspace({ tasks, onSaveTasks });

    const editButtons = screen.getAllByRole("button", { name: t("edit") });
    const targetCard = editButtons[10].closest("article");
    expect(targetCard).not.toBeNull();
    fireEvent.click(editButtons[10]);

    const editor = within(targetCard!).getByRole("form", { name: t("editTask") });
    expect(editor).toHaveClass("task-editor-inline");
    expect(targetCard).toContainElement(editor);
    expect(document.querySelector(".task-catalog > .task-editor")).not.toBeInTheDocument();

    const nameInput = within(editor).getByRole("textbox", { name: t("taskName") });
    await waitFor(() => expect(nameInput).toHaveFocus());
    fireEvent.change(nameInput, { target: { value: "dev-updated" } });
    fireEvent.click(within(editor).getByRole("button", { name: t("save") }));

    await waitFor(() => expect(onSaveTasks).toHaveBeenCalledTimes(1));
    const saved = onSaveTasks.mock.calls[0][0] as TaskDefinition[];
    expect(saved).toHaveLength(tasks.length);
    expect(saved.map((item) => item.id)).toEqual(tasks.map((item) => item.id));
    expect(saved[10].name).toBe("dev-updated");
    await waitFor(() => expect(within(targetCard!).getByRole("button", { name: t("edit") })).toHaveFocus());
  });

  it("keeps each task argument intact when it contains spaces", async () => {
    const onSaveTasks = vi.fn().mockResolvedValue(undefined);
    renderWorkspace({ onSaveTasks, tasks: [{ ...task("task-1"), argv: ["run", "dev"] }] });

    fireEvent.click(screen.getByRole("button", { name: t("edit") }));
    const editor = await screen.findByRole("form", { name: t("editTask") });
    fireEvent.change(within(editor).getByRole("textbox", { name: `${t("argumentsLabel")} 2` }), { target: { value: "hello world" } });
    fireEvent.click(within(editor).getByRole("button", { name: t("save") }));

    await waitFor(() => expect(onSaveTasks).toHaveBeenCalledTimes(1));
    expect((onSaveTasks.mock.calls[0][0] as TaskDefinition[])[0].argv).toEqual(["run", "hello world"]);
  });

  it("uses the detected project tool without guessing new task arguments", async () => {
    renderWorkspace({ tasks: [{ ...task("task-1"), executable: "yarn", argv: ["dev"] }] });
    fireEvent.click(screen.getByRole("button", { name: t("addTask") }));
    const editor = await screen.findByRole("form", { name: t("addTask") });
    expect(within(editor).getByRole("textbox", { name: t("executable") })).toHaveValue("yarn");
    expect(within(editor).queryAllByRole("textbox", { name: /Argument/u })).toHaveLength(0);
  });

  it("opens after run and can stop the live task", async () => {
    const onStopRun = vi.fn();
    const { rerender } = renderWorkspace({ onStopRun });
    expect(screen.getByLabelText("Task console")).toHaveClass("is-collapsed");

    rerender(
      <TaskWorkspace
      tasks={[task("task-1")]}
      runs={[run("running")]}
      loading={false}
      activeRunId="run-1"
        log="listening on 5173"
        t={t}
        notify={vi.fn()}
        onSaveTasks={vi.fn()}
        onRun={vi.fn()}
        onStopRun={onStopRun}
        onClearLog={vi.fn()}
        onHistory={vi.fn()}
      />,
    );

    expect(screen.getByLabelText("Task console")).toHaveClass("is-open");
    expect(screen.getByText("listening on 5173")).toBeInTheDocument();
    expect(screen.queryByLabelText(t("taskInput"))).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: t("stop") }));
    expect(onStopRun).toHaveBeenCalledWith("run-1");
  });
});

describe("TaskWorkspace task locating", () => {
  function namedTask(id: string, name: string, kind: string, inferred = true): TaskDefinition {
    return { ...task(id), name, kind, inferred };
  }

  function runFor(taskId: string, kind: string, status: string, startedAt: string, index = 0): TaskRun {
    return { ...run(status), id: `run-${taskId}-${index}`, taskId, kind, startedAt };
  }

  it("searches tasks and shows the filtered count", () => {
    const tasks = [namedTask("task-1", "build", "build"), namedTask("task-2", "lint", "check"), namedTask("task-3", "dev", "dev")];
    const { container } = renderWorkspace({ tasks });
    fireEvent.change(screen.getByPlaceholderText(t("taskSearchHint")), { target: { value: "lint" } });
    expect(screen.getByTitle(t("taskResults"))).toHaveTextContent("1 / 3");
    expect(container.querySelectorAll(".task-card")).toHaveLength(1);
    expect(container.querySelector(".task-card")).toHaveTextContent("lint");

    fireEvent.change(screen.getByPlaceholderText(t("taskSearchHint")), { target: { value: "nothing-matches" } });
    expect(screen.getByText(t("noTaskMatches"))).toBeInTheDocument();
    fireEvent.click(screen.getAllByRole("button", { name: t("clearTaskFilters") })[0]);
    expect(screen.queryByText(t("noTaskMatches"))).not.toBeInTheDocument();
    expect(container.querySelectorAll(".task-card")).toHaveLength(3);
  });

  it("filters by last-run state and links the badge to that run", () => {
    const tasks = [task("task-1"), { ...namedTask("task-2", "lint", "check"), inferred: false }];
    const runs = [runFor("task-2", "check", "succeeded", "2026-08-24T01:00:00Z"), runFor("task-1", "dev", "failed", "2026-08-24T02:00:00Z")];
    const onHistory = vi.fn();
    const { container } = renderWorkspace({ tasks, runs, onHistory });
    fireEvent.change(screen.getByLabelText(t("taskFilterState")), { target: { value: "lastFailed" } });
    const cards = container.querySelectorAll(".task-card");
    expect(cards).toHaveLength(1);
    expect(cards[0]).toHaveTextContent("dev");

    const badge = within(cards[0] as HTMLElement).getByRole("button", { name: new RegExp(t("failed")) });
    fireEvent.click(badge);
    expect(onHistory).toHaveBeenCalledWith(runs[1]);
    expect(screen.getByLabelText("Task console")).toHaveClass("is-open");
  });

  it("sorts by name and run count with a direction toggle", () => {
    const tasks = [namedTask("task-1", "ccc", "build"), namedTask("task-2", "aaa", "check"), namedTask("task-3", "bbb", "dev")];
    const { container } = renderWorkspace({ tasks });
    const names = () => Array.from(container.querySelectorAll(".task-title-line strong")).map((node) => node.textContent);
    fireEvent.change(screen.getByLabelText(t("taskSortLabel")), { target: { value: "name" } });
    expect(names()).toEqual(["aaa", "bbb", "ccc"]);
    fireEvent.click(screen.getByRole("button", { name: t("sortAscending") }));
    expect(screen.getByRole("button", { name: t("sortDescending") })).toBeInTheDocument();
    expect(names()).toEqual(["ccc", "bbb", "aaa"]);
  });

  it("sorts by run count with the busiest task first", () => {
    const tasks = [namedTask("task-1", "alpha", "build"), namedTask("task-2", "beta", "check"), namedTask("task-3", "gamma", "dev")];
    const runs = [
      runFor("task-1", "build", "succeeded", "2026-08-24T01:00:00Z"),
      runFor("task-3", "dev", "succeeded", "2026-08-24T02:00:00Z", 1),
      runFor("task-3", "dev", "failed", "2026-08-24T03:00:00Z", 2),
    ];
    const { container } = renderWorkspace({ tasks, runs });
    fireEvent.change(screen.getByLabelText(t("taskSortLabel")), { target: { value: "runCount" } });
    const names = Array.from(container.querySelectorAll(".task-title-line strong")).map((node) => node.textContent);
    expect(names).toEqual(["gamma", "alpha", "beta"]);
  });

  it("keeps the editing task visible while a filter hides it", () => {
    const tasks = [namedTask("task-1", "build", "build"), namedTask("task-2", "lint", "check")];
    const { container } = renderWorkspace({ tasks });
    fireEvent.click(screen.getAllByRole("button", { name: t("edit") })[1]);
    fireEvent.change(screen.getByPlaceholderText(t("taskSearchHint")), { target: { value: "build" } });
    expect(screen.getByRole("form", { name: t("editTask") })).toBeInTheDocument();
    expect(container.querySelectorAll(".task-card")).toHaveLength(2);
    expect(container.querySelectorAll(".task-title-line strong")).toHaveLength(1);
  });
});
