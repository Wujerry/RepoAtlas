import { ArrowSquareOut, CaretDown, ChatCircleText, Copy, FolderOpen } from "@phosphor-icons/react";
import { useEffect, useId, useRef, useState } from "react";
import type { MessageKey } from "../i18n";
import { sessionDisplayText, sessionDisplayTitle } from "../lib/session-presentation";
import { sessionAgentName, sessionApi, sessionTime, type AgentSession, type SessionMessage, type SessionMessagePage, type SessionSearchHit } from "../lib/sessions";
import { Button } from "./ui/button";
import { SearchHighlight, searchExcerpt } from "./ui/search-highlight";
import { SessionAgentLabel } from "./ui/session-agent";
import { SessionResumeButton, sessionResumeReason } from "./ui/session-resume";

export interface SessionSearchPreviewProps {
  hit: SessionSearchHit;
  query: string;
  displayTitle?: string;
  t: (key: MessageKey) => string;
  onProject?: (id: string) => void | Promise<void>;
  onSession?: (session: AgentSession, messageIndex?: number) => void | Promise<void>;
}

type LoadState = { selection: string; status: "loading" | "ready" | "error"; page?: SessionMessagePage; error?: string };
type Action = "copy" | "project" | "session";
const CONTEXT_RADIUS = 2;
const PAGE_SIZE = CONTEXT_RADIUS * 2 + 1;

