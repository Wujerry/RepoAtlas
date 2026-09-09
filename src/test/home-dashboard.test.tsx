import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { HomeDashboard } from "../components/HomeDashboard";
import { dictionaries, type MessageKey } from "../i18n";
import type { DashboardSnapshot, ProjectSummary, TaskRun } from "../types";

const mocks = vi.hoisted(() => ({
  getDashboardSnapshot: vi.fn(),
  readProjectIcons: vi.fn(async () => []),
}));

vi.mock("../lib/api", () => ({ api: mocks }));

const project: ProjectSummary = {
  id: "project-1",
  canonicalPath: "C:\\code\\repoatlas",
  displayName: "RepoAtlas",
  detectedName: "repoatlas",
  notes: null,
  description: null,
  vcsKind: "git",
  availability: "ready",
  archived: false,
  favorite: true,
  origin: "manual",
  scanRootId: null,
  languages: ["TypeScript"],
  frameworks: ["React"],
  packageManagers: ["pnpm"],
  tags: [],
  sourceMtime: "2026-09-04T08:00:00Z",
  lastCommitAt: "2026-09-04T08:00:00Z",
  lastOpenedAt: "2026-09-04T09:00:00Z",
  updatedAt: "2026-09-04T09:00:00Z",
};

const run: TaskRun = {
  id: "run-1",
  projectId: project.id,
  taskId: "test",
  kind: "test",
  executable: "pnpm",
  argv: ["test"],
  cwd: project.canonicalPath,
  shellMode: false,
  status: "failed",
  exitCode: 1,
  logPath: "C:\\logs\\run-1.log",
  startedAt: "2026-09-04T08:00:00Z",
  finishedAt: "2026-09-04T08:01:00Z",
};

const snapshot: DashboardSnapshot = {
  generatedAt: "2026-09-04T09:30:00Z",
  projectCount: 12,
  availableProjectCount: 11,
  unavailableProjectCount: 1,
  collectionCount: 1,
  activeRunCount: 2,
  attentionCount: 2,
  sevenDayRuns: { total: 9, succeeded: 6, failed: 2, cancelled: 1 },
  collections: [{
    id: "collection-1",
    name: "Release train",
    description: "Projects shipping together",
    projectCount: 3,
    archivedProjectCount: 1,
    activeRunCount: 1,
    attentionCount: 1,
    lastActivityAt: "2026-09-04T09:00:00Z",
  }],
  recentProjects: [{
    project,
    git: { branch: "main", dirty: true, ahead: 0, behind: 0, lastCommitSha: "abc", lastCommitSubject: "work", lastCommitAt: "2026-09-04T08:00:00Z", observedAt: "2026-09-04T09:00:00Z" },
    latestRun: { kind: "test", status: "failed", startedAt: run.startedAt, finishedAt: run.finishedAt },
  }],
  recentRuns: [{ run, projectName: project.displayName }],
  attentionPreview: [{
    id: "run-failed:run-1",
    kind: "run_failed",
    severity: "error",
    projectId: project.id,
    runId: run.id,
    approvalId: null,
    title: "Tests failed",
    detail: "Open the output to inspect the failure.",
    occurredAt: run.finishedAt!,
    sourceVersion: run.finishedAt!,
    acknowledgeable: true,
  }],
};

const t = (key: MessageKey) => dictionaries.en[key];

