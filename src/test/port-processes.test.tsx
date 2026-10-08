import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { PortProcesses } from "../components/PortProcesses";
import { PortConflictDialog } from "../components/PortConflictDialog";
import { reviewPortConflicts } from "../lib/port-processes";
import { dictionaries, type MessageKey } from "../i18n";
import type { PortProcess } from "../types";

const mock = vi.hoisted(() => ({ listPortProcesses: vi.fn(), listProjects: vi.fn(), previewProcessStop: vi.fn(), confirmProcessStop: vi.fn(), associatePortProcess: vi.fn(), preflightTaskPorts: vi.fn() }));
vi.mock("../lib/api", () => ({ api: mock }));
vi.mock("../lib/task-runs", () => ({ refreshTaskRuns: vi.fn() }));
const t = (key: MessageKey) => dictionaries.en[key];
const row: PortProcess = { pid: 123, name: "node.exe", ports: [5173, 9229], projectId: null, attribution: "unknown", identity: { pid: 123, createdAt: "134000000000000001", executable: "C:\\tools\\node.exe" }, runId: null, restriction: null };
const plan = { token: "one-use", target: row, mode: "force", affected: [row], processCount: 1 };
beforeEach(() => {
  vi.resetAllMocks(); mock.listPortProcesses.mockResolvedValue({ supported: true, processes: [row] });
  mock.listProjects.mockResolvedValue([{ id: "p1", displayName: "Project Atlas", canonicalPath: "C:\\code\\atlas" }]);
  mock.previewProcessStop.mockResolvedValue(plan); mock.confirmProcessStop.mockResolvedValue("stopped");
  mock.preflightTaskPorts.mockResolvedValue([]); mock.associatePortProcess.mockResolvedValue(undefined);
});
function mount() { return render(<PortProcesses t={t} onJump={vi.fn()} />); }

