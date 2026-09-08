import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ModulesPanel } from "../components/ModulesPanel";
import { dictionaries, type MessageKey } from "../i18n";
import { api } from "../lib/api";
import type { ProjectDetail } from "../types";

vi.mock("../lib/api", () => ({ api: { promoteModule: vi.fn(), setDirectoryGroup: vi.fn(), resolveModulePath: vi.fn(), openInTerminal: vi.fn() } }));
const t = (key: MessageKey) => dictionaries.en[key];
const detail = {
  project: { id: "parent" },
  tasks: [{ id: "web-dev", name: "web · dev", cwd: "C:/system/web" }],
  modules: [{ id: "web", relativePath: "web", canonicalPath: "C:/system/web", languages: ["JavaScript"], frameworks: [], runtimeRequirements: [], facts: [], taskIds: ["web-dev"], observedAt: "2026-09-08T00:00:00Z", availability: "ready", projectId: null, evidence: "manifest-candidate" }],
} as unknown as ProjectDetail;

describe("Module actions", () => {
  beforeEach(() => vi.resetAllMocks());
  it("uses the validated Module path for terminal launch and the parent task ID for execution", async () => {
    vi.mocked(api.resolveModulePath).mockResolvedValue("C:/system/web");
    const onRunTask = vi.fn();
    render(<ModulesPanel detail={detail} t={t} notify={vi.fn()} onRefresh={vi.fn()} onRunTask={onRunTask} ides={[]} agents={[]} />);
    fireEvent.click(screen.getByText("web"));
    fireEvent.click(screen.getByRole("button", { name: t("openTerminal") }));
    await waitFor(() => expect(api.openInTerminal).toHaveBeenCalledWith("C:/system/web"));
    expect(api.resolveModulePath).toHaveBeenCalledWith("parent", "web");
    await waitFor(() => expect(screen.getByRole("button", { name: "web · dev" })).toBeEnabled());
    fireEvent.click(screen.getByRole("button", { name: "web · dev" }));
    expect(onRunTask).toHaveBeenCalledWith("web-dev");
  });
  it("reports promotion errors without refreshing and refreshes after success", async () => {
    const refresh = vi.fn(); const notify = vi.fn();
    vi.mocked(api.promoteModule).mockRejectedValueOnce(new Error("running tasks"));
    render(<ModulesPanel detail={detail} t={t} notify={notify} onRefresh={refresh} onRunTask={vi.fn()} ides={[]} agents={[]} />);
    fireEvent.click(screen.getByText("web"));
    fireEvent.click(screen.getByRole("button", { name: t("modulePromote") }));
    await waitFor(() => expect(notify).toHaveBeenCalledWith("error", t("moduleActionFailed"), "Error: running tasks"));
    expect(refresh).not.toHaveBeenCalled();
    vi.mocked(api.promoteModule).mockResolvedValue({ id: "child" } as never);
    fireEvent.click(screen.getByRole("button", { name: t("modulePromote") }));
    await waitFor(() => expect(refresh).toHaveBeenCalledOnce());
    expect(api.promoteModule).toHaveBeenCalledWith("parent", "web");
  });
});
