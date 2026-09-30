import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SessionSearchPreview } from "../components/SessionSearchPreview";
import { SessionResumeButton, sessionResumeReason } from "../components/ui/session-resume";
import { dictionaries, type MessageKey } from "../i18n";
import type { AgentSession, SessionMessage, SessionMessagePage, SessionResumeSpec, SessionSearchHit } from "../lib/sessions";

const mocks = vi.hoisted(() => ({ messages: vi.fn(), resumeSpec: vi.fn(), resume: vi.fn(), writeText: vi.fn() }));
vi.mock("../lib/sessions", async importOriginal => ({
  ...await importOriginal<typeof import("../lib/sessions")>(),
  sessionApi: mocks,
}));

const t = (key: MessageKey) => dictionaries.en[key];
const date = "2026-09-30T08:00:00Z";
function session(id = "first", changes: Partial<AgentSession> = {}): AgentSession {
  return {
    id, sourceId: "codex-source", adapter: "codex", externalId: `external-${id}`, projectId: `project-${id}`,
    projectName: `Project ${id}`, matchKind: "exact", cwd: `F:\\projects\\${id}`, title: `Session ${id}`,
    lastUserExcerpt: "An excerpt, not the original message", startedAt: date, updatedAt: date,
    messageCount: 50, archived: false, sourceMissing: false, sourceLocator: `F:\\history\\${id}.jsonl`,
    revision: "revision-1", resumeReason: null, capabilities: { search: true, transcript: true, directResume: true },
    ...changes,
  };
}
const message = (index: number, content = `Message ${index}`): SessionMessage => ({ index, content, role: index % 2 ? "assistant" : "user", timestamp: date });
const page = (...items: SessionMessage[]): SessionMessagePage => ({ items, total: 50 });
const hit = (id = "first", index = 20): SessionSearchHit => ({ session: session(id), snippets: [message(index, "clipped snippet…")] });
const spec = (id = "first", canResume = true): SessionResumeSpec => ({
  agent: "codex", cwd: `F:\\projects\\${id}`, args: ["resume", `external-${id}`], command: `codex resume external-${id}`,
  env: {}, app: { id: "codex-app", name: "Codex App", canResume },
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

beforeEach(() => {
  vi.resetAllMocks();
  mocks.messages.mockResolvedValue(page(message(20)));
  mocks.resumeSpec.mockResolvedValue(spec());
  mocks.resume.mockResolvedValue(undefined);
  mocks.writeText.mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: mocks.writeText } });
});

