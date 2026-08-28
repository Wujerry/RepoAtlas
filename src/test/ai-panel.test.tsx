import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { dictionaries, type MessageKey } from "../i18n";
import type { AiMemoryItem, AiSummary, ProviderProfile } from "../types";
import { beforeEach, describe, expect, it, vi } from "vitest";

const {
  listProviderProfiles,
  latestSummary,
  listMemory,
  listSummaries,
  acceptSummaryMemory,
  addMemory,
  deleteMemory,
  analysisPlan,
  summarizeProject,
  askProject,
} = vi.hoisted(() => ({
  listProviderProfiles: vi.fn(),
  latestSummary: vi.fn(),
  listMemory: vi.fn(),
  listSummaries: vi.fn(),
  acceptSummaryMemory: vi.fn(),
  addMemory: vi.fn(),
  deleteMemory: vi.fn(),
  analysisPlan: vi.fn(),
  summarizeProject: vi.fn(),
  askProject: vi.fn(),
}));

vi.mock("../lib/api", () => ({
  api: {
    listProviderProfiles,
    latestSummary,
    listMemory,
    listSummaries,
    acceptSummaryMemory,
    addMemory,
    deleteMemory,
    analysisPlan,
    summarizeProject,
    askProject,
  },
}));

import { AiPanel } from "../components/AiPanel";

const t = (key: MessageKey) => dictionaries.en[key];
const notify = vi.fn();

const provider: ProviderProfile = {
  id: "provider-1",
  name: "Ollama",
  protocol: "ollama",
  baseUrl: "http://localhost:11434",
  model: "llama3.2",
  credentialRef: "",
  createdAt: "2026-08-21T00:00:00Z",
  updatedAt: "2026-08-21T00:00:00Z",
};

const memory: AiMemoryItem = {
  id: "memory-1",
  projectId: "project-1",
  text: "Keep the local scan root private.",
  createdAt: "2026-08-21T00:00:00Z",
};

function renderPanel(providers: ProviderProfile[] = [], memories: AiMemoryItem[] = [], onOpenSettings?: () => void, summaries: AiSummary[] = []) {
  listProviderProfiles.mockResolvedValue(providers);
  latestSummary.mockResolvedValue(null);
  listMemory.mockResolvedValue(memories);
  listSummaries.mockResolvedValue(summaries);
  acceptSummaryMemory.mockResolvedValue(memory);
  addMemory.mockResolvedValue(memory);
  deleteMemory.mockResolvedValue(undefined);
  analysisPlan.mockResolvedValue({
    providerId: provider.id,
    providerName: provider.name,
    model: provider.model,
    baseUrl: provider.baseUrl,
    evidenceFiles: ["README.md", "package.json"],
    characterCount: 3200,
    isLocal: true,
    suspiciousSecretCount: 1,
  });
  summarizeProject.mockResolvedValue(null);
  askProject.mockResolvedValue("");
  return render(<AiPanel projectId="project-1" t={t} notify={notify} onOpenSettings={onOpenSettings} />);
}

describe("AiPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("does not turn a loading failure into the empty provider state and can retry", async () => {
    listProviderProfiles.mockRejectedValueOnce(new Error("database unavailable")).mockResolvedValueOnce([]);
    latestSummary.mockResolvedValue(null);
    listMemory.mockResolvedValue([]);
    listSummaries.mockResolvedValue([]);

    render(<AiPanel projectId="project-1" t={t} notify={notify} />);

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("AI knowledge unavailable");
    expect(alert).toHaveTextContent("database unavailable");
    expect(screen.queryByText("No providers configured")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Retry/ }));
    expect(await screen.findByText("No providers configured")).toBeInTheDocument();
    expect(listProviderProfiles).toHaveBeenCalledTimes(2);
  });

  it("offers a clear settings CTA when no provider is configured", async () => {
    const onOpenSettings = vi.fn();
    renderPanel([], [], onOpenSettings);

    await screen.findByText("No providers configured");
    fireEvent.click(screen.getByRole("button", { name: /Open AI settings/ }));

    expect(onOpenSettings).toHaveBeenCalledTimes(1);
  });

  it("shows the exact analysis plan before sending project evidence", async () => {
    renderPanel([provider]);

    fireEvent.click(await screen.findByRole("button", { name: "Generate summary" }));
    const dialog = await screen.findByRole("alertdialog");

    expect(dialog).toHaveTextContent("README.md");
    expect(dialog).toHaveTextContent("package.json");
    expect(dialog).toHaveTextContent("Suspected secrets redacted: 1");
    expect(summarizeProject).not.toHaveBeenCalled();

    fireEvent.click(within(dialog).getByRole("button", { name: "Confirm and send" }));
    await waitFor(() => expect(summarizeProject).toHaveBeenCalledWith("project-1", provider.id, ""));
  });

  it("requires confirmation before deleting memory and ignores duplicate confirmation clicks", async () => {
    let resolveDelete: (() => void) | undefined;
    deleteMemory.mockImplementation(() => new Promise<void>((resolve) => { resolveDelete = resolve; }));
    renderPanel([provider], [memory]);

    await screen.findByText(memory.text);
    fireEvent.click(screen.getByRole("button", { name: "Remove" }));

    const dialog = await screen.findByRole("alertdialog");
    expect(dialog).toHaveTextContent("Remove AI memory");
    expect(deleteMemory).not.toHaveBeenCalled();

    const confirm = within(dialog).getByRole("button", { name: "Remove" });
    fireEvent.click(confirm);
    fireEvent.click(confirm);
    expect(deleteMemory).toHaveBeenCalledTimes(1);

    resolveDelete?.();
    await waitFor(() => expect(deleteMemory).toHaveBeenCalledWith(memory.id));
  });

  it("opens the saved evidence snapshot from summary history", async () => {
    const historical: AiSummary = {
      id: "summary-1",
      projectId: "project-1",
      providerId: provider.id,
      model: provider.model,
      evidenceSnapshot: "README.md\npackage.json#scripts.dev",
      text: "A local-first project atlas.",
      createdAt: "2026-08-21T00:00:00Z",
    };
    renderPanel([provider], [], undefined, [historical]);

    fireEvent.click(await screen.findByRole("button", { name: "Evidence snapshot" }));
    expect(await screen.findByRole("alertdialog")).toHaveTextContent("package.json#scripts.dev");
  });
});
