import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { FootprintsDialog } from "../components/FootprintsDialog";
import { dictionaries } from "../i18n";
import { api } from "../lib/api";
import { dayRange, localDate, offsetDate } from "../lib/footprints";
import type { ActivityHistoryItem, ActivityHistoryResponse } from "../types";

// jsdom has no layout, so the real virtualizer reports an empty viewport and would render no rows at all.
vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: ({ count, estimateSize }: { count: number; estimateSize: (index: number) => number }) => ({
    getVirtualItems: () => Array.from({ length: count }, (_, index) => ({ index, key: index, start: 0, size: estimateSize(index) })),
    getTotalSize: () => 0, measure: () => {}, scrollToIndex: () => {},
  }),
}));
vi.mock("../lib/api", () => ({ api: {
  getActivityHistory: vi.fn(), getActivitySummary: vi.fn(), getActivityDetail: vi.fn(), readProjectIcons: vi.fn(async () => []),
  startGitHistoryRefresh: vi.fn(async () => ({ id: "job", state: "completed", total: 0, completed: 0, commits: 0, failures: [] })),
  getGitHistoryRefreshStatus: vi.fn(async () => ({ id: "job", state: "completed", total: 0, completed: 0, commits: 0, failures: [] })),
  cancelGitHistoryRefresh: vi.fn(async () => {}),
} }));
const today = localDate();
const item: ActivityHistoryItem = { id: "git:p1:abc", projectId: "p1", projectName: "RepoAtlas", canonicalPath: "F:/code/RepoAtlas", category: "git", kind: "commit", title: "A precise commit", occurredAt: new Date().toISOString(), source: "git_history", commitSha: "abcdef", commitShortSha: "abc" };
const response = (items: ActivityHistoryItem[]): ActivityHistoryResponse => ({ items, days: [], totalDays: 0 });
const props = () => ({ open: true, onOpenChange: vi.fn(), t: (key: keyof typeof dictionaries.en) => dictionaries.en[key], notify: vi.fn(), onJumpToProject: vi.fn(), onOpenTaskRun: vi.fn() });
beforeEach(() => {
  vi.clearAllMocks();
  HTMLElement.prototype.scrollTo = vi.fn();
  vi.mocked(api.getActivityHistory).mockResolvedValue(response([item]));
  vi.mocked(api.getActivityDetail).mockImplementation(async id => ({ ...item, id, detail: "Full commit body" }));
  vi.mocked(api.getActivitySummary).mockImplementation(async days => ({ days: days.map(d => ({ date: d.date, totalCount: d.date === today ? 1 : 0, gitCount: 1, taskCount: 0, toolCount: 0, maintenanceCount: 0 })), openCounts: days.map(() => 0), projectCounts: days.map(() => 1), latestAt: Date.now() / 1000, projects: [{ id: "p1", name: "RepoAtlas" }], coverage: [] }));
});
describe("Footprints time navigator", () => {
  it("ignores adjacent-day results after an explicit date change", async () => {
    let finish!: (value: ActivityHistoryResponse) => void;
    vi.mocked(api.getActivityHistory).mockResolvedValueOnce(response([item]))
      .mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }))
      .mockResolvedValue(response([{ ...item, id: "chosen", title: "Chosen date" }]));
    render(<FootprintsDialog {...props()} />);
    await screen.findByText("Full commit body");
    fireEvent.wheel(screen.getByRole("listbox"), { deltaY: 100 });
    fireEvent.change(screen.getByLabelText("Choose date", { selector: "input" }), { target: { value: offsetDate(today, -5) } });
    await screen.findByRole("option", { name: /Chosen date/ });
    await act(async () => finish(response([{ ...item, id: "late", title: "Stale adjacent day" }])));
    expect(screen.queryByRole("option", { name: /Stale adjacent day/ })).not.toBeInTheDocument();
    expect(screen.getByLabelText("Choose date", { selector: "input" })).toHaveValue(offsetDate(today, -5));
  });
  it("finishes same-day pagination before crossing the lower boundary", async () => {
    let finish!: (value: ActivityHistoryResponse) => void;
    vi.mocked(api.getActivityHistory).mockResolvedValueOnce({ ...response([item]), nextCursor: "page-two" })
      .mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
    render(<FootprintsDialog {...props()} />);
    await waitFor(() => expect(api.getActivityHistory).toHaveBeenCalledWith(expect.objectContaining({ ...dayRange(today), cursor: "page-two" })));
    fireEvent.wheel(screen.getByRole("listbox"), { deltaY: 100 });
    expect(api.getActivityHistory).toHaveBeenCalledTimes(2);
    await act(async () => finish(response([{ ...item, id: "second", title: "Second page" }])));
    expect(screen.getByLabelText("Choose date", { selector: "input" })).toHaveValue(today);
  });
  it("keeps the current list visible until scrolling into the previous day completes", async () => {
    let finish!: (value: ActivityHistoryResponse) => void;
    vi.mocked(api.getActivityHistory).mockResolvedValueOnce(response([item]))
      .mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
    render(<FootprintsDialog {...props()} />);
    await screen.findByText("Full commit body");
    fireEvent.wheel(screen.getByRole("listbox"), { deltaY: 100 });
    await waitFor(() => expect(api.getActivityHistory).toHaveBeenLastCalledWith(expect.objectContaining(dayRange(offsetDate(today, -1)))));
    expect(document.getElementById(`fp-row-${item.id}`)).toBeInTheDocument();
    expect(screen.getByLabelText("Choose date", { selector: "input" })).toHaveValue(today);
    await act(async () => finish(response([{ ...item, id: "yesterday", title: "Yesterday entry" }])));
    await screen.findByRole("option", { name: /Yesterday entry/ });
    expect(screen.getByLabelText("Choose date", { selector: "input" })).toHaveValue(offsetDate(today, -1));
    expect(api.getActivityHistory).toHaveBeenCalledTimes(2);
  });
  it("scrolls upward into the next day but never past today", async () => {
    render(<FootprintsDialog {...props()} />);
    await screen.findByText("Full commit body");
    fireEvent.change(screen.getByLabelText("Choose date", { selector: "input" }), { target: { value: offsetDate(today, -1) } });
    await waitFor(() => expect(api.getActivityHistory).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(screen.getByRole("listbox")).toHaveAttribute("aria-busy", "false"));
    fireEvent.wheel(screen.getByRole("listbox"), { deltaY: -100 });
    await waitFor(() => expect(screen.getByLabelText("Choose date", { selector: "input" })).toHaveValue(today));
    expect(api.getActivityHistory).toHaveBeenCalledTimes(3);
    fireEvent.wheel(screen.getByRole("listbox"), { deltaY: -100 });
    expect(api.getActivityHistory).toHaveBeenCalledTimes(3);
  });
  it("retains the current date and records when adjacent loading fails", async () => {
    vi.mocked(api.getActivityHistory).mockResolvedValueOnce(response([item])).mockRejectedValueOnce(new Error("history unavailable"));
    render(<FootprintsDialog {...props()} />);
    await screen.findByText("Full commit body");
    fireEvent.wheel(screen.getByRole("listbox"), { deltaY: 100 });
    await screen.findByText("Error: history unavailable");
    expect(document.getElementById(`fp-row-${item.id}`)).toBeInTheDocument();
    expect(screen.getByLabelText("Choose date", { selector: "input" })).toHaveValue(today);
  });
  it("renders thirty calendar days and loads full detail independently", async () => {
    const p = props(); render(<FootprintsDialog {...p} />);
    expect(await screen.findByText("Full commit body", {}, { timeout: 5000 })).toBeInTheDocument();
    expect(screen.getAllByRole("tab")).toHaveLength(30);
    expect(api.getActivityHistory).toHaveBeenCalledWith(expect.objectContaining(dayRange(today)));
    fireEvent.click(screen.getByRole("button", { name: /Open Project/i }));
    expect(p.onJumpToProject).toHaveBeenCalledWith("p1");
  });
  it("marks every activity row with the stored project thumbnail and its name", async () => {
    vi.mocked(api.readProjectIcons).mockResolvedValue([{ projectId: "p1", kind: "override", source: "icon.png", mimeType: "image/png", dataUrl: "data:image/png;base64,AAAA" }]);
    render(<FootprintsDialog {...props()} />);
    await screen.findByText("Full commit body", {}, { timeout: 5000 });
    await waitFor(() => {
      const row = document.getElementById("fp-row-git:p1:abc");
      expect(row?.querySelector(".fp-project-mark img")?.getAttribute("src")).toBe("data:image/png;base64,AAAA");
      expect(row?.querySelector(".fp-project-name")?.textContent).toBe("RepoAtlas");
      expect(row?.querySelector(".fp-project")?.textContent).toBe("RepoAtlas");
    });
    const detail = document.querySelector(".fp-project-value");
    expect(detail?.textContent).toBe("RepoAtlas");
    expect(detail?.querySelector(".fp-project-mark")).toBeTruthy();
  });
  it("falls back to the project language glyph when no icon is stored", async () => {
    vi.mocked(api.readProjectIcons).mockResolvedValue([{ projectId: "p1", kind: "language", source: "typescript", mimeType: null, dataUrl: null }]);
    render(<FootprintsDialog {...props()} />);
    await screen.findByText("Full commit body", {}, { timeout: 5000 });
    await waitFor(() => {
      const mark = document.getElementById("fp-row-git:p1:abc")?.querySelector(".fp-project-mark");
      expect(mark?.querySelector("svg")).toBeTruthy();
      expect(mark?.querySelector("img")).toBeNull();
    });
  });
  it("searches through the backend instead of filtering the first page", async () => {
    render(<FootprintsDialog {...props()} />);
    fireEvent.change(screen.getByRole("textbox", { name: /Search titles/i }), { target: { value: "beyond first hundred" } });
    await waitFor(() => expect(api.getActivityHistory).toHaveBeenCalledWith(expect.objectContaining({ search: "beyond first hundred" })));
  });
  it("ignores an older response after changing the selected date", async () => {
    let resolveOld!: (r: ActivityHistoryResponse) => void;
    vi.mocked(api.getActivityHistory).mockImplementationOnce(() => new Promise(resolve => { resolveOld = resolve; }));
    render(<FootprintsDialog {...props()} />);
    fireEvent.change(screen.getByLabelText("Choose date", { selector: "input" }), { target: { value: offsetDate(today, -1) } });
    await screen.findByText("Full commit body", {}, { timeout: 5000 });
    await act(async () => { resolveOld(response([{ ...item, id: "old", title: "Outdated result" }])); });
    expect(screen.queryByText("Outdated result")).not.toBeInTheDocument();
  });
  it("does not report a clipboard failure as success", async () => {
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: vi.fn().mockRejectedValue(new Error("denied")) } });
    const p = props(); render(<FootprintsDialog {...p} />); await screen.findByText("Full commit body", {}, { timeout: 5000 });
    fireEvent.click(screen.getByRole("button", { name: dictionaries.en.copyCommitSha }));
    await waitFor(() => expect(p.notify).toHaveBeenCalledWith("error", "Copy failed", expect.any(String)));
    expect(p.notify).not.toHaveBeenCalledWith("success", expect.anything());
  });
  it("renders an empty day without silently selecting another date", async () => {
    vi.mocked(api.getActivityHistory).mockResolvedValue(response([]));
    render(<FootprintsDialog {...props()} />);
    expect(await screen.findByText("No activity recorded for this date")).toBeInTheDocument();
    expect(screen.getByLabelText("Choose date", { selector: "input" })).toHaveValue(today);
  });
  it("cancels an automatic collection that starts after the workspace closes", async () => {
    let finish!: (value: Awaited<ReturnType<typeof api.startGitHistoryRefresh>>) => void;
    vi.mocked(api.startGitHistoryRefresh).mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
    const p = props();
    const view = render(<FootprintsDialog {...p} />);
    await waitFor(() => expect(api.startGitHistoryRefresh).toHaveBeenCalled(), { timeout: 5000 });
    view.rerender(<FootprintsDialog {...p} open={false} />);
    await act(async () => finish({ id: "late-auto", state: "running", total: 1, completed: 0, commits: 0, failures: [] }));
    await waitFor(() => expect(api.cancelGitHistoryRefresh).toHaveBeenCalledWith("late-auto"));
  });
});