describe("SessionSearchPreview", () => {
  it("loads bounded surrounding context, escapes literal query terms and copies the complete original message", async () => {
    const original = '  **a+b** [x] foo.bar\r\n<img src=x onerror="alert(1)"> &amp;\n' + "unaltered text ".repeat(220);
    mocks.messages.mockResolvedValue(page(message(18, "Before"), message(19), message(20, original), message(21), message(22, "After")));
    const { container } = render(<SessionSearchPreview hit={hit()} query="a+b [x] foo.bar" t={t} />);
    expect(screen.getByRole("button", { name: t("qsCopyMessage") })).toBeDisabled();
    await screen.findByRole("button", { name: t("qsExpand") });
    expect(screen.queryByText("Before")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: new RegExp(t("qsContext")) }));
    fireEvent.click(screen.getByRole("button", { name: t("qsExpand") }));
    expect(screen.getByText("Before")).toBeInTheDocument();
    expect(mocks.messages).toHaveBeenCalledWith("first", 18, 5);
    expect(screen.getByText("After")).toBeInTheDocument();
    expect(screen.getByText("Codex CLI")).toBeInTheDocument();
    expect(screen.getByText("Project first")).toBeInTheDocument();
    expect(screen.getByText("F:\\projects\\first")).toBeInTheDocument();
    expect(container.querySelector(".search-preview-message-match pre")?.textContent).toBe(original);
    expect(Array.from(container.querySelectorAll("mark"), node => node.textContent)).toEqual(["a+b", "[x]", "foo.bar"]);
    expect(container.querySelector("img, script, pre strong, pre a")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: t("qsCopyMessage") }));
    await screen.findByText(t("ahCopied"));
    expect(mocks.writeText).toHaveBeenCalledWith(original);
  });

  it.each(["resolve", "reject"] as const)("ignores a stale %s when changing sessions without a parent key", async outcome => {
    const old = deferred<SessionMessagePage>();
    const next = deferred<SessionMessagePage>();
    mocks.messages.mockReturnValueOnce(old.promise).mockReturnValueOnce(next.promise);
    const view = render(<SessionSearchPreview hit={hit()} query="" t={t} />);
    view.rerender(<SessionSearchPreview hit={hit("second", 8)} query="" t={t} />);
    await act(async () => {
      if (outcome === "resolve") old.resolve(page(message(20, "Stale transcript")));
      else old.reject(new Error("Stale read failure"));
    });
    expect(screen.queryByText("Stale transcript")).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByText(t("loading"))).toBeInTheDocument();
    await act(async () => next.resolve(page(message(8, "Current transcript"))));
    expect(screen.getByText("Current transcript")).toBeInTheDocument();
    expect(mocks.messages).toHaveBeenLastCalledWith("second", 6, 5);
  });

  it("rejects an older message selection in the same session and keeps keyboard focus outside the preview", async () => {
    const old = deferred<SessionMessagePage>();
    mocks.messages.mockReturnValueOnce(old.promise).mockResolvedValueOnce(page(message(30, "New match")));
    const renderSelection = (index: number) => <><input aria-label="Search" /><SessionSearchPreview hit={hit("first", index)} query="" t={t} /></>;
    const view = render(renderSelection(20));
    screen.getByRole("textbox", { name: "Search" }).focus();
    view.rerender(renderSelection(30));
    await screen.findByText("New match");
    await act(async () => old.resolve(page(message(20, "Old match"))));
    expect(screen.queryByText("Old match")).not.toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Session first" })).toHaveAttribute("tabindex", "0");
    expect(screen.getByRole("textbox", { name: "Search" })).toHaveFocus();
  });

  it("shows read failure with retry and never copies a clipped snippet or a different message", async () => {
    mocks.messages.mockRejectedValueOnce(new Error("Read denied"))
      .mockResolvedValueOnce(page(message(19, "Wrong message")))
      .mockResolvedValueOnce(page(message(20, "Exact match")));
    render(<SessionSearchPreview hit={hit()} query="" t={t} />);
    expect(await screen.findByRole("alert")).toHaveTextContent("Read denied");
    fireEvent.click(screen.getByRole("button", { name: t("qsCopyMessage") }));
    expect(mocks.writeText).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: t("retry") }));
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent(t("ahReadFailed")));
    expect(screen.queryByText("Wrong message")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: t("qsCopyMessage") })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: t("retry") }));
    await screen.findByText("Exact match");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: t("qsCopyMessage") })).toBeEnabled();
  });

  it("shows clipboard pending and failure, and lets the same exact content be retried", async () => {
    const copying = deferred<void>();
    mocks.writeText.mockReturnValueOnce(copying.promise);
    render(<SessionSearchPreview hit={hit()} query="" t={t} />);
    await screen.findByText("Message 20");
    fireEvent.click(screen.getByRole("button", { name: t("qsCopyMessage") }));
    expect(screen.getByRole("button", { name: t("qsCopyMessage") })).toBeDisabled();
    expect(screen.getByText(t("ahPending"))).toBeInTheDocument();
    await act(async () => copying.reject(new Error("Clipboard denied")));
    expect(screen.getByRole("alert")).toHaveTextContent("Clipboard denied");
    fireEvent.click(screen.getByRole("button", { name: t("qsCopyMessage") }));
    await screen.findByText(t("ahCopied"));
    expect(mocks.writeText.mock.calls).toEqual([["Message 20"], ["Message 20"]]);
  });

  it("forwards exact project/session targets with pending, failure and success feedback", async () => {
    const navigation = deferred<void>();
    const onProject = vi.fn().mockReturnValueOnce(navigation.promise).mockResolvedValue(undefined);
    const onSession = vi.fn();
    const selected = hit();
    render(<SessionSearchPreview hit={selected} query="" t={t} onProject={onProject} onSession={onSession} />);
    await screen.findByText("Message 20");
    fireEvent.click(screen.getByRole("button", { name: t("moduleOpenProject") }));
    expect(onProject).toHaveBeenCalledWith("project-first");
    expect(screen.getByRole("button", { name: t("qsOpenSession") })).toBeDisabled();
    await act(async () => navigation.reject(new Error("Project unavailable")));
    expect(screen.getByRole("alert")).toHaveTextContent("Project unavailable");
    fireEvent.click(screen.getByRole("button", { name: t("qsOpenSession") }));
    await screen.findByText(t("operationCompleted"));
    expect(onSession).toHaveBeenCalledWith(selected.session, 20);
  });

  it("ignores clipboard completion after changing selection", async () => {
    const copying = deferred<void>();
    mocks.writeText.mockReturnValueOnce(copying.promise);
    const view = render(<SessionSearchPreview hit={hit()} query="" t={t} />);
    await screen.findByText("Message 20");
    fireEvent.click(screen.getByRole("button", { name: t("qsCopyMessage") }));
    mocks.messages.mockResolvedValueOnce(page(message(20, "Second session")));
    view.rerender(<SessionSearchPreview hit={hit("second")} query="" t={t} />);
    await screen.findByText("Second session");
    await act(async () => copying.reject(new Error("Previous copy denied")));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: t("qsCopyMessage") })).toBeEnabled();
  });

  it("reads from zero for empty title-only hits and leaves message copy disabled", async () => {
    mocks.messages.mockResolvedValue(page());
    const onSession = vi.fn();
    render(<SessionSearchPreview hit={{ session: session("unlinked", { projectId: null, projectName: null, messageCount: 0 }), snippets: [] }}
      query="" t={t} onProject={vi.fn()} onSession={onSession} />);
    await screen.findByText(`0 ${t("ahMessages")}`, { selector: "p" });
    expect(mocks.messages).toHaveBeenCalledWith("unlinked", 0, 5);
    expect(screen.getByRole("button", { name: t("qsCopyMessage") })).toBeDisabled();
    expect(screen.getByRole("button", { name: t("moduleOpenProject") })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: t("qsOpenSession") }));
    expect(onSession).toHaveBeenCalledWith(expect.objectContaining({ id: "unlinked" }), undefined);
    await screen.findByText(t("operationCompleted"));
  });

  it("copies the last loaded message verbatim when a blank query has no snippet", async () => {
    const original = "  **Latest shown**\r\n[x](file) &amp;";
    mocks.messages.mockResolvedValue(page(message(45, "First shown"), message(49, original)));
    render(<SessionSearchPreview hit={{ session: session(), snippets: [] }} query="" t={t} />);
    await screen.findByText(/Latest shown/);
    expect(screen.queryByText("First shown")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: t("qsCopyMessage") }));
    await screen.findByText(t("ahCopied"));
    expect(mocks.messages).toHaveBeenCalledWith("first", 45, 5);
    expect(mocks.writeText).toHaveBeenCalledWith(original);
  });

  it("starts at the matching message with surrounding context collapsed and leaves focus unchanged", async () => {
    const reading = deferred<SessionMessagePage>();
    mocks.messages.mockReturnValueOnce(reading.promise);
    const bounds = HTMLElement.prototype.getBoundingClientRect;
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
      if (this.classList.contains("search-preview-transcript")) return { top: 100, height: 400 } as DOMRect;
      if (this.classList.contains("search-preview-message-match")) return { top: 1800, height: 200 } as DOMRect;
      return bounds.call(this);
    });
    const { container } = render(<><input aria-label="Search" /><SessionSearchPreview hit={hit()} query="" t={t} /></>);
    const pane = container.querySelector<HTMLElement>(".search-preview-transcript")!;
    Object.defineProperty(pane, "clientHeight", { configurable: true, value: 400 });
    screen.getByRole("textbox", { name: "Search" }).focus();
    await act(async () => reading.resolve(page(message(18, "Long context ".repeat(500)), message(20, "Matched"))));
    expect(pane.scrollTop).toBe(0);
    expect(screen.getByText("Matched")).toBeInTheDocument();
    expect(screen.queryByText(/Long context/)).not.toBeInTheDocument();
    expect(container.scrollTop).toBe(0);
    expect(screen.getByRole("textbox", { name: "Search" })).toHaveFocus();
  });

  it("retains cached messages while explaining missing sources and resume reasons", async () => {
    const selected = hit();
    selected.session.sourceMissing = true;
    selected.session.resumeReason = "session_cwd_missing";
    render(<SessionSearchPreview hit={selected} query="" t={t} />);
    await screen.findByText("Message 20");
    expect(screen.getByRole("status")).toHaveTextContent(t("ahSourceMissing"));
    expect(screen.getByRole("status")).toHaveTextContent(t("ahCwdMissing"));
    expect(screen.getByRole("button", { name: t("qsContinueIn").replace("{agent}", "Codex CLI") })).toBeDisabled();
    expect(screen.getByRole("button", { name: t("qsCopyMessage") })).toBeEnabled();
    expect(mocks.resumeSpec).not.toHaveBeenCalled();
  });
  it("skips an internal latest notification but still copies the preceding real message exactly", async () => {
    const original = "  Fixed the login redirect.\nRun the focused test.";
    mocks.messages.mockResolvedValue(page(message(48, original), message(49, '<subagent_notification>{"status":"done"}</subagent_notification>')));
    render(<SessionSearchPreview hit={{ session: session(), snippets: [] }} query="" t={t} />);
    await screen.findByText(/Fixed the login redirect/);
    expect(screen.queryByText(/subagent_notification/)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: t("qsCopyMessage") }));
    await screen.findByText(t("ahCopied"));
    expect(mocks.writeText).toHaveBeenCalledWith(original);
  });
  it("shows a late literal match immediately and expands to the full original on request", async () => {
    const original = 'First paragraph. '.repeat(250) + 'NEEDLE: the actual fix\n' + 'Additional context. '.repeat(70);
    mocks.messages.mockResolvedValue(page(message(20, original)));
    const { container } = render(<SessionSearchPreview hit={hit()} query="NEEDLE" t={t} />);
    await screen.findByText("NEEDLE", { selector: "mark" });
    expect(container.querySelector("pre")?.textContent?.length).toBeLessThan(570);
    fireEvent.click(screen.getByRole("button", { name: t("qsExpand") }));
    expect(container.querySelector("pre")?.textContent).toBe(original);
    expect(screen.getByRole("button", { name: t("qsCollapse") })).toHaveAttribute("aria-expanded", "true");
  });
});