describe("HomeDashboard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.getDashboardSnapshot.mockResolvedValue(snapshot);
  });

  it("shows source-backed status, collections, recent Projects, runs, and attention", async () => {
    render(<HomeDashboard t={t} refreshKey="initial" onOpenProject={vi.fn()} onOpenCollection={vi.fn()} onOpenRun={vi.fn()} onOpenAttention={vi.fn()} onOpenAttentionItem={vi.fn()} />);

    expect(await screen.findByRole("region", { name: "Current work status" })).toHaveTextContent("12");
    expect(screen.getByRole("heading", { name: "Dashboard" })).toBeInTheDocument();
    expect(screen.getByText("6")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Release train/ })).toHaveTextContent("3 Projects");
    expect(screen.getByRole("button", { name: /RepoAtlas.*main.*Failed/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Tests failed/ })).toBeInTheDocument();
    await waitFor(() => expect(mocks.readProjectIcons).toHaveBeenCalledWith([project.id]));
  });

  it("routes every dashboard entry through its typed callback", async () => {
    const onOpenProject = vi.fn();
    const onOpenCollection = vi.fn();
    const onOpenRun = vi.fn();
    const onOpenAttention = vi.fn();
    const onOpenAttentionItem = vi.fn();
    render(<HomeDashboard t={t} refreshKey="initial" onOpenProject={onOpenProject} onOpenCollection={onOpenCollection} onOpenRun={onOpenRun} onOpenAttention={onOpenAttention} onOpenAttentionItem={onOpenAttentionItem} />);
    await screen.findByRole("region", { name: "Current work status" });

    fireEvent.click(screen.getByRole("button", { name: /Release train/ }));
    fireEvent.click(screen.getByRole("button", { name: /RepoAtlas.*main.*Failed/i }));
    fireEvent.click(screen.getByRole("button", { name: /RepoAtlas.*test.*Failed/i }));
    fireEvent.click(screen.getByRole("button", { name: /Tests failed/ }));
    fireEvent.click(screen.getByRole("button", { name: "View all" }));

    expect(onOpenCollection).toHaveBeenCalledWith("collection-1");
    expect(onOpenProject).toHaveBeenCalledWith("project-1");
    expect(onOpenRun).toHaveBeenCalledWith("run-1");
    expect(onOpenAttentionItem).toHaveBeenCalledWith(snapshot.attentionPreview[0]);
    expect(onOpenAttention).toHaveBeenCalledOnce();
  });

  it("opens the featured available Project through the same callback", async () => {
    const onOpenProject = vi.fn();
    mocks.getDashboardSnapshot.mockResolvedValue({ ...snapshot, recentProjects: [
      { ...snapshot.recentProjects[0], project: { ...project, id: "unavailable", displayName: "Missing", availability: "missing" } },
      snapshot.recentProjects[0],
    ] });
    render(<HomeDashboard t={t} refreshKey="featured" onOpenProject={onOpenProject} onOpenCollection={vi.fn()} onOpenRun={vi.fn()} onOpenAttention={vi.fn()} onOpenAttentionItem={vi.fn()} />);
    fireEvent.click(await screen.findByRole("button", { name: "Open Project" }));
    expect(onOpenProject).toHaveBeenCalledWith(project.id);
    expect(screen.getByRole("region", { name: "Continue" })).toHaveTextContent("RepoAtlas");
  });

  it("shows honest guidance without a resume action when no available recent Project exists", async () => {
    mocks.getDashboardSnapshot.mockResolvedValue({ ...snapshot, recentProjects: [] });
    render(<HomeDashboard t={t} refreshKey="empty" onOpenProject={vi.fn()} onOpenCollection={vi.fn()} onOpenRun={vi.fn()} onOpenAttention={vi.fn()} onOpenAttentionItem={vi.fn()} />);
    expect(await screen.findByRole("heading", { name: "A place for every Project." })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Open Project" })).not.toBeInTheDocument();
  });

  it("keeps the last snapshot visible when a refresh fails", async () => {
    mocks.getDashboardSnapshot.mockResolvedValueOnce(snapshot).mockRejectedValueOnce(new Error("database busy"));
    const { rerender } = render(<HomeDashboard t={t} refreshKey="one" onOpenProject={vi.fn()} onOpenCollection={vi.fn()} onOpenRun={vi.fn()} onOpenAttention={vi.fn()} onOpenAttentionItem={vi.fn()} />);
    await screen.findByRole("region", { name: "Current work status" });
    rerender(<HomeDashboard t={t} refreshKey="two" onOpenProject={vi.fn()} onOpenCollection={vi.fn()} onOpenRun={vi.fn()} onOpenAttention={vi.fn()} onOpenAttentionItem={vi.fn()} />);

    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Refresh failed"));
    expect(screen.getByRole("region", { name: "Current work status" })).toHaveTextContent("12");
  });
});
