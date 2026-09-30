import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { dictionaries, type MessageKey } from "../i18n";
import type { AgentSession, SessionMessagePage, SessionSearchHit, SessionSearchResult, SessionSource } from "../lib/sessions";
import type { ProjectSummary, SearchHit } from "../types";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  searchProjects: vi.fn<typeof import("../lib/api").api.searchProjects>(),
  search: vi.fn<typeof import("../lib/sessions").sessionApi.search>(),
  sources: vi.fn<typeof import("../lib/sessions").sessionApi.sources>(),
  messages: vi.fn<typeof import("../lib/sessions").sessionApi.messages>(),
  resumeSpec: vi.fn<typeof import("../lib/sessions").sessionApi.resumeSpec>(),
  resume: vi.fn<typeof import("../lib/sessions").sessionApi.resume>(),
}));

vi.mock("../lib/api", () => ({
  api: { searchProjects: mocks.searchProjects },
}));
vi.mock("../lib/sessions", async importOriginal => ({
  ...await importOriginal<typeof import("../lib/sessions")>(),
  sessionApi: mocks,
}));

import { CommandPalette } from "../components/CommandPalette";

const t = (key: MessageKey) => dictionaries.en[key];
const date = "2026-09-30T08:00:00Z";
const enabledSource: SessionSource = {
  id: "source-1", adapter: "codex", path: "C:/history/codex", enabled: true, lastScannedAt: date, lastError: null,
};

