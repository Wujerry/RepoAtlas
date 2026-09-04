import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ProjectQuery, ProjectSummary } from "../types";

const first: ProjectSummary = {
  id: "first", canonicalPath: "C:\\code\\first", displayName: "First Project", detectedName: "first", notes: null, description: null,
  vcsKind: "git", availability: "ready", archived: false, favorite: false, origin: "manual", scanRootId: null,
  languages: [], frameworks: [], packageManagers: [], tags: [], sourceMtime: null, lastCommitAt: null, lastOpenedAt: null, updatedAt: "2026-09-04T00:00:00Z",
};
const second: ProjectSummary = { ...first, id: "second", canonicalPath: "C:\\code\\second", displayName: "Second Project" };
const apiMocks = vi.hoisted(() => ({
  listProjects: vi.fn(),
  getProject: vi.fn(),
  markOpened: vi.fn(async () => undefined),
}));
const activeRunsSnapshot: never[] = [];
const updaterSnapshot = { status: "idle" as const, currentVersion: "0.1.0", downloadedBytes: 0, restartRequired: false };

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("../components/TitleBar", () => ({ TitleBar: ({ onLibrary }: { onLibrary: () => void }) => <button onClick={onLibrary}>Home</button> }));
vi.mock("../components/ProjectList", () => ({ ProjectList: ({ projects, selectedId, onSelect }: { projects: ProjectSummary[]; selectedId?: string; onSelect: (id: string) => void }) => <aside data-selected={selectedId ?? "none"}>{projects.map((project) => <button key={project.id} onClick={() => onSelect(project.id)}>{project.displayName}</button>)}</aside> }));
vi.mock("../components/HomeDashboard", () => ({ HomeDashboard: ({ onOpenProject, onOpenCollection }: { onOpenProject: (id: string) => void; onOpenCollection: (id: string) => void }) => <main><h1>Working Dashboard</h1><button onClick={() => onOpenProject("first")}>Recent First</button><button onClick={() => onOpenCollection("release")}>Release Collection</button></main> }));
vi.mock("../components/Dashboard", () => ({ Dashboard: ({ detail }: { detail: { project: ProjectSummary } }) => <main><h1>{detail.project.displayName} workspace</h1></main> }));
vi.mock("../components/SplashScreen", () => ({ SplashScreen: () => null }));
vi.mock("../components/OnboardingDialog", () => ({ OnboardingDialog: () => null }));
vi.mock("../components/CommandPalette", () => ({ CommandPalette: () => null }));
vi.mock("../components/TaskWorkbench", () => ({ TaskWorkbench: () => <section aria-label="Task workbench mock" /> }));
vi.mock("../components/CollectionControls", () => ({ CollectionControls: () => null }));
vi.mock("../components/HelpPage", () => ({ HelpPage: () => null }));
vi.mock("../components/SettingsPane", () => ({ SettingsPane: () => null }));
vi.mock("../lib/onboarding", () => ({
  readOnboardingState: () => ({ completed: true }), resolveOnboardingVisibility: () => ({ open: false }),
  useOnboardingProjectWatch: () => ({ checkNow: vi.fn(async () => undefined) }), writeOnboardingState: () => undefined,
}));
vi.mock("../lib/task-runs", () => ({
  getActiveTaskRunsSnapshot: () => activeRunsSnapshot, openTaskRun: vi.fn(), refreshTaskRuns: vi.fn(async () => undefined),
  startTaskRunListeners: vi.fn(async () => undefined), subscribeActiveTaskRuns: () => () => undefined,
}));
vi.mock("../lib/updater", () => ({ updaterService: {
  subscribe: () => () => undefined, getSnapshot: () => updaterSnapshot,
  checkForUpdates: vi.fn(), downloadUpdate: vi.fn(), installUpdate: vi.fn(), restartApp: vi.fn(), deferUpdate: vi.fn(),
} }));
vi.mock("../lib/api", () => ({
  api: {
    bootstrap: vi.fn(async () => ({ settings: { theme: "system", locale: "en", uiFont: "", consoleFont: "" }, scanRoots: [], projects: [first, second], collections: [{ id: "release", name: "Release", description: null, projectCount: 1, createdAt: "", updatedAt: "" }] })),
    listProjects: apiMocks.listProjects,
    listScanRoots: vi.fn(async () => []),
    listCollections: vi.fn(async () => [{ id: "release", name: "Release", description: null, projectCount: 1, createdAt: "", updatedAt: "" }]),
    getProject: apiMocks.getProject,
    markOpened: apiMocks.markOpened,
    getAttentionCenter: vi.fn(async () => ({ approvals: [], items: [] })),
    mcpSetupInfo: vi.fn(async () => undefined),
  },
  onAttentionChanged: vi.fn(async () => () => undefined), onScanCompleted: vi.fn(async () => () => undefined),
  onScanFailed: vi.fn(async () => () => undefined), onScanProgress: vi.fn(async () => () => undefined),
  onTaskExited: vi.fn(async () => () => undefined), onTaskPersistenceFailed: vi.fn(async () => () => undefined),
}));

import App from "../App";

describe("App dashboard navigation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("starts on the Dashboard and the title-bar brand returns there", async () => {
    apiMocks.listProjects.mockImplementation(async () => [first, second]);
    apiMocks.getProject.mockImplementation(async (id: string) => ({ project: id === first.id ? first : second, facts: [], git: null, dependencySnapshot: null, tasks: [], lineage: [] }));
    render(<App />);

    expect(await screen.findByRole("heading", { name: "Working Dashboard" })).toBeInTheDocument();
    expect(apiMocks.getProject).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Recent First" }));
    expect(await screen.findByRole("heading", { name: "First Project workspace" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Home" }));
    expect(await screen.findByRole("heading", { name: "Working Dashboard" })).toBeInTheDocument();
  });

  it("opens a Dashboard Collection through the existing filtered project library", async () => {
    apiMocks.listProjects.mockImplementation(async (query?: ProjectQuery) => query?.collectionId === "release" ? [second] : [first, second]);
    apiMocks.getProject.mockImplementation(async (id: string) => ({ project: id === first.id ? first : second, facts: [], git: null, dependencySnapshot: null, tasks: [], lineage: [] }));
    render(<App />);
    await screen.findByRole("heading", { name: "Working Dashboard" });
    fireEvent.click(screen.getByRole("button", { name: "Release Collection" }));

    expect(await screen.findByRole("heading", { name: "Second Project workspace" })).toBeInTheDocument();
    expect(apiMocks.listProjects).toHaveBeenCalledWith(expect.objectContaining({ collectionId: "release" }));
    await waitFor(() => expect(apiMocks.markOpened).toHaveBeenCalledWith("second"));
  });

  it("toggles the task workbench with Ctrl+Backquote", async () => {
    apiMocks.listProjects.mockImplementation(async () => [first, second]);
    render(<App />);
    await screen.findByRole("heading", { name: "Working Dashboard" });

    fireEvent.keyDown(window, { key: "`", code: "Backquote", ctrlKey: true });
    expect(screen.getByLabelText("Task workbench mock")).toBeInTheDocument();

    fireEvent.keyDown(window, { key: "`", code: "Backquote", ctrlKey: true });
    expect(screen.queryByLabelText("Task workbench mock")).not.toBeInTheDocument();
  });
});
