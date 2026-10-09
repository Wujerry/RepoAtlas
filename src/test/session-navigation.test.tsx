import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { AIHistory } from "../components/AIHistory";
import { dictionaries, type MessageKey } from "../i18n";
import { openSessionHistory, type AgentSession, type SessionMessagePage, type SessionRefreshJob } from "../lib/sessions";

const mocks = vi.hoisted(() => ({
  sources: vi.fn(), refresh: vi.fn(), status: vi.fn(), search: vi.fn(), messages: vi.fn(), listProjects: vi.fn(),
}));
vi.mock("../lib/sessions", async importOriginal => ({
  ...await importOriginal<typeof import("../lib/sessions")>(),
  sessionApi: mocks,
}));
vi.mock("../lib/api", () => ({ api: { listProjects: mocks.listProjects } }));

const t = (key: MessageKey) => dictionaries.en[key];
const date = "2026-09-30T08:00:00Z";
const anchorScrollTop = 940;
const completedJob: SessionRefreshJob = { running: false, canceled: false, processed: 1, errors: 0, sourceId: null };
function session(id: string): AgentSession {
  return {
    id, sourceId: "authorized-source", adapter: "codex", externalId: `external-${id}`,
    projectId: null, projectName: null, matchKind: "unlinked", cwd: `F:\\projects\\${id}`,
    title: `Session ${id}`, lastUserExcerpt: "", startedAt: date, updatedAt: date, messageCount: 90,
    archived: false, sourceMissing: false, sourceLocator: `F:\\history\\${id}.jsonl`, revision: "revision-1",
    resumeReason: null, capabilities: { search: true, transcript: true, directResume: true },
  };
}
function page(id: string, offset: number, refreshed = false): SessionMessagePage {
  return {
    total: 90,
    items: Array.from({ length: 40 }, (_, position) => ({
      index: offset + position, role: position % 2 ? "assistant" : "user", timestamp: date,
      content: `${id} message ${offset + position}${refreshed ? " refreshed" : ""}`,
    })),
  };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(done => { resolve = done; });
  return { promise, resolve };
}

let backgroundRefresh = deferred<SessionRefreshJob>();
let frames = new Map<number, FrameRequestCallback>();
let nextFrame = 0;
let scrolls: { element: HTMLElement; options: boolean | ScrollIntoViewOptions | undefined; committed: boolean }[] = [];

function advanceFrame() {
  const pending = [...frames.values()];
  frames.clear();
  for (const frame of pending) frame(performance.now());
}

async function deliverMessages(request: ReturnType<typeof deferred<SessionMessagePage>>, result: SessionMessagePage) {
  await act(async () => {
    request.resolve(result);
    await Promise.resolve();
    // Exercise the desktop race: a frame may run after the promise callback but before React commits.
    // The old promise-owned anchor ran here against absent DOM and discarded the requested index.
    advanceFrame();
  });
  act(advanceFrame);
}

beforeEach(() => {
  vi.resetAllMocks();
  frames = new Map(); nextFrame = 0; scrolls = [];
  backgroundRefresh = deferred<SessionRefreshJob>();
  mocks.sources.mockResolvedValue([{
    id: "authorized-source", adapter: "codex", path: "F:\\history", enabled: true, lastScannedAt: date, lastError: null,
  }]);
  mocks.listProjects.mockResolvedValue([]);
  mocks.search.mockResolvedValue({ items: [], total: 0 });
  mocks.refresh.mockResolvedValue({ ...completedJob, running: true });
  mocks.status.mockReturnValue(backgroundRefresh.promise);
  vi.spyOn(window, "requestAnimationFrame").mockImplementation(callback => {
    const id = ++nextFrame; frames.set(id, callback); return id;
  });
  vi.spyOn(window, "cancelAnimationFrame").mockImplementation(id => { frames.delete(id); });
  vi.spyOn(HTMLElement.prototype, "scrollIntoView").mockImplementation(function (this: HTMLElement, options) {
    if (!this.classList.contains("ah-message")) return;
    scrolls.push({ element: this, options, committed: this.isConnected && this.closest(".ah-detail")?.getAttribute("aria-busy") === "false" });
    // jsdom has no layout scrolling. Model the browser effect without firing a synthetic scroll event:
    // AIHistory must save the anchor position itself before the background refresh replaces the page.
    const transcript = this.closest<HTMLElement>(".ah-transcript");
    if (transcript) transcript.scrollTop = anchorScrollTop;
  });
});

afterEach(() => {
  cleanup();
  frames.clear();
  vi.restoreAllMocks();
});

