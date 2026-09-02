import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { OnboardingDialog } from "../components/OnboardingDialog";
import { dictionaries, type MessageKey } from "../i18n";
import type { McpSetupInfo, ProjectSummary, ScanRoot } from "../types";

const t = (key: MessageKey) => dictionaries.en[key];

const installed: McpSetupInfo = {
  dbPath: "C:\\Users\\dev\\repoatlas.sqlite",
  platform: "windows",
  binaryName: "repoatlas-mcp.exe",
  binaryPath: "C:\\Program Files\\RepoAtlas\\repoatlas-mcp.exe",
  workspacePath: null,
};

const unavailable: McpSetupInfo = {
  dbPath: "/data/repoatlas.sqlite",
  platform: "macos",
  binaryName: "repoatlas-mcp",
  binaryPath: null,
  workspacePath: null,
};

const root: ScanRoot = { id: "root-1", path: "C:\\code", createdAt: "2026-08-31T00:00:00Z", lastScannedAt: null };

function project(): ProjectSummary {
  return {
    id: "atlas",
    canonicalPath: "C:\\code\\atlas",
    displayName: "Atlas",
    detectedName: "Atlas",
    description: null,
    notes: null,
    vcsKind: "git",
    availability: "ready",
    archived: false,
    favorite: false,
    origin: "mcp",
    scanRootId: null,
    languages: ["TypeScript"],
    frameworks: ["React"],
    packageManagers: ["pnpm"],
    tags: [],
    sourceMtime: null,
    lastCommitAt: null,
    lastOpenedAt: null,
    updatedAt: "2026-08-31T00:00:00Z",
  };
}

function renderDialog(overrides: Partial<React.ComponentProps<typeof OnboardingDialog>> = {}) {
  const props = {
    open: true,
    step: "welcome" as const,
    t,
    locale: "en" as const,
    mcpInfo: installed,
    scanRoots: [root],
    selectedRootId: root.id,
    scanning: false,
    progress: null,
    waiting: false,
    checkFailed: false,
    notify: vi.fn(),
    onStep: vi.fn(),
    onCopyInstruction: vi.fn(async () => true),
    onChooseRoot: vi.fn(),
    onSelectRoot: vi.fn(),
    onStartScan: vi.fn(),
    onCancelScan: vi.fn(),
    onCheckNow: vi.fn(),
    onFinish: vi.fn(),
    onSkip: vi.fn(),
    onOpenChange: vi.fn(),
    ...overrides,
  };
  render(<OnboardingDialog {...props} />);
  return props;
}

describe("OnboardingDialog", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it("continues from welcome into the MCP path", () => {
    const { onStep } = renderDialog();
    fireEvent.click(screen.getByRole("button", { name: t("onboardingContinue") }));
    expect(onStep).toHaveBeenCalledWith("mcp");
  });

  it("falls back to manual discovery when MCP cannot launch", () => {
    const { onStep } = renderDialog({ mcpInfo: unavailable });
    fireEvent.click(screen.getByRole("button", { name: t("onboardingContinue") }));
    expect(onStep).toHaveBeenCalledWith("manual");
  });

  it("copies the MCP instruction and waits for a new project", async () => {
    const { onCopyInstruction, onStep, notify } = renderDialog({ step: "mcp" });
    fireEvent.click(screen.getByRole("button", { name: t("onboardingCopyAdd") }));
    await Promise.resolve();
    expect(onCopyInstruction).toHaveBeenCalledTimes(1);
    expect(onStep).toHaveBeenCalledWith("waiting");
    expect(notify).toHaveBeenCalledWith("success", t("onboardingCopiedAdd"));
  });

  it("stays on the MCP step when copying fails", async () => {
    const { onCopyInstruction, onStep, notify } = renderDialog({
      step: "mcp",
      onCopyInstruction: vi.fn(async () => false),
    });
    fireEvent.click(screen.getByRole("button", { name: t("onboardingCopyAdd") }));
    await Promise.resolve();
    expect(onCopyInstruction).toHaveBeenCalledTimes(1);
    expect(onStep).not.toHaveBeenCalled();
    expect(notify).toHaveBeenCalledWith("error", t("copyFailed"));
  });

  it("disables copy when MCP is unavailable", () => {
    renderDialog({ step: "mcp", mcpInfo: unavailable });
    expect(screen.getByRole("button", { name: t("onboardingCopyAdd") })).toBeDisabled();
    expect(screen.getByText(t("onboardingMcpUnavailable"))).toBeInTheDocument();
  });

  it("does not start discovery until a scan root exists", () => {
    const { onStartScan } = renderDialog({ step: "manual", scanRoots: [], selectedRootId: undefined });
    expect(screen.getByRole("button", { name: t("onboardingStartScan") })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: t("onboardingChooseRoot") }));
    expect(onStartScan).not.toHaveBeenCalled();
  });

  it("keeps the dialog open while a scan is running", () => {
    const { onSkip, notify } = renderDialog({ step: "manual", scanning: true });
    fireEvent.keyDown(document, { key: "Escape" });
    expect(onSkip).not.toHaveBeenCalled();
    expect(notify).toHaveBeenCalledWith("warning", t("onboardingCloseBlocked"));
  });

  it("lets a zero-result scan finish without forcing another directory", () => {
    const { onFinish } = renderDialog({ step: "manual", scanOutcome: "empty" });
    fireEvent.click(screen.getByRole("button", { name: t("onboardingFinishAnyway") }));
    expect(onFinish).toHaveBeenCalledTimes(1);
  });

  it("shows a newly added project and opens the library", () => {
    const found = project();
    const { onFinish } = renderDialog({ step: "waiting", foundProject: found });
    expect(screen.getByText("Atlas")).toBeInTheDocument();
    expect(screen.getByText("C:\\code\\atlas")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: t("onboardingEnterLibrary") }));
    expect(onFinish).toHaveBeenCalledTimes(1);
  });
});