function sessionHit(id = "session-1", changes: Partial<AgentSession> = {}): SessionSearchHit {
  return {
    session: {
      id, sourceId: enabledSource.id, adapter: "codex", externalId: `external-${id}`,
      projectId: "project-1", projectName: "Clinical Atlas", matchKind: "exact", cwd: "C:/code/project-1",
      title: `Session ${id}`, lastUserExcerpt: "Cached excerpt", startedAt: date, updatedAt: date,
      messageCount: 6, archived: false, sourceMissing: false, sourceLocator: `C:/history/${id}.jsonl`,
      revision: "revision-1", resumeReason: null, capabilities: { search: true, transcript: true, directResume: true },
      ...changes,
    },
    snippets: [{ index: 4, role: "user", content: `Snippet ${id}`, timestamp: date }],
  };
}
const results = (...items: SessionSearchHit[]): SessionSearchResult => ({ items, total: items.length });
const messages = (content: string): SessionMessagePage => ({
  items: [{ index: 4, role: "user", content, timestamp: date }], total: 6,
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

async function readyInput() {
  const input = await screen.findByRole("combobox");
  await waitFor(() => expect(input).toHaveAttribute("aria-busy", "false"));
  return input;
}

function project(id = "project-1"): ProjectSummary {
  return {
    id,
    canonicalPath: `C:/code/${id}`,
    displayName: "Clinical Atlas",
    detectedName: "Clinical Atlas",
    description: null,
    notes: null,
    vcsKind: "git",
    availability: "ready",
    archived: false,
    favorite: false,
    origin: "scan",
    scanRootId: "root-1",
    languages: ["TypeScript"],
    frameworks: ["React"],
    packageManagers: ["pnpm"],
    tags: [],
    sourceMtime: null,
    lastCommitAt: null,
    lastOpenedAt: null,
    updatedAt: "2026-08-21T00:00:00Z",
  };
}

function renderPalette(overrides: Partial<React.ComponentProps<typeof CommandPalette>> = {}) {
  const onClose = vi.fn();
  const runScan = vi.fn();
  const runJump = vi.fn();
  const currentProject = project();
  let props: React.ComponentProps<typeof CommandPalette> = {
    open: true, onClose, t, projects: [currentProject],
    actions: [
      { id: "scan-all", title: "Scan all", run: runScan },
      { id: `jump:${currentProject.id}`, title: "Jump to project", run: runJump },
    ],
    ...overrides,
  };
  const view = render(<CommandPalette {...props} />);
  return {
    ...view, onClose, runScan, runJump, currentProject,
    rerenderPalette(next: Partial<typeof props>) {
      props = { ...props, ...next };
      view.rerender(<CommandPalette {...props} />);
    },
  };
}

describe("CommandPalette", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mocks.searchProjects.mockResolvedValue([]);
    mocks.search.mockResolvedValue(results());
    mocks.sources.mockResolvedValue([enabledSource]);
    mocks.messages.mockImplementation(async id => messages(`Full transcript ${id}`));
    mocks.resumeSpec.mockImplementation(async id => ({
      agent: "codex", cwd: "C:/code/project-1", args: ["resume", `external-${id}`],
      command: `codex resume external-${id}`, env: {}, app: null,
    }));
    mocks.resume.mockResolvedValue(undefined);
  });

  it("focuses and clears the search field when opened", async () => {
    const { rerenderPalette } = renderPalette();

    const input = await readyInput();
    const listbox = screen.getByRole("listbox");
    await waitFor(() => expect(input).toHaveFocus());
    expect(input).toHaveValue("");
    expect(input).toHaveAttribute("aria-haspopup", "listbox");
    expect(input).toHaveAttribute("aria-controls", listbox.id);
    expect(input).toHaveAttribute("aria-activedescendant");
    expect(screen.getByRole("group", { name: t("commands") })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: t("projects") })).toBeInTheDocument();
    expect(screen.getAllByRole("group")).toEqual([
      screen.getByRole("group", { name: t("qsFilter") }),
      screen.getByRole("group", { name: t("projects") }),
      screen.getByRole("group", { name: t("commands") }),
    ]);
    expect(screen.getByRole("option", { name: /^Clinical Atlas/ })).toHaveAttribute("aria-selected", "true");
    expect(mocks.search).toHaveBeenCalledWith({ query: undefined, limit: 12, archived: false });
    expect(mocks.searchProjects).not.toHaveBeenCalled();

    fireEvent.change(input, { target: { value: "previous query" } });
    rerenderPalette({ open: false });
    rerenderPalette({ open: true });
    const reopened = await readyInput();
    expect(reopened).toHaveValue("");
    await waitFor(() => expect(reopened).toHaveFocus());
  });

  it("moves from a session to its Project with ArrowDown and executes the jump with Enter", async () => {
    mocks.search.mockResolvedValue(results(sessionHit()));
    const { onClose, runJump } = renderPalette();
    const input = await readyInput();
    const projectItem = await screen.findByRole("option", { name: /^Clinical Atlas/ });

    expect(projectItem).toHaveAttribute("aria-selected", "false");
    fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(projectItem).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(input, { key: "Enter" });

    expect(runJump).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
  });

  it("keeps projects discoverable when the caller supplies an explicit project handler", async () => {
    const onProject = vi.fn();
    const currentProject = project();
    const { onClose } = renderPalette({
      actions: [{ id: "scan-all", title: "Scan all", run: vi.fn() }],
      onProject,
    });

    await readyInput();
    const projectItem = screen.getByRole("option", { name: /^Clinical Atlas/ });
    fireEvent.click(projectItem);

    expect(onProject).toHaveBeenCalledWith(currentProject.id);
    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
  });

  it("searches across sessions and Projects, prioritizes both over commands, and highlights literal terms", async () => {
    const query = "a+b [x]";
    const currentProject = { ...project(), displayName: "Project A+B [x]" };
    const linked = sessionHit("linked", { title: "Investigate a+b [x]" });
    const unlinked = sessionHit("unlinked", { title: "Unlinked a+b [x]", projectId: null, projectName: null, adapter: "claude", archived: true });
    linked.snippets[0].content = 'a+b [x] <img src=x onerror="bad()">';
    mocks.searchProjects.mockResolvedValue([{ project: currentProject, score: 12 }]);
    mocks.search.mockImplementation(async request => request?.query ? results(linked, unlinked) : results());
    renderPalette({ projects: [], onProject: vi.fn(), actions: [{ id: "literal-command", title: "Search a+b [x]", run: vi.fn() }] });

    const input = await readyInput();
    fireEvent.change(input, { target: { value: query } });
    const first = await screen.findByRole("option", { name: /Investigate a\+b \[x\]/ });
    expect(mocks.searchProjects).toHaveBeenCalledWith(query);
    expect(mocks.search).toHaveBeenLastCalledWith({ query, limit: 12, archived: true });
    expect(screen.getAllByRole("group")).toEqual([
      screen.getByRole("group", { name: t("qsFilter") }),
      screen.getByRole("group", { name: t("projects") }),
      screen.getByRole("group", { name: t("qsSessions") }),
      screen.getByRole("group", { name: t("commands") }),
    ]);
    expect(screen.getByRole("option", { name: /Unlinked a\+b \[x\]/ })).toHaveTextContent("Claude Code");
    expect(screen.getAllByRole("option")).toHaveLength(4);
    expect(Array.from(first.querySelectorAll("mark"), mark => mark.textContent)).toEqual(["a+b", "[x]", "a+b", "[x]"]);
    const projectItem = screen.getByRole("option", { name: /Project A\+B \[x\]/ });
    expect(Array.from(projectItem.querySelectorAll("mark"), mark => mark.textContent)).toEqual(["A+B", "[x]"]);
    expect(first).toHaveTextContent('<img src=x onerror="bad()">');
    expect(first.querySelector("img, script")).toBeNull();
    expect(projectItem).toHaveAttribute("aria-selected", "true");
    fireEvent.click(first);
    await screen.findByText("Full transcript linked");
  });

  it("automatically previews the first session and keeps the palette open until explicit session navigation", async () => {
    const hit = sessionHit();
    const onSession = vi.fn();
    mocks.search.mockResolvedValue(results(hit));
    const { onClose } = renderPalette({ onSession });
    const input = await readyInput();
    const first = screen.getByRole("option", { name: /Session session-1/ });

    expect(first).toHaveAttribute("aria-selected", "true");
    expect(input).toHaveAttribute("aria-activedescendant", first.id);
    expect(await screen.findByText("Full transcript session-1")).toBeInTheDocument();
    expect(mocks.messages).toHaveBeenCalledWith(hit.session.id, 2, 5);
    fireEvent.click(first);
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onClose).not.toHaveBeenCalled();
    expect(onSession).not.toHaveBeenCalled();
    expect(mocks.resumeSpec).not.toHaveBeenCalled();
    expect(mocks.resume).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog", { name: t("qsTitle") })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: t("qsOpenSession") }));
    expect(onSession).toHaveBeenCalledWith(hit.session, 4);
    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
  });

  it("requires the preview's validated Resume action before dispatching an Agent", async () => {
    mocks.search.mockResolvedValue(results(sessionHit()));
    const { onClose } = renderPalette();
    await readyInput();
    fireEvent.click(screen.getByRole("button", { name: t("qsContinueIn").replace("{agent}", "Codex CLI") }));
    const dialog = within(await screen.findByRole("dialog", { name: t("ahResume") }));
    await dialog.findByText("codex resume external-session-1");
    expect(mocks.resumeSpec).toHaveBeenCalledWith("session-1");
    expect(mocks.resume).not.toHaveBeenCalled();
    fireEvent.click(dialog.getByRole("button", { name: t("ahLaunchNow") }));
    await dialog.findByText(t("ahStarted"));
    expect(mocks.resume).toHaveBeenCalledWith("session-1", "cli");
    expect(onClose).not.toHaveBeenCalled();
  });

  it.each([{ isComposing: true }, { keyCode: 229 }])("ignores IME Enter %j before a normal Enter opens a Project", async ime => {
    const { runJump, onClose } = renderPalette();
    const input = await readyInput();
    fireEvent.keyDown(input, { key: "Enter", ...ime });
    expect(runJump).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
    expect(runJump).toHaveBeenCalledTimes(1);
  });

  it("keeps commands keyboard accessible after Project results", async () => {
    const { runScan, onClose } = renderPalette();
    const input = await readyInput();
    fireEvent.keyDown(input, { key: "ArrowUp" });
    expect(screen.getByRole("option", { name: "Scan all" })).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(input, { key: "Enter" });
    expect(runScan).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
  });

  it.each(["resolve", "reject"] as const)("ignores late search %s after the query changes", async outcome => {
    const oldProjects = deferred<SearchHit[]>();
    const oldSessions = deferred<SessionSearchResult>();
    mocks.searchProjects.mockImplementation(query => query === "old" ? oldProjects.promise : Promise.resolve([{ project: { ...project("new"), displayName: "New Project" }, score: 1 }]));
    mocks.search.mockImplementation(async request => request?.query === "old" ? oldSessions.promise : request?.query ? results(sessionHit("new")) : results());
    renderPalette({ onProject: vi.fn() });
    const input = await readyInput();
    fireEvent.change(input, { target: { value: "old" } });
    await waitFor(() => expect(mocks.searchProjects).toHaveBeenCalledWith("old"));
    fireEvent.change(input, { target: { value: "new" } });
    await screen.findByRole("option", { name: /New Project/ });
    fireEvent.click(await screen.findByRole("option", { name: /Session new/ }));
    await screen.findByText((_, element) => element?.tagName === "PRE" && element.textContent === "Full transcript new");

    await act(async () => {
      if (outcome === "resolve") {
        oldProjects.resolve([{ project: { ...project("old"), displayName: "Old Project" }, score: 9 }]);
        oldSessions.resolve(results(sessionHit("old")));
      } else {
        oldProjects.reject(new Error("Old Project search failed"));
        oldSessions.reject(new Error("Old session search failed"));
      }
    });
    expect(screen.getByRole("option", { name: /New Project/ })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: /Session new/ })).toBeInTheDocument();
    expect(screen.queryByRole("option", { name: /Old Project|Session old/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(input).toHaveAttribute("aria-busy", "false");
  });

  it("retains Project matches on session search failure and lets the user retry", async () => {
    mocks.search.mockImplementation(async request => {
      if (request?.query) throw new Error("Session index unavailable");
      return results();
    });
    mocks.searchProjects.mockResolvedValue([{ project: project(), score: 12 }]);
    renderPalette({ onProject: vi.fn() });
    fireEvent.change(await readyInput(), { target: { value: "clinical" } });
    expect(await screen.findByRole("alert")).toHaveTextContent("Session index unavailable");
    expect(screen.getByRole("option", { name: /^Clinical Atlas/ })).toBeInTheDocument();
    mocks.search.mockResolvedValue(results(sessionHit()));
    fireEvent.click(screen.getByRole("button", { name: t("retry") }));
    fireEvent.click(await screen.findByRole("option", { name: /Session session-1/ }));
    await screen.findByText("Full transcript session-1");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByRole("option", { name: /^Clinical Atlas/ })).toBeInTheDocument();
  });

  it("keeps the palette open on async Project failure and prevents duplicate pending navigation", async () => {
    const navigation = deferred<void>();
    const onProject = vi.fn().mockReturnValueOnce(navigation.promise).mockResolvedValue(undefined);
    const { onClose } = renderPalette({ onProject });
    await readyInput();
    const item = screen.getByRole("option", { name: /^Clinical Atlas/ });
    fireEvent.click(item);
    fireEvent.click(item);
    expect(onProject).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("listbox")).toHaveAttribute("aria-busy", "true");
    expect(onClose).not.toHaveBeenCalled();
    await act(async () => navigation.reject(new Error("Project unavailable")));
    expect(screen.getByRole("alert")).toHaveTextContent("Project unavailable");
    expect(screen.getByRole("dialog", { name: t("qsTitle") })).toBeInTheDocument();
    expect(screen.getByRole("listbox")).toHaveAttribute("aria-busy", "false");
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.click(item);
    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it.each([{ sources: [] }, { sources: [{ ...enabledSource, enabled: false }] }])("explains empty or disabled Session Sources and opens their management view: $sources", async ({ sources }) => {
    mocks.sources.mockResolvedValue(sources);
    const onSources = vi.fn();
    const { onClose } = renderPalette({ onSources });
    await readyInput();
    expect(await screen.findByText(t("ahNoSources"))).toBeInTheDocument();
    expect(screen.getByText(t("ahNoSourcesHint"))).toBeInTheDocument();
    expect(screen.getByRole("option", { name: /^Clinical Atlas/ })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: t("ahSources") }));
    expect(onSources).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
  });

  it("ignores pending search and source reads after closing and reopening", async () => {
    const oldProjects = deferred<SearchHit[]>();
    const oldSessions = deferred<SessionSearchResult>();
    const oldSources = deferred<SessionSource[]>();
    mocks.sources.mockReturnValueOnce(oldSources.promise);
    const { rerenderPalette } = renderPalette({ standalone: true, onProject: vi.fn() });
    const input = await readyInput();
    mocks.searchProjects.mockReturnValueOnce(oldProjects.promise);
    mocks.search.mockReturnValueOnce(oldSessions.promise);
    fireEvent.change(input, { target: { value: "old" } });
    await waitFor(() => expect(mocks.searchProjects).toHaveBeenCalledWith("old"));
    rerenderPalette({ open: false });
    expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
    rerenderPalette({ open: true });
    await readyInput();
    await act(async () => {
      oldProjects.resolve([{ project: { ...project("old"), displayName: "Old Project" }, score: 1 }]);
      oldSessions.resolve(results(sessionHit("old")));
      oldSources.resolve([]);
    });
    expect(screen.getByRole("combobox")).toHaveValue("");
    expect(screen.queryByRole("option", { name: /Old Project|Session old/ })).not.toBeInTheDocument();
    expect(screen.queryByText(t("ahNoSources"))).not.toBeInTheDocument();
    expect(mocks.messages).not.toHaveBeenCalled();
  });

  it("filters result kinds without changing the query or stealing selection on pointer movement", async () => {
    mocks.search.mockResolvedValue(results(sessionHit("one"), sessionHit("two")));
    renderPalette();
    const input = await readyInput();
    const first = screen.getByRole("option", { name: /^Session one/ });
    const second = screen.getByRole("option", { name: /^Session two/ });
    fireEvent.pointerMove(second, { movementY: 3 });
    expect(first).toHaveAttribute("aria-selected", "true");
    fireEvent.click(screen.getByRole("button", { name: t("projects") }));
    expect(screen.queryByRole("option", { name: /^Session/ })).not.toBeInTheDocument();
    expect(screen.getByRole("option", { name: /^Clinical Atlas/ })).toBeInTheDocument();
    expect(input).toHaveValue("");
    fireEvent.click(screen.getByRole("button", { name: t("qsSessions") }));
    expect(screen.getAllByRole("option")).toHaveLength(2);
    expect(screen.queryByRole("option", { name: /^Clinical Atlas/ })).not.toBeInTheDocument();
  });

  it("recovers a cached protocol title from a bounded real user message and hides default snippets", async () => {
    mocks.search.mockResolvedValue(results(sessionHit("wrapped", {
      title: '<external_codex_apps_open_page>{"page_id":null}</external_codex_apps_open_page>',
      lastUserExcerpt: '<subagent_notification>{"status":"done"}</subagent_notification>',
    })));
    mocks.messages.mockImplementation(async (_id, _offset, limit) => limit === 20 ? {
      items: [
        { index: 0, role: "user", content: '<environment_context>setup</environment_context>', timestamp: date },
        { index: 1, role: "user", content: 'Fix the login redirect', timestamp: date },
      ], total: 6,
    } : messages('Useful preview'));
    renderPalette();
    expect(await screen.findByRole("option", { name: /^Fix the login redirect/ })).toBeInTheDocument();
    expect(mocks.messages).toHaveBeenCalledWith("wrapped", 0, 20);
    expect(screen.queryByText(/external_codex_apps_open_page|subagent_notification/)).not.toBeInTheDocument();
    expect(screen.queryByText("Snippet wrapped")).not.toBeInTheDocument();
  });

  it.each(["resolve", "reject"] as const)("discards transcript %s after the palette closes", async outcome => {
    const oldMessages = deferred<SessionMessagePage>();
    mocks.search.mockResolvedValueOnce(results(sessionHit("old"))).mockResolvedValue(results(sessionHit("new")));
    mocks.messages.mockReturnValueOnce(oldMessages.promise);
    const { rerenderPalette } = renderPalette({ standalone: true });
    await readyInput();
    await waitFor(() => expect(mocks.messages).toHaveBeenCalledWith("old", 2, 5));
    rerenderPalette({ open: false });
    rerenderPalette({ open: true });
    await screen.findByText((_, element) => element?.tagName === "PRE" && element.textContent === "Full transcript new");
    await act(async () => {
      if (outcome === "resolve") oldMessages.resolve(messages("Stale transcript"));
      else oldMessages.reject(new Error("Stale transcript failure"));
    });
    expect(screen.getByText("Full transcript new")).toBeInTheDocument();
    expect(screen.queryByText("Stale transcript")).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});