describe("Windows port inventory", () => {
  const sortingRows: PortProcess[] = [
    { ...row, pid: 20, name: "node10.exe", ports: [9000, 80], projectId: "p1" },
    { ...row, pid: 3, name: "node2.exe", ports: [443], projectId: "p2" },
    { ...row, pid: 100, name: "node1.exe", ports: [3000] },
  ];
  const visiblePids = () => screen.getAllByRole("row").slice(1).map(item => Number(within(item).getAllByRole("cell")[2].textContent));
  it.each([
    ["Listening ports", [20, 3, 100], [100, 3, 20]],
    ["Process", [100, 3, 20], [20, 3, 100]],
    ["PID", [3, 20, 100], [100, 20, 3]],
    ["Project", [3, 20, 100], [20, 3, 100]],
  ])("sorts %s in both directions and exposes the current order", async (label, ascending, descending) => {
    mock.listPortProcesses.mockResolvedValue({ supported: true, processes: sortingRows });
    mock.listProjects.mockResolvedValue([{ id: "p1", displayName: "Project 10" }, { id: "p2", displayName: "Project 2" }]);
    mount(); await screen.findByText("node10.exe");
    const button = screen.getByRole("button", { name: label as string });
    if (label !== "Listening ports") fireEvent.click(button);
    expect(visiblePids()).toEqual(ascending);
    expect(button.closest("th")).toHaveAttribute("aria-sort", "ascending");
    fireEvent.click(button);
    expect(visiblePids()).toEqual(descending);
    expect(button.closest("th")).toHaveAttribute("aria-sort", "descending");
    expect(document.querySelectorAll("th[aria-sort]")).toHaveLength(1);
    expect(sortingRows[0].ports).toEqual([9000, 80]);
  });
  it("keeps sort, search and Project filter after refreshing a reordered snapshot", async () => {
    mock.listPortProcesses.mockResolvedValue({ supported: true, processes: sortingRows });
    mount(); await screen.findByText("node10.exe");
    fireEvent.click(screen.getByRole("button", { name: "PID" }));
    fireEvent.click(screen.getByRole("button", { name: "PID" }));
    fireEvent.click(screen.getByRole("button", { name: /Linked to Projects/ }));
    fireEvent.change(screen.getByRole("textbox", { name: t("processSearch") }), { target: { value: "node" } });
    expect(visiblePids()).toEqual([20, 3]);
    mock.listPortProcesses.mockResolvedValue({ supported: true, processes: [...sortingRows].reverse() });
    fireEvent.click(screen.getByRole("button", { name: t("processRecheck") }));
    await screen.findByText(t("processRefreshed"));
    expect(visiblePids()).toEqual([20, 3]);
    expect(screen.getByRole("textbox", { name: t("processSearch") })).toHaveValue("node");
    expect(screen.getByRole("button", { name: "PID" }).closest("th")).toHaveAttribute("aria-sort", "descending");
  });
  it("breaks equal sort values by PID regardless of snapshot order", async () => {
    mock.listPortProcesses.mockResolvedValue({ supported: true, processes: [{ ...row, pid: 20 }, { ...row, pid: 3 }] });
    mount(); await screen.findAllByText("node.exe");
    expect(visiblePids()).toEqual([3, 20]);
    fireEvent.click(screen.getByRole("button", { name: t("listeningPorts") }));
    expect(visiblePids()).toEqual([3, 20]);
  });
  it("shows a retry after initial failure without claiming the inventory is empty", async () => {
    mock.listPortProcesses.mockRejectedValueOnce("inspection_unavailable");
    mount();
    expect(await screen.findByRole("alert")).toHaveTextContent(t("portInspectionUnavailable"));
    expect(screen.queryByText(t("processEmpty"))).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: t("retry") }));
    expect(await screen.findByText("node.exe")).toBeVisible();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
  it("retains the last inventory with an explicit cached warning when refresh fails", async () => {
    mount(); await screen.findByText("node.exe");
    mock.listPortProcesses.mockRejectedValueOnce("inspection_unavailable");
    fireEvent.click(screen.getByRole("button", { name: t("processRecheck") }));
    expect(await screen.findByRole("alert")).toHaveTextContent(t("stateCached"));
    expect(screen.getByText("node.exe")).toBeVisible();
  });
  it("clears combined filters from the empty state and returns focus to search", async () => {
    mount(); await screen.findByText("node.exe");
    fireEvent.click(screen.getByRole("button", { name: /Linked to Projects/ }));
    const search = screen.getByRole("textbox", { name: t("processSearch") });
    fireEvent.change(search, { target: { value: "missing" } });
    const empty = screen.getByText(t("processEmpty")).closest(".empty-state")!;
    fireEvent.click(within(empty as HTMLElement).getByRole("button", { name: t("clear") }));
    expect(search).toHaveValue("");
    expect(search).toHaveFocus();
    expect(screen.getByText("node.exe")).toBeVisible();
    expect(screen.getByRole("button", { name: /All services/ })).toHaveAttribute("aria-pressed", "true");
  });
  it("filters linked and unknown Projects while retaining the search", async () => {
    mock.listPortProcesses.mockResolvedValue({ supported: true, processes: [row, { ...row, pid: 456, name: "vite.exe", ports: [3000], projectId: "p1", attribution: "cwd" }] });
    mount(); await screen.findByText("vite.exe");
    fireEvent.click(screen.getByRole("button", { name: /Linked to Projects/ }));
    expect(screen.getByText("vite.exe")).toBeVisible(); expect(screen.queryByText("node.exe")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /Unknown Project/ }));
    expect(screen.getByText("node.exe")).toBeVisible(); expect(screen.queryByText("vite.exe")).not.toBeInTheDocument();
    fireEvent.change(screen.getByRole("textbox", { name: t("processSearch") }), { target: { value: "3000" } });
    expect(screen.queryByText("node.exe")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /All services/ }));
    expect(screen.getByText("vite.exe")).toBeVisible(); expect(screen.queryByText("node.exe")).not.toBeInTheDocument();
  });
  it("shows unknown attribution, searches ports/PIDs, and explains unavailable external normal stop", async () => {
    mount(); await screen.findByText("node.exe");
    expect(screen.getByText("Unknown")).toBeInTheDocument(); expect(screen.queryByRole("button", { name: "Normal stop" })).not.toBeInTheDocument();
    expect(screen.getByText(t("processGracefulUnavailable"))).toBeVisible();
    fireEvent.change(screen.getByRole("textbox", { name: t("processSearch") }), { target: { value: "123" } }); expect(screen.getByText("node.exe")).toBeVisible();
    fireEvent.change(screen.getByRole("textbox", { name: t("processSearch") }), { target: { value: "9999" } }); expect(screen.queryByText("node.exe")).not.toBeInTheDocument();
  });
  it("previews every port and impact, then submits only the one-use confirmation", async () => {
    mount(); fireEvent.click(await screen.findByRole("button", { name: "Force terminate" }));
    const dialog = await screen.findByRole("alertdialog");
    expect(within(dialog).getByText(/5173, 9229/)).toBeVisible(); expect(within(dialog).getByText(t("processExternalScope"))).toBeVisible();
    expect(mock.confirmProcessStop).not.toHaveBeenCalled();
    fireEvent.click(within(dialog).getByRole("button", { name: t("processForceConfirm") }));
    await waitFor(() => expect(mock.confirmProcessStop).toHaveBeenCalledExactlyOnceWith("one-use"));
    await waitFor(() => expect(mock.listPortProcesses).toHaveBeenCalledTimes(2));
  });
  it.each(["identity_changed", "permission_denied", "process_exited", "scope_changed"])("reports %s and requires a new preview", async error => {
    mock.confirmProcessStop.mockRejectedValue(error); mount();
    fireEvent.click(await screen.findByRole("button", { name: "Force terminate" }));
    fireEvent.click(await screen.findByRole("button", { name: t("processForceConfirm") }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/process|Process|Permission|Listening/);
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(mock.confirmProcessStop).toHaveBeenCalledTimes(1);
  });
  it("keeps unverifiable targets visible with a compact restriction and accessible explanation", async () => {
    mock.listPortProcesses.mockResolvedValue({ supported: true, processes: [{ ...row, identity: null, restriction: "permission_denied" }] }); mount();
    await screen.findByText("node.exe"); expect(screen.queryByRole("button", { name: "Force terminate" })).not.toBeInTheDocument();
    const reason = screen.getByText(t("processStopPermissionDenied"));
    expect(reason).toBeVisible();
    expect(reason.closest("td")?.textContent).toBe(t("processStopPermissionDenied"));
    expect(screen.queryByText(t("processPermissionDenied"))).not.toBeInTheDocument();
    expect(screen.getByText(t("processPathPermissionDenied"))).toBeVisible();
    expect(screen.queryByText(t("processPathUnavailable"))).not.toBeInTheDocument();
    expect(mock.previewProcessStop).not.toHaveBeenCalled();
  });
  it("shows executable path, process source and attribution evidence without expanding anything", async () => {
    mount(); await screen.findByText("node.exe");
    expect(screen.getByText(row.identity!.executable)).toBeVisible();
    expect(screen.getByText(t("processEvidenceUnknown"))).toBeVisible();
    expect(screen.getByText(t("processExternal"))).toBeVisible();
    expect(mock.previewProcessStop).not.toHaveBeenCalled();
  });
  it("shows the Project checkout and association basis for a managed listener", async () => {
    mock.listPortProcesses.mockResolvedValue({ supported: true, processes: [{ ...row, projectId: "p1", attribution: "task", runId: "run1" }] });
    mount(); await screen.findByText("node.exe");
    expect(screen.getByText("C:\\code\\atlas")).toBeVisible();
    expect(screen.getByText(t("processEvidenceTask"))).toBeVisible();
    expect(screen.getByText(t("processManaged"))).toBeVisible();
  });
  it("associates only the chosen instance with a registered Project", async () => {
    mount(); fireEvent.click(await screen.findByRole("button", { name: t("processAssociate") }));
    const dialog = await screen.findByRole("alertdialog");
    fireEvent.change(within(dialog).getByRole("combobox"), { target: { value: "p1" } });
    fireEvent.click(within(dialog).getByRole("button", { name: t("save") }));
    await waitFor(() => expect(mock.associatePortProcess).toHaveBeenCalledExactlyOnceWith(row.identity, "p1"));
  });
  it("keeps normal stop as a request with no force fallback", async () => {
    mock.listPortProcesses.mockResolvedValue({ supported: true, processes: [{ ...row, runId: "run1", attribution: "task" }] });
    mock.previewProcessStop.mockResolvedValue({ ...plan, mode: "graceful", target: { ...row, runId: "run1" } }); mock.confirmProcessStop.mockResolvedValue("signal_sent");
    mount(); fireEvent.click(await screen.findByRole("button", { name: "Normal stop" }));
    fireEvent.click(await screen.findByRole("button", { name: t("processGracefulConfirm") }));
    expect(await screen.findByText(t("processSignalSent"))).toHaveAttribute("role", "status");
    expect(mock.previewProcessStop).toHaveBeenCalledExactlyOnceWith(row.identity, "graceful");
  });
  it("reuses task preflight after handling a conflict", async () => {
    render(<PortProcesses t={t} onJump={vi.fn()} context={{ projectId: "p1", taskId: "task1", ports: [5173] }} />);
    expect(await screen.findByText(t("processPortsClear"))).toBeVisible();
    expect(mock.preflightTaskPorts).toHaveBeenCalledWith("p1", "task1");
  });
});

it("port-conflict review locates the owner without starting the task", async () => {
  render(<PortConflictDialog t={t} />); const opened = vi.fn(); window.addEventListener("repoatlas:port-processes", opened);
  const result = reviewPortConflicts("p1", "task1", [{ port: 5173, pid: 123, processName: "node.exe" }]);
  fireEvent.click(await screen.findByRole("button", { name: t("processLocate") }));
  expect(await result).toBe(false); expect(opened).toHaveBeenCalledOnce(); window.removeEventListener("repoatlas:port-processes", opened);
});