/** Parent may key by session.id; request sequencing also protects unkeyed selection changes. */
export function SessionSearchPreview({ hit, query, t, onProject, onSession, displayTitle }: SessionSearchPreviewProps) {
  const { session } = hit;
  const titleId = useId();
  const snippetIndex = hit.snippets[0]?.index;
  const messageIndex = snippetIndex !== undefined && Number.isSafeInteger(snippetIndex) && snippetIndex >= 0 ? snippetIndex : undefined;
  const selection = JSON.stringify([session.id, session.revision, messageIndex, session.capabilities.transcript]);
  const [load, setLoad] = useState<LoadState>({ selection, status: "loading" });
  const [retry, setRetry] = useState(0);
  const [action, setAction] = useState<Action | null>(null);
  const [actionError, setActionError] = useState("");
  const [notice, setNotice] = useState("");
  const transcript = useRef<HTMLDivElement>(null);
  const [expanded, setExpanded] = useState(false);
  const [contextOpen, setContextOpen] = useState(false);
  const sequence = useRef(0);
  const actionGuard = useRef(false);

  useEffect(() => {
    const request = ++sequence.current;
    const current = () => sequence.current === request;
    setAction(null); actionGuard.current = false; setActionError(""); setNotice("");
    setExpanded(false); setContextOpen(false);
    setLoad({ selection, status: "loading" });
    if (!session.capabilities.transcript) {
      setLoad({ selection, status: "error" });
    } else {
      void sessionApi.messages(session.id, messageIndex === undefined ? Math.max(0, session.messageCount - PAGE_SIZE) : Math.max(0, messageIndex - CONTEXT_RADIUS), PAGE_SIZE).then(page => {
        if (!current()) return;
        // Search snippets are clipped; only the message page can supply the exact copyable text.
        if (messageIndex !== undefined && !page.items.some(message => message.index === messageIndex)) {
          setLoad({ selection, status: "error" });
        } else {
          setLoad({ selection, status: "ready", page });
        }
      }).catch(failure => {
        if (current()) setLoad({ selection, status: "error", error: failure instanceof Error ? failure.message : String(failure) });
      });
    }
    return () => { sequence.current += 1; };
  }, [selection, session.id, session.messageCount, session.capabilities.transcript, messageIndex, retry]);

  const currentLoad = load.selection === selection ? load : undefined;
  const loading = !currentLoad || currentLoad.status === "loading";
  const page = currentLoad?.status === "ready" ? currentLoad.page : undefined;
  const visibleMessages = page?.items.filter(message => sessionDisplayText(message.content).trim()) ?? [];
  const copyMessage = messageIndex === undefined ? visibleMessages[visibleMessages.length - 1]
    : page?.items.find(message => message.index === messageIndex);
  const contextMessages = visibleMessages.filter(message => message.index !== copyMessage?.index);
  const title = displayTitle || sessionDisplayTitle(session.title, t("ahUntitled"));
  const messageText = copyMessage ? (query.trim() ? copyMessage.content : sessionDisplayText(copyMessage.content)) : "";
  const condensedMessage = messageText.length > 360 ? searchExcerpt(messageText, query, 360, 32) : messageText;

  useEffect(() => {
    if (transcript.current) transcript.current.scrollTop = 0;
  }, [page, messageIndex]);

  async function runAction(kind: Action, operation: () => void | Promise<void>) {
    if (actionGuard.current) return;
    const request = sequence.current;
    actionGuard.current = true; setAction(kind); setActionError(""); setNotice("");
    try {
      await operation();
      if (sequence.current === request) setNotice(t(kind === "copy" ? "ahCopied" : "operationCompleted"));
    } catch (failure) {
      if (sequence.current === request) setActionError(failure instanceof Error ? failure.message : String(failure));
    } finally {
      if (sequence.current === request) { actionGuard.current = false; setAction(null); }
    }
  }

  const timestamp = (value: string) => <time dateTime={value} title={new Date(value).toLocaleString()}>
    {sessionTime(value, t("ahUser") === "用户")}
  </time>;
  const messageMeta = (message: SessionMessage) => <header className="search-preview-message-meta">
    <strong>{t(message.role === "user" ? "ahUser" : "ahAssistant")}</strong>
    <span>#{message.index + 1}</span>{message.timestamp && timestamp(message.timestamp)}
  </header>;

  return <section className="search-preview" tabIndex={0} aria-labelledby={titleId}>
    <header className="search-preview-header">
      <div className="search-preview-eyebrow"><span>{t("qsPreview")}</span><SessionAgentLabel adapter={session.adapter} /></div>
      <h2 id={titleId} className="search-preview-title" title={title}>{title}</h2>
      <div className="search-preview-meta"><strong className="search-preview-project">{session.projectName ?? t("ahUnlinked")}</strong><span>{timestamp(session.updatedAt)} · {page?.total ?? session.messageCount} {t("ahMessages")}</span></div>
      <div className="search-preview-location"><FolderOpen size={14} aria-hidden /><code className="search-preview-path" title={session.cwd}>{session.cwd || t("ahCwdMissing")}</code>
        {onProject && <Button type="button" variant="quiet" size="icon" disabled={!session.projectId || action !== null} loading={action === "project"}
          aria-label={t("moduleOpenProject")} title={session.projectId ? t("moduleOpenProject") : t("ahUnlinked")}
          onClick={() => { if (session.projectId) void runAction("project", () => onProject(session.projectId!)); }}><ArrowSquareOut aria-hidden /></Button>}
      </div>
    </header>
    <div ref={transcript} className="search-preview-transcript" aria-busy={loading}>
      <div className="search-preview-section-heading"><h3>{t(messageIndex === undefined ? "qsLatest" : "qsMatch")}</h3>
        <Button type="button" variant="quiet" size="icon" aria-label={t("qsCopyMessage")} title={t("qsCopyMessage")}
          disabled={!copyMessage || action !== null} loading={action === "copy"}
          onClick={() => { if (copyMessage) void runAction("copy", () => navigator.clipboard.writeText(copyMessage.content)); }}><Copy aria-hidden /></Button>
      </div>
      {loading && <p className="search-preview-loading" role="status">{t("loading")}</p>}
      {currentLoad?.status === "error" && <div className="search-preview-error">
        <p role="alert">{t("ahReadFailed")}{currentLoad.error && <>: {sessionResumeReason(currentLoad.error, t)}</>}</p>
        {session.capabilities.transcript && <Button type="button" variant="quiet" onClick={() => setRetry(value => value + 1)}>{t("retry")}</Button>}
      </div>}
      {page?.items.length === 0 && <p className="search-preview-empty" role="status">0 {t("ahMessages")}</p>}
      {page && page.items.length > 0 && !copyMessage && <p className="search-preview-empty">{t("qsNoPreview")}</p>}
      {copyMessage && <article className={`search-preview-message search-preview-message-featured${messageIndex !== undefined ? " search-preview-message-match" : ""}`} aria-current="true">
        {messageMeta(copyMessage)}
        <pre className="search-preview-message-content"><SearchHighlight text={expanded ? messageText : condensedMessage} query={query} /></pre>
        {condensedMessage !== messageText && <Button variant="quiet" className="search-preview-expand" aria-expanded={expanded} onClick={() => setExpanded(value => !value)}>{t(expanded ? "qsCollapse" : "qsExpand")}</Button>}
      </article>}
      {contextMessages.length > 0 && <div className="search-preview-context">
        <Button variant="quiet" aria-expanded={contextOpen} onClick={() => setContextOpen(value => !value)}><CaretDown aria-hidden />{t("qsContext")}<span>{contextMessages.length}</span></Button>
        {contextOpen && contextMessages.map(message => <article key={message.index} className="search-preview-message">
          {messageMeta(message)}<pre className="search-preview-message-content"><SearchHighlight text={sessionDisplayText(message.content)} query={query} /></pre>
        </article>)}
      </div>}
    </div>
    <footer className="search-preview-actions">
      {onSession && <Button type="button" variant="quiet" disabled={action !== null} loading={action === "session"}
        onClick={() => void runAction("session", () => onSession(session, messageIndex))}>
        <ChatCircleText aria-hidden="true" />{t("qsOpenSession")}
      </Button>}
      <SessionResumeButton session={session} t={t} label={t("qsContinueIn").replace("{agent}", sessionAgentName(session.adapter))} />
    </footer>
    {action && <p className="search-preview-status" role="status">{t("ahPending")}</p>}
    {notice && <p className="search-preview-status" role="status">{notice}</p>}
    {actionError && <p className="search-preview-error" role="alert">{actionError}</p>}
  </section>;
}
