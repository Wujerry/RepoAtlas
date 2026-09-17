import { invalidateOverviewCache, overviewTools } from "../lib/overview-cache";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { dictionaries, type MessageKey } from "../i18n";
import type { EnvironmentInspection, ProjectDetail } from "../types";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { inspectProjectEnvironment, readProjectFile, revealProjectFile, openProjectFile } = vi.hoisted(() => ({
  inspectProjectEnvironment: vi.fn(),
  readProjectFile: vi.fn(),
  revealProjectFile: vi.fn(),
  openProjectFile: vi.fn(),
}));

vi.mock("../lib/api", () => ({
  api: {
    inspectProjectEnvironment,
    readProjectFile,
    revealProjectFile,
    openProjectFile,
    gitStatus: vi.fn().mockResolvedValue({ snapshot: { branch: "main", dirty: false, ahead: 0, behind: 0, lastCommitSha: null, lastCommitSubject: null, lastCommitAt: null, observedAt: "2026-08-26T00:00:00Z" }, files: [], branches: ["main"], log: [] }),
    listTaskRuns: vi.fn().mockResolvedValue([]),
    readProjectReadme: vi.fn().mockRejectedValue(new Error("missing")),
    readProjectDocument: vi.fn().mockRejectedValue(new Error("missing")),
    updateProject: vi.fn(),
    startTask: vi.fn(),
    stopTask: vi.fn(),
    readTaskLog: vi.fn(),
    writeTaskStdin: vi.fn(),
    gitDiff: vi.fn(),
    gitExecute: vi.fn(),
    listExternalTools: vi.fn().mockResolvedValue({ agents: [], ides: [], terminals: [] }),
  },
  onTaskLog: vi.fn(async () => () => undefined),
  onTaskExited: vi.fn(async () => () => undefined),
  onTaskPersistenceFailed: vi.fn(async () => () => undefined),
}));

import { Dashboard } from "../components/Dashboard";
import { GitWorkspace } from "../components/GitWorkspace";

const t = (key: MessageKey) => dictionaries.en[key];

function detail(): ProjectDetail {
  return {
    project: {
      id: "atlas",
      canonicalPath: "C:/code/atlas",
      displayName: "Atlas",
      detectedName: "Atlas",
      notes: null,
      description: "Local atlas",
      vcsKind: "git",
      availability: "ready",
      archived: false,
      favorite: false,
      origin: "scan",
      scanRootId: "root",
      languages: ["TypeScript"],
      frameworks: ["React"],
      packageManagers: ["pnpm"],
      tags: [],
      sourceMtime: null,
      lastCommitAt: null,
      lastOpenedAt: null,
      updatedAt: "2026-08-26T00:00:00Z",
    },
    facts: [
      { kind: "runtime", value: "node@^18", confidence: 0.9, source: "package.json#engines.node" },
      { kind: "manifest", value: "package.json", confidence: 0.95, source: "package.json" },
    ],
    readmePath: null,
    readmeExcerpt: null,
    tasks: [{ id: "dev", kind: "dev", name: "dev", executable: "pnpm", argv: ["dev"], cwd: null, inferred: true, shellMode: false }],
    git: { branch: "main", dirty: false, ahead: 0, behind: 0, lastCommitSha: null, lastCommitSubject: null, lastCommitAt: null, observedAt: "2026-08-26T00:00:00Z" },
    dependencies: null,
    runtimeRequirements: [{ ecosystem: "node", label: "Node.js", constraint: "^18", source: "package.json#engines.node" }],
    projectFiles: [{ kind: "manifest", path: "package.json", source: "package.json" }],
    startHere: [{ taskId: "dev", kind: "dev", name: "Start development", source: "package.json#scripts.dev", inferred: true }],
    recentEvents: [{ id: "event-1", projectId: "atlas", kind: "ide", title: "Opened IDE", detail: "vscode", createdAt: "2026-08-26T00:00:00Z" }, { id: "event-2", projectId: "atlas", kind: "open", title: "Opened project", detail: null, createdAt: "2026-08-26T00:01:00Z" }],
    lineage: null,
  };
}

function environment(): EnvironmentInspection {
  return {
    projectId: "atlas",
    runtimes: [
      { ecosystem: "node", label: "Node.js", constraint: "^18", source: "package.json#engines.node", localVersion: "18.20.4", matchState: "match" },
      { ecosystem: "python", label: "Python", constraint: ">=3.12", source: "pyproject.toml#requires-python", localVersion: "3.11.9", matchState: "mismatch" },
      { ecosystem: "rust", label: "Rust", constraint: "1.80", source: "rust-toolchain.toml", localVersion: null, matchState: "missing" },
      { ecosystem: "go", label: "Go", constraint: null, source: null, localVersion: "1.23.0", matchState: "undeclared" },
      { ecosystem: "java", label: "Java", constraint: "21", source: "pom.xml", localVersion: null, matchState: "unknown" },
    ],
    files: [
      { kind: "manifest", path: "package.json", source: "package.json" },
      { kind: "packageManager", path: "package.json", source: "package.json#packageManager" },
      { kind: "asset", path: "icon.png", source: "icon.png" },
    ],
  };
}