describe("SessionResumeButton", () => {
  async function openResume() {
    fireEvent.click(screen.getByRole("button", { name: t("ahResume") }));
    return within(await screen.findByRole("dialog", { name: t("ahResume") }));
  }

  it("keeps launch disabled after failed validation and offers explicit revalidation", async () => {
    const onError = vi.fn();
    mocks.resumeSpec.mockRejectedValueOnce(new Error("session_source_missing"));
    render(<SessionResumeButton session={session()} t={t} onError={onError} />);
    const dialog = await openResume();
    expect(await dialog.findByRole("alert")).toHaveTextContent(t("ahSourceMissing"));
    const launch = dialog.getByRole("button", { name: t("ahLaunchNow") });
    expect(launch).toBeDisabled();
    fireEvent.click(launch);
    expect(mocks.resume).not.toHaveBeenCalled();
    expect(onError).toHaveBeenCalledWith(t("ahSourceMissing"));
    fireEvent.click(dialog.getByRole("button", { name: t("retry") }));
    await waitFor(() => expect(launch).toBeEnabled());
    expect(mocks.resumeSpec).toHaveBeenCalledTimes(2);
    expect(mocks.resume).not.toHaveBeenCalled();
  });

  it.each(["cli", "app"] as const)("previews and copies the validated %s command before dispatch, with pending and success feedback", async target => {
    const dispatch = deferred<void>();
    mocks.resume.mockReturnValueOnce(dispatch.promise);
    render(<SessionResumeButton session={session()} t={t} />);
    const dialog = await openResume();
    await dialog.findByText("codex resume external-first");
    if (target === "app") fireEvent.click(dialog.getByRole("radio", { name: "Codex App / App" }));
    const command = target === "app" ? "codex://threads/external-first" : "codex resume external-first";
    expect(dialog.getByText(command)).toBeInTheDocument();
    expect(dialog.getByText("F:\\projects\\first")).toBeInTheDocument();
    fireEvent.click(dialog.getByRole("button", { name: t("ahCopy") }));
    await dialog.findByText(t("ahCopied"));
    expect(mocks.writeText).toHaveBeenCalledWith(command);
    expect(mocks.resume).not.toHaveBeenCalled();
    fireEvent.click(dialog.getByRole("button", { name: t("ahLaunchNow") }));
    expect(mocks.resume).toHaveBeenCalledWith("first", target);
    expect(dialog.getByRole("button", { name: t("ahLaunchNow") })).toBeDisabled();
    expect(dialog.getByText(t("ahPending"))).toBeInTheDocument();
    await act(async () => dispatch.resolve());
    expect(dialog.getByText(t("ahStarted"))).toBeInTheDocument();
  });

  it("explains an unsupported App and cannot select it for resume", async () => {
    mocks.resumeSpec.mockResolvedValue(spec("first", false));
    render(<SessionResumeButton session={session()} t={t} />);
    const dialog = await openResume();
    expect(await dialog.findByText(t("ahAppUnsupported"))).toBeInTheDocument();
    const app = dialog.getByRole("radio", { name: "Codex App / App" });
    expect(app).toBeDisabled();
    act(() => (app as HTMLInputElement).click());
    expect(app).not.toBeChecked();
    fireEvent.click(dialog.getByRole("button", { name: t("ahLaunchNow") }));
    await dialog.findByText(t("ahStarted"));
    expect(mocks.resume).toHaveBeenCalledWith("first", "cli");
  });

  it.each(["resolve", "reject"] as const)("ignores stale spec %s after switching session, and requires the new spec before dispatch", async outcome => {
    const old = deferred<SessionResumeSpec>();
    const next = deferred<SessionResumeSpec>();
    const onError = vi.fn();
    mocks.resumeSpec.mockReturnValueOnce(old.promise).mockReturnValueOnce(next.promise);
    const view = render(<SessionResumeButton session={session()} t={t} onError={onError} />);
    await openResume();
    view.rerender(<SessionResumeButton session={session("second")} t={t} onError={onError} />);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    const dialog = await openResume();
    await act(async () => {
      if (outcome === "resolve") old.resolve(spec());
      else old.reject(new Error("session_source_missing"));
    });
    expect(dialog.queryByText("codex resume external-first")).not.toBeInTheDocument();
    expect(dialog.getByRole("button", { name: t("ahLaunchNow") })).toBeDisabled();
    fireEvent.click(dialog.getByRole("button", { name: t("ahLaunchNow") }));
    expect(mocks.resume).not.toHaveBeenCalled();
    expect(onError).not.toHaveBeenCalled();
    await act(async () => next.resolve(spec("second")));
    fireEvent.click(dialog.getByRole("button", { name: t("ahLaunchNow") }));
    await dialog.findByText(t("ahStarted"));
    expect(mocks.resume.mock.calls).toEqual([["second", "cli"]]);
  });

  it("discards old dispatch feedback without clearing the new session's pending validation", async () => {
    const dispatch = deferred<void>();
    const nextSpec = deferred<SessionResumeSpec>();
    const onError = vi.fn();
    mocks.resume.mockReturnValueOnce(dispatch.promise);
    mocks.resumeSpec.mockResolvedValueOnce(spec()).mockReturnValueOnce(nextSpec.promise);
    const view = render(<SessionResumeButton session={session()} t={t} onError={onError} />);
    const first = await openResume();
    await first.findByText("codex resume external-first");
    fireEvent.click(first.getByRole("button", { name: t("ahLaunchNow") }));
    view.rerender(<SessionResumeButton session={session("second")} t={t} onError={onError} />);
    const second = await openResume();
    await act(async () => dispatch.reject(new Error("session_cli_probe_timeout")));
    expect(onError).not.toHaveBeenCalled();
    expect(second.queryByRole("alert")).not.toBeInTheDocument();
    expect(second.getByRole("button", { name: t("ahLaunchNow") })).toBeDisabled();
    await act(async () => nextSpec.resolve(spec("second")));
    expect(second.getByRole("button", { name: t("ahLaunchNow") })).toBeEnabled();
    expect(mocks.resume.mock.calls).toEqual([["first", "cli"]]);
  });

  it("invalidates a failed resume and never falls back to another target or launches again before revalidation", async () => {
    const onError = vi.fn();
    mocks.resume.mockRejectedValueOnce("session_cli_unavailable");
    render(<SessionResumeButton session={session()} t={t} onError={onError} />);
    const dialog = await openResume();
    await dialog.findByText("codex resume external-first");
    fireEvent.click(dialog.getByRole("button", { name: t("ahLaunchNow") }));
    expect(await dialog.findByRole("alert")).toHaveTextContent(t("ahCliUnavailable"));
    expect(onError).toHaveBeenCalledWith(t("ahCliUnavailable"));
    expect(dialog.getByRole("button", { name: t("ahLaunchNow") })).toBeDisabled();
    fireEvent.click(dialog.getByRole("button", { name: t("ahLaunchNow") }));
    expect(mocks.resume.mock.calls).toEqual([["first", "cli"]]);
    expect(dialog.queryByText(t("ahStarted"))).not.toBeInTheDocument();
  });

  it("invalidates a validated launch panel when the same session's source becomes unavailable", async () => {
    const view = render(<SessionResumeButton session={session()} t={t} />);
    const dialog = await openResume();
    await dialog.findByText("codex resume external-first");
    view.rerender(<SessionResumeButton session={session("first", { sourceMissing: true })} t={t} />);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: t("ahResume") })).toBeDisabled();
    expect(screen.getByText(t("ahSourceMissing"))).toBeInTheDocument();
    expect(mocks.resume).not.toHaveBeenCalled();
  });

  it("exports all existing reason translations and preserves unknown reasons as plain text", () => {
    const reasons: Record<string, MessageKey> = {
      session_cli_unavailable: "ahCliUnavailable", session_cli_probe_timeout: "ahCliProbeTimeout",
      session_source_layout_unknown: "ahSourceLayout", session_cwd_missing: "ahCwdMissing",
      session_agent_missing: "ahAgentMissing", session_source_missing: "ahSourceMissing", invalid_session_id: "ahInvalidId",
      source_disabled: "qsSourceDisabled", session_not_found: "qsSessionMissing",
    };
    for (const [reason, key] of Object.entries(reasons)) expect(sessionResumeReason(reason, t)).toBe(t(key));
    expect(sessionResumeReason(null, t)).toBe("");
    expect(sessionResumeReason("Unknown reason <text>", t)).toBe("Unknown reason <text>");
    expect(sessionResumeReason("toString", t)).toBe("toString");
  });
});
