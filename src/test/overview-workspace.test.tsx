import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { dictionaries, type MessageKey } from "../i18n";
import type { EnvironmentInspection, ProjectDetail } from "../types";
import { describe, expect, it, vi } from "vitest";

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
    files: [{ kind: "manifest", path: "package.json", source: "package.json" }],
  };
}

describe("overview workspace", () => {
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
        onOpenSettings={vi.fn()}
        onDescription={vi.fn()}
        onNotes={vi.fn()}
        onTags={vi.fn()}
      />,
    );

    expect(document.querySelector(".overview-nav")).toBeNull();
    const switcher = document.querySelector(".overview-switcher button");
    expect(switcher).not.toBeNull();
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
    expect(within(environmentRegion).getAllByText(t("versionMatch")).length).toBeGreaterThan(0);
    expect(within(environmentRegion).getAllByText(t("versionMismatch")).length).toBeGreaterThan(0);
    expect(within(environmentRegion).getAllByText(t("versionMissing")).length).toBeGreaterThan(0);
    expect(within(environmentRegion).getAllByText(t("undeclaredVersion")).length).toBeGreaterThan(0);
    expect(within(environmentRegion).getAllByText(t("unknownVersion")).length).toBeGreaterThan(0);
    fireEvent.click(await screen.findByRole("button", { name: "package.json" }));
    expect(await screen.findByText('{ "name": "atlas" }')).toBeInTheDocument();
    expect(screen.getByRole("button", { name: t("revealInExplorer") })).toBeInTheDocument();
  });
});