it("opens the linked Project and keeps the session visible on navigation failure",async()=>{
  const selected={...session("linked"),projectId:"project-42",projectName:"App"};
  mocks.messages.mockResolvedValue(page("linked",0));
  const navigate=vi.fn().mockRejectedValueOnce(new Error("gone")).mockResolvedValueOnce(undefined);
  render(<AIHistory t={t} onOpenProject={navigate}/>);
  await act(async()=>openSessionHistory(undefined,selected));
  const button=screen.getByRole("button",{name:/App.*Open project/});
  await act(async()=>fireEvent.click(button));
  expect(navigate).toHaveBeenCalledWith("project-42");
  expect(screen.getByRole("dialog")).toBeInTheDocument();
  expect(screen.getByText(t("projectLoadFailed"))).toBeInTheDocument();
  await act(async()=>fireEvent.click(button));
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
});

it("opens session details outside the reader without reloading messages or losing scroll", async () => {
  mocks.messages.mockResolvedValue(page("floating", 0));
  render(<AIHistory t={t}/>);
  await act(async () => openSessionHistory(undefined, { ...session("floating"), usage: {
    totalTokens: 1200, inputTokens: 1000, outputTokens: 200, cacheReadTokens: 600,
    cacheWriteTokens: null, reasoningTokens: 50, model: "sample", partial: false,
  }}));
  act(advanceFrame);
  const reader = document.querySelector<HTMLElement>(".ah-transcript")!;
  const message = document.getElementById("ah-message-0");
  reader.scrollTop = 340;
  fireEvent.scroll(reader);
  for (const label of ["usageDetails", "ahSessionDetails"] as const) {
    const trigger = screen.getByRole("button", { name: t(label) });
    await act(async () => fireEvent.click(trigger));
    act(advanceFrame);
    const popup = screen.getByRole("dialog", { name: t(label) });
    expect(popup.closest(".ah-detail")).toBeNull();
    expect(reader.scrollTop).toBe(340);
    expect(document.getElementById("ah-message-0")).toBe(message);
    expect(trigger).toHaveAttribute("aria-expanded", "true");
    await act(async () => fireEvent.click(within(popup).getByRole("button", { name: t("close") })));
    act(advanceFrame);
    expect(trigger).toHaveAttribute("aria-expanded", "false");
    expect(reader.scrollTop).toBe(340);
  }
  // The primary link action still opens metadata after replacing native <details>.
  await act(async () => fireEvent.click(screen.getByRole("button", { name: t("ahLink") })));
  expect(screen.getByRole("dialog", { name: t("ahSessionDetails") })).toBeInTheDocument();
  expect(mocks.messages).toHaveBeenCalledTimes(1);
  expect(scrolls).toHaveLength(0);
});

it("explains an oversized source record without exposing only an internal error code", async () => {
  mocks.sources.mockResolvedValue([{
    id: "authorized-source", adapter: "codex", path: "F:\\history", enabled: true,
    lastScannedAt: date, lastError: "session_line_limit",
  }]);
  render(<AIHistory t={t}/>);
  await act(async () => openSessionHistory());
  fireEvent.click(await screen.findByRole("button", { name: t("ahSources") }));
  expect(await screen.findByText(t("ahRecordTooLarge"), { exact: false })).toBeInTheDocument();
  expect(screen.queryByText("session_line_limit")).not.toBeInTheDocument();
});

it("does not clear a source failure when a session search succeeds", async () => {
  mocks.sources.mockRejectedValue(new Error("Source access failed"));
  render(<AIHistory t={t} initialOpen />);
  await waitFor(() => expect(mocks.search).toHaveBeenCalled());
  expect(screen.getByRole("status")).toHaveTextContent("Source access failed");
  expect(screen.queryByText(t("ahNoSources"), { selector: "h3" })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: t("retry") })).toBeInTheDocument();
});

it("does not present search failure as an empty result and can retry", async () => {
  mocks.search.mockRejectedValueOnce(new Error("Search unavailable"));
  render(<AIHistory t={t} initialOpen />);
  expect(await screen.findByText(/Search unavailable/)).toBeInTheDocument();
  expect(screen.queryByText(t("ahEmpty"), { selector: "h3" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: t("retry") }));
  await waitFor(() => expect(mocks.search).toHaveBeenCalledTimes(2));
  expect(screen.queryByText(/Search unavailable/)).not.toBeInTheDocument();
});

