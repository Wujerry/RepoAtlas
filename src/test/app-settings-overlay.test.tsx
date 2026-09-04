import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

const updateState = {
  status: "idle" as const,
  currentVersion: "0.1.0",
  downloadedBytes: 0,
  restartRequired: false,
};
const taskSnapshot = { runs: [], logs: {}, runtime: {}, selectedRunId: undefined };
const projectContextMenu = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("../components/TitleBar", () => ({
  TitleBar: ({ onSettings, onHelp }: { onSettings: () => void; onHelp: () => void }) => <><button onClick={onSettings}>Open settings</button><button onClick={onHelp}>Open help</button></>,
}));
vi.mock("../components/ProjectList", () => ({
  ProjectList: () => <><label>Library state<input aria-label="Library state" defaultValue="" /></label><button data-repoatlas-context-menu="true" onContextMenu={projectContextMenu}>Project menu</button></>,
}));
vi.mock("../components/SettingsPane", () => ({
  SettingsPane: ({ onBack }: { onBack: () => void }) => <section><h1 id="settings-page-title">Settings</h1><button onClick={onBack}>Close settings</button></section>,
}));
vi.mock("../components/SplashScreen", () => ({ SplashScreen: () => null }));
vi.mock("../components/OnboardingDialog", () => ({ OnboardingDialog: () => null }));
vi.mock("../components/CommandPalette", () => ({ CommandPalette: () => null }));
vi.mock("../components/TaskWorkbench", () => ({ TaskWorkbench: () => null }));
vi.mock("../components/CollectionControls", () => ({ CollectionControls: () => null }));
vi.mock("../components/HelpPage", () => ({
  HelpPage: ({ onBack }: { onBack: () => void }) => <section><h1 id="help-page-title">Help</h1><button onClick={onBack}>Close help</button></section>,
}));
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
    checkForUpdates: vi.fn(async () => updateState),
    downloadUpdate: vi.fn(async () => updateState),
    installUpdate: vi.fn(async () => true),
    restartApp: vi.fn(async () => undefined),
    deferUpdate: vi.fn(async () => undefined),
  },
}));
vi.mock("../lib/api", () => ({
  api: {
    bootstrap: vi.fn(async () => ({
      settings: { theme: "system", locale: "en", uiFont: "", consoleFont: "" },
      scanRoots: [],
      projects: [],
      collections: [],
    })),
    listProjects: vi.fn(async () => []),
    listPendingApprovals: vi.fn(async () => []),
    listAttentionItems: vi.fn(async () => []),
    getAttentionCenter: vi.fn(async () => ({ approvals: [], items: [] })),
    mcpSetupInfo: vi.fn(async () => undefined),
  },
  onAttentionChanged: vi.fn(async () => () => undefined),
  onScanCompleted: vi.fn(async () => () => undefined),
  onScanFailed: vi.fn(async () => () => undefined),
  onScanProgress: vi.fn(async () => () => undefined),
  onTaskExited: vi.fn(async () => () => undefined),
  onTaskPersistenceFailed: vi.fn(async () => () => undefined),
}));

import App from "../App";

describe("settings overlay", () => {
  it("keeps the underlying library mounted while settings opens and closes", async () => {
    render(<App />);
    const libraryInput = await screen.findByRole("textbox", { name: "Library state" });
    fireEvent.change(libraryInput, { target: { value: "keep this state" } });

    fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    expect(await screen.findByRole("dialog", { name: "Settings" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Close settings" }));

    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Settings" })).not.toBeInTheDocument());
    const restoredInput = screen.getByRole("textbox", { name: "Library state" });
    expect(restoredInput).toBe(libraryInput);
    expect(restoredInput).toHaveValue("keep this state");
  });

  it("keeps the underlying library mounted while help opens and closes", async () => {
    render(<App />);
    const libraryInput = await screen.findByRole("textbox", { name: "Library state" });
    fireEvent.change(libraryInput, { target: { value: "keep help state" } });

    fireEvent.click(screen.getByRole("button", { name: "Open help" }));
    expect(await screen.findByRole("dialog", { name: "Help" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Close help" }));

    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Help" })).not.toBeInTheDocument());
    const restoredInput = screen.getByRole("textbox", { name: "Library state" });
    expect(restoredInput).toBe(libraryInput);
    expect(restoredInput).toHaveValue("keep help state");
  });

  it("blocks the native context menu except on custom project-menu triggers", async () => {
    render(<App />);
    const libraryInput = await screen.findByRole("textbox", { name: "Library state" });
    const nativeEvent = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });
    fireEvent(libraryInput, nativeEvent);
    expect(nativeEvent.defaultPrevented).toBe(true);

    const projectMenuTrigger = screen.getByRole("button", { name: "Project menu" });
    const projectEvent = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });
    fireEvent(projectMenuTrigger, projectEvent);
    expect(projectEvent.defaultPrevented).toBe(false);
    expect(projectContextMenu).toHaveBeenCalled();
  });
});
