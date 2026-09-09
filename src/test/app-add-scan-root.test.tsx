import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { open } from "@tauri-apps/plugin-dialog";
import { api } from "../lib/api";
import App from "../App";

const taskSnapshot = { runs: [], logs: {}, runtime: {}, selectedRunId: undefined };
const updateState = {
  status: "idle" as const,
  currentVersion: "0.1.0",
  downloadedBytes: 0,
  restartRequired: false,
};

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("../components/TitleBar", () => ({
  TitleBar: ({ onSettings }: { onSettings: () => void }) => <button onClick={onSettings}>Open settings</button>,
}));
vi.mock("../components/ProjectList", () => ({
  ProjectList: ({ onScanFolder }: { onScanFolder: (path: string) => void }) => <button onClick={() => onScanFolder("C:/RepoAtlas Showcase/Demo/web")}>Rescan folder</button>,
}));
vi.mock("../components/SettingsPane", () => ({
  SettingsPane: ({ onAddRoot }: { onAddRoot: () => void }) => <button onClick={onAddRoot}>Add scan root</button>,
}));
vi.mock("../components/SplashScreen", () => ({ SplashScreen: () => null }));
vi.mock("../components/OnboardingDialog", () => ({ OnboardingDialog: () => null }));
vi.mock("../components/CommandPalette", () => ({ CommandPalette: () => null }));
vi.mock("../components/TaskWorkbench", () => ({ TaskWorkbench: () => null }));
vi.mock("../components/CollectionControls", () => ({ CollectionControls: () => null }));
vi.mock("../components/HelpPage", () => ({ HelpPage: () => null }));
vi.mock("../components/Dashboard", () => ({ Dashboard: () => null }));
vi.mock("../components/HomeDashboard", () => ({ HomeDashboard: () => <main>Dashboard</main> }));
vi.mock("../lib/onboarding", () => ({
  readOnboardingState: () => ({ completed: true }),
  resolveOnboardingVisibility: () => ({ open: false }),
  useOnboardingProjectWatch: () => ({ checkNow: vi.fn(async () => undefined) }),
  writeOnboardingState: () => undefined,
}));
vi.mock("../lib/task-runs", () => ({
  getTaskRunSnapshot: () => taskSnapshot,
  getActiveTaskRunsSnapshot: () => taskSnapshot.runs,
  openTaskRun: vi.fn(),
  refreshTaskRuns: vi.fn(async () => undefined),
  startTaskRunListeners: vi.fn(async () => undefined),
  subscribeTaskRuns: () => () => undefined,
  subscribeActiveTaskRuns: () => () => undefined,
}));
vi.mock("../lib/updater", () => ({
  updaterService: {
    subscribe: () => () => undefined,
    getSnapshot: () => updateState,
    checkForUpdates: vi.fn(async () => undefined),
    downloadUpdate: vi.fn(async () => undefined),
    installUpdate: vi.fn(async () => true),
    restartApp: vi.fn(async () => undefined),
    deferUpdate: vi.fn(async () => undefined),
  },
}));

const scanRoot = {
  id: "root-1",
  path: "C:/RepoAtlas Showcase/Demo",
  createdAt: "2026-09-04T00:00:00.000Z",
  lastScannedAt: null,
};

vi.mock("../lib/api", () => ({
  api: {
    bootstrap: vi.fn(async () => ({
      settings: { theme: "system", locale: "en", uiFont: "", consoleFont: "" },
      scanRoots: [],
      projects: [],
      collections: [],
    })),
    listScanRoots: vi.fn(async () => [scanRoot]),
    listProjects: vi.fn(async () => []),
    listCollections: vi.fn(async () => []),
    listPendingApprovals: vi.fn(async () => []),
    listAttentionItems: vi.fn(async () => []),
    getAttentionCenter: vi.fn(async () => ({ approvals: [], items: [] })),
    getProject: vi.fn(async () => {
      throw new Error("not used in this flow");
    }),
    mcpSetupInfo: vi.fn(async () => undefined),
    addScanRoot: vi.fn(async () => scanRoot),
    startScan: vi.fn(async () => undefined),
  },
  onAttentionChanged: vi.fn(async () => () => undefined),
  onScanCompleted: vi.fn(async () => () => undefined),
  onScanFailed: vi.fn(async () => () => undefined),
  onScanProgress: vi.fn(async () => () => undefined),
  onTaskExited: vi.fn(async () => () => undefined),
  onTaskPersistenceFailed: vi.fn(async () => () => undefined),
}));

describe("adding a scan root", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(api.startScan).mockResolvedValue(undefined);
    vi.mocked(open).mockResolvedValue(scanRoot.path);
  });

  it("passes the selected folder without registering another Scan Root", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Rescan folder" }));
    await waitFor(() => expect(api.startScan).toHaveBeenCalledWith(undefined, "C:/RepoAtlas Showcase/Demo/web"));
    expect(api.addScanRoot).not.toHaveBeenCalled();
  });

  it("starts a scan for the new root immediately after adding it", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Open settings" }));
    fireEvent.click(await screen.findByRole("button", { name: "Add scan root" }));

    await waitFor(() => expect(api.addScanRoot).toHaveBeenCalledWith(scanRoot.path));
    await waitFor(() => expect(api.startScan).toHaveBeenCalledWith("root-1", undefined));
  });

  it("warns instead of starting a second scan while one is already running", async () => {
    let releaseScan: () => void = () => undefined;
    vi.mocked(api.startScan).mockImplementation(() => new Promise<void>((resolve) => { releaseScan = resolve; }));
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Open settings" }));

    fireEvent.click(screen.getByRole("button", { name: "Add scan root" }));
    await waitFor(() => expect(api.startScan).toHaveBeenCalledTimes(1));

    fireEvent.click(screen.getByRole("button", { name: "Add scan root" }));
    expect(await screen.findByText("A scan is already running; the new root is not scanned yet")).toBeInTheDocument();
    expect(api.addScanRoot).toHaveBeenCalledTimes(2);
    expect(api.startScan).toHaveBeenCalledTimes(1);
    releaseScan();
  });
});