it("groups technical context by default and reveals an exact search-hit anchor",async()=>{
  const result:SessionMessagePage={total:3,items:[
    {index:0,role:"user",timestamp:"",content:"<environment_context>private context</environment_context>"},
    {index:1,role:"user",timestamp:"",content:"<subagent_notification>done</subagent_notification>"},
    {index:2,role:"user",timestamp:"",content:"Please fix navigation"},
  ]};
  mocks.messages.mockResolvedValue(result);
  render(<AIHistory t={t}/>);
  await act(async()=>openSessionHistory(undefined,session("technical")));
  act(advanceFrame);
  expect(document.querySelector(".ah-technical-group")).not.toHaveAttribute("open");
  expect(screen.getByText(t("ahSystemContext"))).toBeInTheDocument();
  await act(async()=>openSessionHistory(undefined,{...session("technical"),revision:"new"},1));
  act(advanceFrame);
  expect(document.querySelector(".ah-technical-group")).toHaveAttribute("open");
  expect(scrolls[scrolls.length-1]?.element.id).toBe("ah-message-1");
});

it("opens message 47 on page offset 40 after commit and retains its page and scroll position after automatic refresh", async () => {
  const selected = session("selected");
  const initial = deferred<SessionMessagePage>();
  const refreshed = deferred<SessionMessagePage>();
  mocks.messages.mockReturnValueOnce(initial.promise).mockReturnValueOnce(refreshed.promise);
  render(<AIHistory t={t} />);

  await act(async () => openSessionHistory(undefined, selected, 47));
  expect(mocks.messages).toHaveBeenCalledExactlyOnceWith("selected", 40);
  expect(mocks.refresh).toHaveBeenCalledExactlyOnceWith();
  expect(mocks.status).toHaveBeenCalledTimes(1);
  expect(document.getElementById("ah-message-47")).toBeNull();
  expect(scrolls).toEqual([]);

  await deliverMessages(initial, page("selected", 40));
  const target = document.getElementById("ah-message-47")!;
  expect(target).toHaveTextContent("selected message 47");
  expect(scrolls).toEqual([{ element: target, options: { block: "start" }, committed: true }]);
  expect(screen.getByText("41–80 / 90")).toBeInTheDocument();
  const transcript = target.closest<HTMLElement>(".ah-transcript")!;
  expect(transcript.scrollTop).toBe(anchorScrollTop);

  // Complete the automatically started indexing job; no manual refresh or second navigation event.
  await act(async () => backgroundRefresh.resolve(completedJob));
  expect(mocks.messages.mock.calls).toEqual([["selected", 40], ["selected", 40]]);
  expect(document.getElementById("ah-message-47")).toBeNull();
  // Removing rendered messages can clamp a real scroll container to zero while it is loading.
  transcript.scrollTop = 0;
  await deliverMessages(refreshed, page("selected", 40, true));
  expect(document.getElementById("ah-message-47")).toHaveTextContent("selected message 47 refreshed");
  expect(screen.getByText("41–80 / 90")).toBeInTheDocument();
  expect(transcript.scrollTop).toBe(anchorScrollTop);
  expect(scrolls).toHaveLength(1);
});

it.each(["before", "after"] as const)("ignores the previous session's pending messages when they finish %s the chosen page", async order => {
  const previous = deferred<SessionMessagePage>();
  const chosen = deferred<SessionMessagePage>();
  mocks.messages.mockReturnValueOnce(previous.promise).mockReturnValueOnce(chosen.promise);
  render(<AIHistory t={t} />);

  await act(async () => openSessionHistory(undefined, session("previous"), 6));
  await act(async () => openSessionHistory(undefined, session("chosen"), 47));
  expect(mocks.messages.mock.calls).toEqual([["previous", 0], ["chosen", 40]]);

  if (order === "before") {
    await deliverMessages(previous, page("previous", 0));
    expect(document.getElementById("ah-message-6")).toBeNull();
    expect(document.getElementById("ah-message-47")).toBeNull();
    expect(document.querySelector(".ah-detail")).toHaveAttribute("aria-busy", "true");
    expect(scrolls).toEqual([]);
  }
  await deliverMessages(chosen, page("chosen", 40));
  const target = document.getElementById("ah-message-47")!;
  expect(target).toHaveTextContent("chosen message 47");
  expect(scrolls).toEqual([{ element: target, options: { block: "start" }, committed: true }]);

  if (order === "after") await deliverMessages(previous, page("previous", 0));
  expect(screen.queryByText("previous message 6")).not.toBeInTheDocument();
  expect(document.getElementById("ah-message-47")).toBe(target);
  expect(screen.getByText("41–80 / 90")).toBeInTheDocument();
  expect(target.closest<HTMLElement>(".ah-transcript")!.scrollTop).toBe(anchorScrollTop);
  expect(scrolls).toHaveLength(1);
});
