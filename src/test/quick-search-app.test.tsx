import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import QuickSearchApp from "../QuickSearchApp";
import { QuickSearchShortcut } from "../components/QuickSearchShortcut";
import { dictionaries, type MessageKey } from "../i18n";
import type { QuickSearchStatus, QuickSearchTarget } from "../lib/quick-search";
import type { SessionSearchHit } from "../lib/sessions";
import type { AppSettings, ProjectSummary } from "../types";

const mocks = vi.hoisted(() => ({
  isTauri: vi.fn(),
  getSettings: vi.fn<typeof import("../lib/api").api.getSettings>(),
  listProjects: vi.fn<typeof import("../lib/api").api.listProjects>(),
  searchProjects: vi.fn<typeof import("../lib/api").api.searchProjects>(),
  search: vi.fn<typeof import("../lib/sessions").sessionApi.search>(),
  sources: vi.fn<typeof import("../lib/sessions").sessionApi.sources>(),
  messages: vi.fn<typeof import("../lib/sessions").sessionApi.messages>(),
  resumeSpec: vi.fn<typeof import("../lib/sessions").sessionApi.resumeSpec>(),
  resume: vi.fn<typeof import("../lib/sessions").sessionApi.resume>(),
  sourcesChanged: vi.fn<typeof import("../lib/sessions").onSessionSourcesChanged>(),
  connect: vi.fn<typeof import("../lib/quick-search").connectQuickSearchRequests>(),
  statusChanged: vi.fn<typeof import("../lib/quick-search").onQuickSearchStatusChanged>(),
  status: vi.fn<typeof import("../lib/quick-search").quickSearchApi.status>(),
  hide: vi.fn<typeof import("../lib/quick-search").quickSearchApi.hide>(),
  openMain: vi.fn<typeof import("../lib/quick-search").quickSearchApi.openMain>(),
  show: vi.fn<typeof import("../lib/quick-search").quickSearchApi.show>(),
  retryRegistration: vi.fn<typeof import("../lib/quick-search").quickSearchApi.retryRegistration>(),
  disposeRequests: vi.fn(),
  disposeStatus: vi.fn(),
  disposeSources: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", async importOriginal => ({
  ...await importOriginal<typeof import("@tauri-apps/api/core")>(), isTauri: mocks.isTauri,
}));
vi.mock("../lib/api", () => ({ api: {
  getSettings: mocks.getSettings, listProjects: mocks.listProjects, searchProjects: mocks.searchProjects,
} }));
vi.mock("../lib/sessions", async importOriginal => ({
  ...await importOriginal<typeof import("../lib/sessions")>(),
  onSessionSourcesChanged: mocks.sourcesChanged,
  sessionApi: { search: mocks.search, sources: mocks.sources, messages: mocks.messages, resumeSpec: mocks.resumeSpec, resume: mocks.resume },
}));
vi.mock("../lib/quick-search", () => ({
  connectQuickSearchRequests: mocks.connect,
  onQuickSearchStatusChanged: mocks.statusChanged,
  quickSearchApi: { status: mocks.status, hide: mocks.hide, openMain: mocks.openMain, show: mocks.show, retryRegistration: mocks.retryRegistration },
}));

