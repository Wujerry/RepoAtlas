import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactNode } from "react";

import { api } from "../lib/api";
import App from "../App";

const taskSnapshot = { runs: [], logs: {}, runtime: {}, selectedRunId: undefined };
const updateState = {
  status: "idle" as const,
  currentVersion: "0.1.0",
  downloadedBytes: 0,
  restartRequired: false,
};

const collection = {
  id: "collection-1",
  name: "Daily work",
  description: null,
  projectCount: 0,
  createdAt: "2026-09-04T00:00:00.000Z",
  updatedAt: "2026-09-04T00:00:00.000Z",
};

vi.mock("../components/TitleBar", () => ({
  TitleBar: () => <div>TitleBar</div>,
}));
vi.mock("../components/ProjectList", () => ({
  ProjectList: ({ collectionControls }: { collectionControls?: ReactNode }) => (
    <div>{collectionControls}</div>
  ),
}));
vi.mock("../components/CollectionControls", () => ({
  CollectionControls: ({ collections, onSelect, onChanged }: {
    collections: Array<{ id: string }>;
    onSelect: (id?: string) => void;
    onChanged: (id?: string) => Promise<void>;
  }) => (
    <div>
      <span>Collections: {collections.length}</span>
      <button onClick={() => onSelect(collections[0]?.id)}>select collection</button>
      <button onClick={() => void onChanged(undefined)}>delete collection</button>
    </div>
  ),
}));
vi.mock("../components/SplashScreen", () => ({ SplashScreen: () => null }));
vi.mock("../components/OnboardingDialog", () => ({ OnboardingDialog: () => null }));
vi.mock("../components/CommandPalette", () => ({ CommandPalette: () => null }));
vi.mock("../components/TaskWorkbench", () => ({ TaskWorkbench: () => null }));
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

vi.mock("../lib/api", () => ({
  api: {
    bootstrap: vi.fn(async () => ({
      settings: { theme: "system", locale: "en", uiFont: "", consoleFont: "" },
      scanRoots: [],
      projects: [],
      collections: [collection],
    })),
    listScanRoots: vi.fn(async () => []),
    listProjects: vi.fn(async () => []),
    // After deletion the collection is gone, so reloads must observe this.
    listCollections: vi.fn(async () => []),
    listPendingApprovals: vi.fn(async () => []),
    listAttentionItems: vi.fn(async () => []),
    getAttentionCenter: vi.fn(async () => ({ approvals: [], items: [] })),
    getProject: vi.fn(async () => {
      throw new Error("not used in this flow");
    }),
    mcpSetupInfo: vi.fn(async () => undefined),
  },
  onAttentionChanged: vi.fn(async () => () => undefined),
  onScanCompleted: vi.fn(async () => () => undefined),
  onScanFailed: vi.fn(async () => () => undefined),
  onScanProgress: vi.fn(async () => () => undefined),
  onTaskExited: vi.fn(async () => () => undefined),
  onTaskPersistenceFailed: vi.fn(async () => () => undefined),
}));

describe("deleting a collection", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("refreshes the collection menu immediately after deletion", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "select collection" }));
    fireEvent.click(await screen.findByRole("button", { name: "delete collection" }));

    await waitFor(() => expect(screen.getByText("Collections: 0")).toBeInTheDocument(), { timeout: 3000 });
    await waitFor(() => expect(api.listCollections).toHaveBeenCalled());
  });
});