describe("overview workspace", () => {
  beforeEach(() => { invalidateOverviewCache(); overviewTools.clear(); });
  it("keeps the live header branch when switching workspace tabs", async () => {
    inspectProjectEnvironment.mockResolvedValue(environment());
    const original = detail();
    original.git = { ...original.git!, branch: "old-scanned-branch" };
    const props = { t, notify: vi.fn(), onFavorite: vi.fn(), onArchive: vi.fn(), onRefresh: vi.fn(), onRemove: vi.fn(), onOpenExplorer: vi.fn(), onOpenTerminal: vi.fn(), onOpenIde: vi.fn(), onOpenAgent: vi.fn(), onDescription: vi.fn(), onNotes: vi.fn(), onTags: vi.fn() };
    const { container } = render(<Dashboard {...props} detail={original} />);
    await waitFor(() => expect(screen.queryByText("old-scanned-branch")).not.toBeInTheDocument());
    for (const tab of ["tasks", "git", "overview", "tasks"]) {
      fireEvent.click(container.querySelector(`#project-tab-${tab}`)!);
      expect(screen.queryByText("old-scanned-branch")).not.toBeInTheDocument();
      expect(screen.getAllByText("main").length).toBeGreaterThan(0);
    }
  });
  it("reuses the cached environment immediately after switching away and back", async () => {
    inspectProjectEnvironment.mockClear();
    inspectProjectEnvironment.mockImplementation(async (id: string) => ({ ...environment(), projectId: id }));
    const props = { t, notify: vi.fn(), onFavorite: vi.fn(), onArchive: vi.fn(), onRefresh: vi.fn(), onRemove: vi.fn(), onOpenExplorer: vi.fn(), onOpenTerminal: vi.fn(), onOpenIde: vi.fn(), onOpenAgent: vi.fn(), onDescription: vi.fn(), onNotes: vi.fn(), onTags: vi.fn() };
    const original = detail();
    const { rerender } = render(<Dashboard {...props} detail={original} />);
    await screen.findByText("18.20.4");
    const another = { ...original, project: { ...original.project, id: "other", displayName: "Other" } };
    rerender(<Dashboard {...props} detail={another} />);
    await waitFor(() => expect(inspectProjectEnvironment).toHaveBeenCalledWith("other"));
    rerender(<Dashboard {...props} detail={original} />);
    expect(screen.getByText("18.20.4")).toBeInTheDocument();
    await new Promise((resolve) => setTimeout(resolve, 220));
    expect(inspectProjectEnvironment.mock.calls.filter(([id]) => id === "atlas")).toHaveLength(1);
  });
  it("refreshes untouched metadata editors and preserves active drafts", async () => {
    inspectProjectEnvironment.mockResolvedValue(environment());
    const onNotes = vi.fn();
    const props = { t, notify: vi.fn(), onFavorite: vi.fn(), onArchive: vi.fn(), onRefresh: vi.fn(), onRemove: vi.fn(), onOpenExplorer: vi.fn(), onOpenTerminal: vi.fn(), onOpenIde: vi.fn(), onOpenAgent: vi.fn(), onDescription: vi.fn(), onNotes, onTags: vi.fn() };
    const initial = detail();
    initial.project.notes = "Original notes";
    const { rerender } = render(<Dashboard {...props} detail={initial} />);
    const updated = { ...initial, project: { ...initial.project, description: "Agent description", notes: "Agent notes" } };
    rerender(<Dashboard {...props} detail={updated} />);
    const notes = screen.getByLabelText(t("notes"));
    expect(notes).toHaveValue("Agent notes");
    fireEvent.blur(notes);
    expect(onNotes).not.toHaveBeenCalled();
    fireEvent.change(notes, { target: { value: "Local draft" } });
    const later = { ...updated, project: { ...updated.project, notes: "Later Agent notes" } };
    rerender(<Dashboard {...props} detail={later} />);
    expect(notes).toHaveValue("Local draft");
    fireEvent.blur(notes);
    expect(onNotes).toHaveBeenCalledWith("Local draft");
    fireEvent.click(screen.getByRole("button", { name: "Agent description" }));
    const dialog = await screen.findByRole("dialog");
    const description = within(dialog).getByRole("textbox");
    expect(description).toHaveValue("Agent description");
    fireEvent.change(description, { target: { value: "Local description draft" } });
    rerender(<Dashboard {...props} detail={{ ...later, project: { ...later.project, description: "Later Agent description" } }} />);
    expect(description).toHaveValue("Local description draft");
    fireEvent.click(within(dialog).getByRole("button", { name: t("save") }));
    await waitFor(() => expect(props.onDescription).toHaveBeenCalledWith("Local description draft"));
  });

  it("counts a mixed Git status as one file and keeps both diff actions", () => {
    const onDiff = vi.fn();
    render(<GitWorkspace git={{ snapshot: detail().git!, files: [{ path: "file.txt", status: "M", staged: true }, { path: "file.txt", status: "M", staged: false }], branches: [], log: [] }} loading={false} busy={false} commitMessage="" setCommitMessage={vi.fn()} stagedCount={1} diff="" diffLoading={false} t={t} onRetry={vi.fn()} onGit={vi.fn()} onDiff={onDiff} />);
    expect(screen.getByText(`1 ${t("changedFiles")}`)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: `Mfile.txt${t("staged")}` }));
    fireEvent.click(screen.getByRole("button", { name: `Mfile.txt${t("unstaged")}` }));
    expect(onDiff.mock.calls).toEqual([["file.txt", true], ["file.txt", false]]);
  });

  it("keeps the overview switcher in the tab row and opens a file preview", async () => {
    inspectProjectEnvironment.mockResolvedValue(environment());
    readProjectFile.mockResolvedValue({ path: "package.json", content: '{ "name": "atlas" }', truncated: false });
    render(
      <Dashboard
        detail={detail()}
        t={t}
        notify={vi.fn()}
        onFavorite={vi.fn()}
        onArchive={vi.fn()}
        onRefresh={vi.fn()}
        onRemove={vi.fn()}
        onOpenExplorer={vi.fn()}
        onOpenTerminal={vi.fn()}
        onOpenIde={vi.fn()}
        onOpenAgent={vi.fn()}
        onDescription={vi.fn()}
        onNotes={vi.fn()}
        onTags={vi.fn()}
      />,
    );

    expect(document.querySelector(".overview-nav")).toBeNull();
    expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual([
      t("overview"),
      t("files"),
      t("git"),
      t("tasks"),
    ]);
    const basics = screen.getByLabelText(t("projectBasics"));
    expect(basics).toHaveTextContent(`${t("projectStatus")}${t("ready")}`);
    expect(basics).toHaveTextContent("Gitmain");
    expect(basics).toHaveTextContent("TypeScript");
    expect(basics).toHaveTextContent(`${t("projectTaskCount")}1`);
    const switcher = document.querySelector(".overview-switcher button");
    expect(switcher).not.toBeNull();
    const overviewContent = document.querySelector("#project-panel-overview > .workspace-content");
    expect(overviewContent).not.toBeNull();
    expect(overviewContent).not.toHaveAttribute("style");
    await waitFor(() => expect(inspectProjectEnvironment).toHaveBeenCalledWith("atlas"));
    expect(document.querySelector(".overview-switcher button")).toHaveTextContent("01");
    expect(document.querySelector(".overview-switcher button")).toHaveTextContent("06");
    const continueRegion = screen.getByRole("region", { name: t("continueWork") });
    expect(continueRegion).toHaveTextContent("main");
    expect(continueRegion).toHaveTextContent("Start development");
    expect(continueRegion).toHaveTextContent("package.json#scripts.dev");
    expect(continueRegion).toHaveTextContent("Opened IDE");
    expect(continueRegion).not.toHaveTextContent("Opened project");
    expect(within(continueRegion).getByLabelText("Ahead 0")).toBeInTheDocument();
    expect(within(continueRegion).getByLabelText("Behind 0")).toBeInTheDocument();
    const environmentRegion = screen.getByRole("region", { name: t("environment") });
    expect((await within(environmentRegion).findAllByText(t("versionMatch"))).length).toBeGreaterThan(0);
    expect(within(environmentRegion).getAllByText(t("versionMismatch")).length).toBeGreaterThan(0);
    expect(within(environmentRegion).getAllByText(t("versionMissing")).length).toBeGreaterThan(0);
    expect(within(environmentRegion).getAllByText(t("undeclaredVersion")).length).toBeGreaterThan(0);
    expect(within(environmentRegion).getAllByText(t("unknownVersion")).length).toBeGreaterThan(0);
    for (const mark of environmentRegion.querySelectorAll(".runtime-monogram")) {
      expect(mark.querySelector("svg")).toBeInTheDocument();
      expect(mark.textContent).toBe("");
      expect(mark).toHaveAttribute("aria-hidden", "true");
    }
    expect(within(environmentRegion).getAllByRole("button", { name: "package.json" })).toHaveLength(1);
    expect(within(environmentRegion).getByRole("button", { name: "package.json" })).toHaveTextContent(t("fileGroupPackageManager"));
    expect(within(environmentRegion).queryByRole("button", { name: "icon.png" })).not.toBeInTheDocument();
    fireEvent.click(await screen.findByRole("button", { name: "package.json" }));
    await waitFor(() => expect(screen.getByRole("button", { name: t("copyFileContent") })).toBeEnabled());
    expect(readProjectFile).toHaveBeenCalledWith("atlas", "package.json");
    expect(screen.getByRole("button", { name: t("revealInExplorer") })).toBeInTheDocument();
  });
});