const t = (key: MessageKey) => dictionaries.en[key];
const date = "2026-09-30T08:00:00Z";
const settings: AppSettings = { theme: "light", locale: "en", uiFont: "Geist", consoleFont: "" };
const registered: QuickSearchStatus = { shortcut: "Ctrl+Shift+K", registered: true, error: null };
const project: ProjectSummary = {
  id: "project-1", canonicalPath: "C:/code/atlas", displayName: "Local Atlas", detectedName: "Local Atlas",
  description: null, notes: null, vcsKind: "git", availability: "ready", archived: false, favorite: false,
  origin: "scan", scanRootId: "root-1", languages: ["TypeScript"], frameworks: ["React"], packageManagers: ["pnpm"],
  tags: [], sourceMtime: null, lastCommitAt: null, lastOpenedAt: null, updatedAt: date,
};
const hit: SessionSearchHit = {
  session: {
    id: "session-1", sourceId: "source-1", adapter: "codex", externalId: "external-session-1", projectId: project.id,
    projectName: project.displayName, matchKind: "exact", cwd: project.canonicalPath, title: "Fix session search",
    lastUserExcerpt: "Cached excerpt", startedAt: date, updatedAt: date, messageCount: 6, archived: false,
    sourceMissing: false, sourceLocator: "C:/history/session.jsonl", revision: "revision-1", resumeReason: null,
    capabilities: { search: true, transcript: true, directResume: true },
  },
  snippets: [{ index: 4, role: "user", content: "Search snippet", timestamp: date }],
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

async function focusWindow(requestId = "focus-1") {
  await act(async () => { await mocks.connect.mock.calls[0][0]({ requestId, action: "focus" }); });
  const input = await screen.findByRole("combobox");
  await waitFor(() => expect(input).toHaveAttribute("aria-busy", "false"));
  return input;
}

let originalTitle: string;
let originalLang: string;
let originalTheme: string | undefined;
let originalFont: string;

beforeEach(() => {
  originalTitle = document.title;
  originalLang = document.documentElement.lang;
  originalTheme = document.documentElement.dataset.theme;
  originalFont = document.documentElement.style.getPropertyValue("--ui-font");
  vi.resetAllMocks();
  mocks.isTauri.mockReturnValue(true);
  mocks.getSettings.mockResolvedValue(settings);
  mocks.listProjects.mockResolvedValue([project]);
  mocks.searchProjects.mockResolvedValue([]);
  mocks.search.mockResolvedValue({ items: [], total: 0 });
  mocks.sources.mockResolvedValue([{ id: "source-1", adapter: "codex", path: "C:/history", enabled: true, lastScannedAt: date, lastError: null }]);
  mocks.messages.mockResolvedValue({ items: [{ index: 4, role: "user", content: "Complete session message", timestamp: date }], total: 6 });
  mocks.resumeSpec.mockResolvedValue({ agent: "codex", cwd: project.canonicalPath, args: ["resume", "external-session-1"], command: "codex resume external-session-1", env: {}, app: null });
  mocks.resume.mockResolvedValue(undefined);
  mocks.sourcesChanged.mockResolvedValue(mocks.disposeSources);
  mocks.connect.mockResolvedValue(mocks.disposeRequests);
  mocks.statusChanged.mockResolvedValue(mocks.disposeStatus);
  mocks.status.mockResolvedValue(registered);
  mocks.hide.mockResolvedValue(undefined);
  mocks.openMain.mockResolvedValue("navigation-1");
  mocks.show.mockResolvedValue(undefined);
  mocks.retryRegistration.mockResolvedValue(registered);
});

afterEach(() => {
  document.title = originalTitle;
  document.documentElement.lang = originalLang;
  if (originalTheme === undefined) delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = originalTheme;
  if (originalFont) document.documentElement.style.setProperty("--ui-font", originalFont);
  else document.documentElement.style.removeProperty("--ui-font");
});

describe("QuickSearchApp", () => {
  it("loads preferences and cached Projects on focus, and resets the search on the next focus", async () => {
    render(<QuickSearchApp />);
    expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
    expect(mocks.listProjects).not.toHaveBeenCalled();
    const input = await focusWindow();
    expect(mocks.listProjects).toHaveBeenCalledWith({ section: "recent", limit: 8 });
    expect(screen.getByRole("main", { name: t("qsTitle") })).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByRole("option", { name: /Local Atlas/ })).toBeInTheDocument();
    expect(document.documentElement).toHaveAttribute("data-theme", "light");
    expect(document.documentElement).toHaveAttribute("lang", "en");
    expect(document.documentElement.style.getPropertyValue("--ui-font")).toBe('"Geist", var(--font-sans)');
    expect(document.title).toBe(t("qsTitle"));
    expect(mocks.searchProjects).not.toHaveBeenCalled();
    expect(mocks.resumeSpec).not.toHaveBeenCalled();
    expect(mocks.resume).not.toHaveBeenCalled();
    fireEvent.change(input, { target: { value: "previous query" } });
    const reopened = await focusWindow("focus-2");
    expect(reopened).toHaveValue("");
    await waitFor(() => expect(reopened).toHaveFocus());
    expect(mocks.listProjects).toHaveBeenCalledTimes(2);
  });

  it("keeps the window open on blur and failed Escape hide, then allows Close to retry", async () => {
    mocks.hide.mockRejectedValueOnce(new Error("Window hide failed"));
    render(<QuickSearchApp />);
    const input = await focusWindow();
    fireEvent(window, new Event("blur"));
    expect(mocks.hide).not.toHaveBeenCalled();
    fireEvent.keyDown(input, { key: "Escape" });
    expect(await screen.findByRole("alert")).toHaveTextContent("Window hide failed");
    expect(screen.getByRole("combobox")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: t("close") }));
    await waitFor(() => expect(screen.queryByRole("combobox")).not.toBeInTheDocument());
    expect(mocks.hide).toHaveBeenCalledTimes(2);
    await focusWindow("focus-2");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  const destinations: { action: "project" | "session" | "sources" | "more"; target: QuickSearchTarget }[] = [
    { action: "project", target: { kind: "project", projectId: project.id } },
    { action: "session", target: { kind: "sessions", sessionId: hit.session.id, messageIndex: 4 } },
    { action: "sources", target: { kind: "sessions", sources: true } },
    { action: "more", target: { kind: "sessions" } },
  ];
  it.each(destinations)("routes $action to the main window and hides only after successful navigation", async ({ action, target }) => {
    const navigation = deferred<string>();
    mocks.openMain.mockReturnValueOnce(navigation.promise);
    if (action === "session") mocks.search.mockResolvedValue({ items: [hit], total: 1 });
    render(<QuickSearchApp />);
    await focusWindow();
    if (action === "project") fireEvent.click(screen.getByRole("option", { name: /Local Atlas/ }));
    else if (action === "session") {
      await screen.findByText("Complete session message");
      expect(mocks.openMain).not.toHaveBeenCalled();
      fireEvent.click(screen.getByRole("button", { name: t("qsOpenSession") }));
    } else if (action === "sources") fireEvent.click(screen.getByRole("button", { name: t("ahSources") }));
    else fireEvent.click(screen.getByRole("option", { name: t("ahMore") }));
    expect(mocks.openMain).toHaveBeenCalledWith(target);
    expect(mocks.hide).not.toHaveBeenCalled();
    expect(screen.getByRole("combobox")).toBeInTheDocument();
    await act(async () => navigation.resolve("navigation-1"));
    await waitFor(() => expect(screen.queryByRole("combobox")).not.toBeInTheDocument());
    expect(mocks.hide).toHaveBeenCalledTimes(1);
    expect(mocks.resume).not.toHaveBeenCalled();
  });

  it("shows main-window navigation failure without hiding and retries the same Project", async () => {
    mocks.openMain.mockRejectedValueOnce(new Error("Main window unavailable"));
    render(<QuickSearchApp />);
    await focusWindow();
    const item = screen.getByRole("option", { name: /Local Atlas/ });
    fireEvent.click(item);
    expect(await screen.findByRole("alert")).toHaveTextContent("Main window unavailable");
    expect(screen.getByRole("combobox")).toBeInTheDocument();
    expect(mocks.hide).not.toHaveBeenCalled();
    fireEvent.click(item);
    await waitFor(() => expect(screen.queryByRole("combobox")).not.toBeInTheDocument());
    expect(mocks.openMain.mock.calls).toEqual([[{ kind: "project", projectId: project.id }], [{ kind: "project", projectId: project.id }]]);
  });

  it.each(["preferences", "projects"] as const)("reports a partial $0 load failure while keeping successful cached data usable", async failure => {
    if (failure === "preferences") mocks.getSettings.mockRejectedValueOnce(new Error("Preferences unavailable"));
    else {
      mocks.listProjects.mockRejectedValueOnce(new Error("Projects unavailable"));
      mocks.search.mockResolvedValue({ items: [hit], total: 1 });
    }
    render(<QuickSearchApp />);
    await focusWindow();
    expect(screen.getByRole("alert")).toHaveTextContent(failure === "preferences" ? "Preferences unavailable" : "Projects unavailable");
    if (failure === "preferences") expect(screen.getByRole("option", { name: /Local Atlas/ })).toBeInTheDocument();
    else await screen.findByText("Complete session message");
    expect(screen.getByRole("button", { name: t("ahSources") })).toBeEnabled();
  });

  it("surfaces connection failure with an available close action", async () => {
    mocks.connect.mockRejectedValueOnce(new Error("Request listener unavailable"));
    render(<QuickSearchApp />);
    expect(await screen.findByRole("alert")).toHaveTextContent("Request listener unavailable");
    await screen.findByRole("combobox");
    fireEvent.click(screen.getByRole("button", { name: t("close") }));
    await waitFor(() => expect(screen.queryByRole("combobox")).not.toBeInTheDocument());
    expect(mocks.hide).toHaveBeenCalledTimes(1);
  });

  it("reports request delivery errors and disposes the listener when unmounted", async () => {
    const view = render(<QuickSearchApp />);
    await focusWindow();
    act(() => mocks.connect.mock.calls[0][1](new Error("Request delivery failed")));
    expect(screen.getByRole("alert")).toHaveTextContent("Request delivery failed");
    view.unmount();
    await waitFor(() => expect(mocks.disposeRequests).toHaveBeenCalledTimes(1));
    expect(mocks.disposeStatus).toHaveBeenCalledTimes(1);
  });

  it("disposes a connection that resolves after unmount and ignores late focus requests", async () => {
    const connection = deferred<() => void>();
    mocks.connect.mockReturnValueOnce(connection.promise);
    const view = render(<QuickSearchApp />);
    view.unmount();
    await act(async () => {
      connection.resolve(mocks.disposeRequests);
      await mocks.connect.mock.calls[0][0]({ requestId: "late", action: "focus" });
    });
    expect(mocks.disposeRequests).toHaveBeenCalledTimes(1);
    expect(mocks.getSettings).not.toHaveBeenCalled();
    expect(mocks.listProjects).not.toHaveBeenCalled();
  });

  it("discards cached data reads that finish after unmount", async () => {
    const preferences = deferred<AppSettings>();
    const projects = deferred<ProjectSummary[]>();
    mocks.getSettings.mockReturnValueOnce(preferences.promise);
    mocks.listProjects.mockReturnValueOnce(projects.promise);
    const view = render(<QuickSearchApp />);
    let loading!: void | Promise<void>;
    act(() => { loading = mocks.connect.mock.calls[0][0]({ requestId: "focus-1", action: "focus" }); });
    view.unmount();
    await act(async () => {
      preferences.resolve({ ...settings, theme: "dark", locale: "zh" });
      projects.resolve([project]);
      await loading;
    });
    expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
    expect(document.documentElement.lang).toBe("en");
    expect(mocks.disposeRequests).toHaveBeenCalledTimes(1);
  });

  it.each(["resolve", "reject"] as const)("ignores stale hide %s after a newer focus request", async outcome => {
    const hiding = deferred<void>();
    mocks.hide.mockReturnValueOnce(hiding.promise);
    render(<QuickSearchApp />);
    const input = await focusWindow();
    fireEvent.keyDown(input, { key: "Escape" });
    expect(mocks.hide).toHaveBeenCalledTimes(1);
    await focusWindow("focus-2");
    await act(async () => {
      if (outcome === "resolve") hiding.resolve();
      else hiding.reject(new Error("Previous hide failed"));
    });
    expect(screen.queryByRole("combobox")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("does not hide the new palette when a previous main-window navigation completes", async () => {
    const navigation = deferred<string>();
    mocks.openMain.mockReturnValueOnce(navigation.promise);
    render(<QuickSearchApp />);
    await focusWindow();
    fireEvent.click(screen.getByRole("option", { name: /Local Atlas/ }));
    await focusWindow("focus-2");
    await act(async () => navigation.resolve("old-navigation"));
    expect(mocks.hide).not.toHaveBeenCalled();
    expect(screen.queryByRole("combobox")).toBeInTheDocument();
  });
});

describe("QuickSearchShortcut", () => {
  it("omits native shortcut controls outside Tauri", () => {
    mocks.isTauri.mockReturnValue(false);
    const { container } = render(<QuickSearchShortcut t={t} />);
    expect(container).toBeEmptyDOMElement();
    expect(mocks.status).not.toHaveBeenCalled();
    expect(mocks.statusChanged).not.toHaveBeenCalled();
  });

  it("updates the compact unavailable hint when shortcut registration recovers", async () => {
    mocks.status.mockResolvedValueOnce({ ...registered, registered: false, error: "Shortcut conflict" });
    const view = render(<QuickSearchShortcut t={t} compact />);
    await waitFor(() => expect(screen.getByTitle(`${t("qsShortcutUnavailable")} Shortcut conflict`)).toBeInTheDocument());
    expect(screen.getByRole("status")).toHaveTextContent(t("qsShortcutUnavailable"));
    act(() => mocks.statusChanged.mock.calls[0][0]({ ...registered, shortcut: "Cmd+Shift+K" }));
    expect(screen.getByText("Cmd+Shift+K")).toBeInTheDocument();
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    view.unmount();
    await waitFor(() => expect(mocks.disposeStatus).toHaveBeenCalledTimes(1));
  });

  it("keeps manual Open available after status failure and refreshes its status after opening", async () => {
    mocks.status.mockRejectedValueOnce(new Error("Shortcut status unavailable"));
    render(<QuickSearchShortcut t={t} />);
    await screen.findByText(/Shortcut status unavailable/);
    const open = screen.getByRole("button", { name: t("qsOpenWindow") });
    expect(open).toBeEnabled();
    fireEvent.click(open);
    await waitFor(() => expect(mocks.status).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(screen.queryByRole("status")).not.toBeInTheDocument());
    expect(mocks.show).toHaveBeenCalledTimes(1);
  });

  it("shows pending and failed registration, then clears failure on a successful retry", async () => {
    const registration = deferred<QuickSearchStatus>();
    mocks.status.mockResolvedValueOnce({ ...registered, registered: false, error: "Shortcut conflict" });
    mocks.retryRegistration.mockReturnValueOnce(registration.promise);
    render(<QuickSearchShortcut t={t} />);
    await screen.findByText(/Shortcut conflict/);
    const retry = screen.getByRole("button", { name: t("qsRetryShortcut") });
    fireEvent.click(retry);
    expect(retry).toBeDisabled();
    expect(screen.getByRole("button", { name: t("qsOpenWindow") })).toBeDisabled();
    await act(async () => registration.reject(new Error("Shortcut still in use")));
    expect(screen.getByRole("status")).toHaveTextContent("Shortcut still in use");
    expect(retry).toBeEnabled();
    fireEvent.click(retry);
    await waitFor(() => expect(screen.queryByRole("status")).not.toBeInTheDocument());
    expect(mocks.retryRegistration).toHaveBeenCalledTimes(2);
    expect(retry).toBeEnabled();
  });

  it("disposes a late status subscription and ignores its subsequent events", async () => {
    const listener = deferred<() => void>();
    const status = deferred<QuickSearchStatus>();
    mocks.statusChanged.mockReturnValueOnce(listener.promise);
    mocks.status.mockReturnValueOnce(status.promise);
    const view = render(<QuickSearchShortcut t={t} />);
    view.unmount();
    await act(async () => {
      listener.resolve(mocks.disposeStatus);
      status.reject(new Error("Stale status failure"));
      mocks.statusChanged.mock.calls[0][0](registered);
    });
    expect(mocks.disposeStatus).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });
});
