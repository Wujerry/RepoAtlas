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
