import { Dialog } from "@base-ui/react/dialog";
import { isTauri } from "@tauri-apps/api/core";
import { ArrowDown, ArrowElbowDownLeft, ArrowUp, FolderSimple, Lightning, MagnifyingGlass, SlidersHorizontal, X } from "@phosphor-icons/react";
import { MotionConfig } from "framer-motion";
import { useEffect, useId, useMemo, useRef, useState, type ReactNode } from "react";
import type { MessageKey } from "../i18n";
import { api } from "../lib/api";
import { onSessionSourcesChanged, openSessionHistory, sessionApi, sessionTime, type AgentSession, type SessionSearchHit, type SessionSource } from "../lib/sessions";
import { sessionDisplayTitle, sessionExcerpt } from "../lib/session-presentation";
import { SessionSnippet } from "./ui/session-content";
import { AgentBrandIcon } from "../lib/brand-icons";
import type { ProjectSummary, SearchHit } from "../types";
import { SessionSearchPreview } from "./SessionSearchPreview";
import { Button } from "./ui/button";
import { sessionAgentName } from "../lib/sessions";
import { SearchHighlight } from "./ui/search-highlight";
import { TokenBadge } from "./ui/token-usage";

interface Action { id: string; title: string; hint?: string; run: () => void | Promise<void> }
type PaletteItem = { key: string; title: string; hint?: string } & (
  { kind: "action" | "project"; run: () => void | Promise<void> } |
  { kind: "session"; hit: SessionSearchHit }
);

export function CommandPalette({ open, onClose, t, projects, actions, onProject, onSession, onSources, standalone = false, shortcutHint }: {
  open: boolean;
  onClose: () => void;
  t: (key: MessageKey) => string;
  projects: ProjectSummary[];
  actions: Action[];
  onProject?: (id: string) => void | Promise<void>;
  onSession?: (session: AgentSession, messageIndex?: number) => void | Promise<void>;
  onSources?: () => void | Promise<void>;
  standalone?: boolean;
  shortcutHint?: ReactNode;
}) {
  const [query, setQuery] = useState("");
  const [projectHits, setProjectHits] = useState<SearchHit[]>([]);
  const [sessionHits, setSessionHits] = useState<SessionSearchHit[]>([]);
  const [sources, setSources] = useState<SessionSource[] | null>(null);
  const [searching, setSearching] = useState(false);
  const [errors, setErrors] = useState<string[]>([]);
  const [sourceError, setSourceError] = useState("");
  const [actionError, setActionError] = useState("");
  const [actionBusy, setActionBusy] = useState(false);
  const [activeIndex, setActiveIndex] = useState(0);
  const [revision, setRevision] = useState(0);
  const [scope, setScope] = useState("all");
  const [resolvedTitles, setResolvedTitles] = useState<Record<string, string>>({});
  const input = useRef<HTMLInputElement>(null);
  const preview = useRef<HTMLDivElement>(null);
  const sequence = useRef(0);
  const actionGuard = useRef(false);
  const listId = useId();
  const q = query.trim();
  const chinese = t("ahUser") === "用户";

  useEffect(() => {
    setQuery(""); setScope("all"); setResolvedTitles({}); setProjectHits([]); setSessionHits([]); setSources(null); setErrors([]); setActionError(""); setActiveIndex(0);
    if (!open) return;
    const frame = requestAnimationFrame(() => input.current?.focus());
    const changed = () => { sequence.current++; setResolvedTitles({}); setSessionHits([]); setSources(null); setRevision(value => value + 1); };
    window.addEventListener("repoatlas:sessions-updated", changed);
    const listener = isTauri() ? onSessionSourcesChanged(changed) : undefined;
    void listener?.catch(error => setSourceError(String(error)));
    return () => { cancelAnimationFrame(frame); window.removeEventListener("repoatlas:sessions-updated", changed); void listener?.then(dispose => dispose(), () => undefined); };
  }, [open]);

  useEffect(() => {
    if (!open) return;
    let current = true;
    setSourceError("");
    void sessionApi.sources().then(items => { if (current) setSources(items); }).catch(error => { if (current) { setSources(null); setSourceError(String(error)); } });
    return () => { current = false; };
  }, [open, revision]);

  useEffect(() => {
    if (!open) return;
    const ticket = ++sequence.current;
    setActiveIndex(0); setSearching(true); setErrors([]); setProjectHits([]); setSessionHits([]);
    const timer = setTimeout(() => {
      // Each source can succeed independently; a damaged history index never hides Projects.
      void Promise.allSettled([
        q ? api.searchProjects(q) : Promise.resolve([] as SearchHit[]),
        sessionApi.search({ query: q || undefined, limit: 12, archived: Boolean(q) }),
      ]).then(([projectResult, sessionResult]) => {
        if (sequence.current !== ticket) return;
        const failures: string[] = [];
        if (projectResult.status === "fulfilled") setProjectHits(projectResult.value);
        else failures.push(`${t("projects")}: ${String(projectResult.reason)}`);
        if (sessionResult.status === "fulfilled") setSessionHits(sessionResult.value.items);
        else failures.push(`${t("ahReadFailed")}: ${String(sessionResult.reason)}`);
        setErrors(failures); setSearching(false);
      });
    }, q ? 160 : 0);
    return () => { clearTimeout(timer); sequence.current++; };
  }, [open, q, revision, t]);

  useEffect(() => {
    if (!open) return;
    let current = true;
    // Older cached titles can contain client envelopes. Recover a real user line from
    // the existing authorized index without rescanning or inventing a summary.
    const untitled = sessionHits.filter(hit => !sessionDisplayTitle(hit.session.title, ""));
    for (const { session } of untitled) {
      void sessionApi.messages(session.id, 0, 20).then(page => {
        const title = page.items.filter(message => message.role === "user")
          .map(message => sessionDisplayTitle(message.content, "")).find(Boolean);
        if (current && title) setResolvedTitles(values => ({ ...values, [session.id]: title }));
      }).catch(() => { /* The preview retains its own visible read/retry state. */ });
    }
    return () => { current = false; };
  }, [open, sessionHits]);

  const groups = useMemo(() => {
    const jumps = new Map(actions.map(action => [action.id, action]));
    const projectItems: PaletteItem[] = (q ? projectHits.map(hit => hit.project) : projects).slice(0, 8).flatMap(project => {
      const run = onProject ? () => onProject(project.id) : jumps.get(`jump:${project.id}`)?.run;
      return run ? [{ key: `project:${project.id}`, title: project.displayName, hint: project.canonicalPath, kind: "project" as const, run }] : [];
    });
    const sessionItems: PaletteItem[] = sessionHits.map(hit => ({ key: `session:${hit.session.id}`,
      title: resolvedTitles[hit.session.id] || sessionDisplayTitle(hit.session.title, "") || sessionExcerpt(hit.session.lastUserExcerpt) || t("ahUntitled"), kind: "session", hit }));
    const operations: PaletteItem[] = actions.filter(action => !action.id.startsWith("jump:") && (!q || action.title.toLowerCase().includes(q.toLowerCase())))
      .map(action => ({ key: `action:${action.id}`, title: action.title, hint: action.hint, kind: "action", run: action.run }));
    const sessions = { key: "sessions", label: t(q ? "qsSessions" : "qsRecentSessions"), items: sessionItems };
    const projectGroup = { key: "projects", label: t("projects"), items: projectItems };
    // A named Project must remain one Enter away instead of falling below many transcripts.
    return [...(q ? [projectGroup, sessions] : [sessions, projectGroup]),
      { key: "actions", label: t("commands"), items: operations },
    ].filter(group => group.items.length && (scope === "all" || scope === group.key));
  }, [actions, onProject, projectHits, projects, q, sessionHits, t, scope, resolvedTitles]);
  const items = useMemo(() => groups.flatMap(group => group.items), [groups]);
  const activeItem = items[Math.min(activeIndex, Math.max(0, items.length - 1))];
  const activeId = activeItem ? `${listId}-${activeItem.key}` : undefined;

  useEffect(() => { setActiveIndex(0); }, [q, scope]);
  useEffect(() => { if (activeId) document.getElementById(activeId)?.scrollIntoView({ block: "nearest" }); }, [activeId]);

  async function perform(run: () => void | Promise<void>, close = true) {
    if (actionGuard.current) return;
    actionGuard.current = true; setActionBusy(true); setActionError("");
    try { await run(); if (close) onClose(); }
    catch (error) { setActionError(String(error)); }
    finally { actionGuard.current = false; setActionBusy(false); }
  }
  function activate(item: PaletteItem) {
    if (item.kind === "session") {
      setActiveIndex(items.indexOf(item));
      requestAnimationFrame(() => preview.current?.focus());
    } else void perform(item.run);
  }
  function keyDown(event: React.KeyboardEvent<HTMLInputElement>) {
    if (event.nativeEvent.isComposing || event.keyCode === 229) return;
    if (["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
      event.preventDefault();
      if (items.length) setActiveIndex(current => event.key === "Home" ? 0 : event.key === "End" ? items.length - 1 : (current + (event.key === "ArrowDown" ? 1 : -1) + items.length) % items.length);
    } else if (event.key === "Enter" && activeItem && !searching) { event.preventDefault(); activate(activeItem); }
  }
  async function showSources() {
    if (onSources) await onSources();
    else window.dispatchEvent(new CustomEvent("repoatlas:session-history", { detail: { sources: true } }));
  }
  const content = <div className="command-panel">
    <div className="command-search-row">
      <MagnifyingGlass size={20} className="command-search-icon" aria-hidden />
      <input ref={input} autoFocus maxLength={512} role="combobox" aria-label={t("commandHint")} aria-autocomplete="list" aria-haspopup="listbox"
        aria-controls={`${listId}-list`} aria-expanded={open} aria-activedescendant={activeId} aria-busy={searching}
        className="command-search-input" placeholder={t("qsSearchPrompt")} value={query} onChange={event => setQuery(event.target.value)} onKeyDown={keyDown} />
      <Button variant="quiet" size="icon" aria-label={t("close")} onClick={onClose}><X size={17} /></Button>
    </div>
    <div className="command-toolbar" data-tauri-drag-region={standalone || undefined}>
      <div className="command-scopes" role="group" aria-label={t("qsFilter")}>
        {([['all', 'qsAll'], ['sessions', 'qsSessions'], ['projects', 'projects'], ['actions', 'qsActions']] as const).map(([value, label]) =>
          <button key={value} type="button" aria-pressed={scope === value} onClick={() => setScope(value)}>{t(label)}</button>)}
      </div>
      <Button variant="quiet" size="icon" aria-label={t("ahSources")} title={t("ahSources")} onClick={() => void perform(showSources)}><SlidersHorizontal size={17} /></Button>
    </div>
    {(errors.length > 0 || actionError || sourceError) && <div className="command-error" role="alert">{[...errors, actionError, sourceError].filter(Boolean).join(" · ")}
      {(errors.length > 0 || sourceError) && <Button variant="quiet" onClick={() => setRevision(value => value + 1)}>{t("retry")}</Button>}
    </div>}
    <div className={`command-body${activeItem?.kind === "session" ? " command-body-preview" : ""}`}>
      <div className="command-list-pane">
        {searching && <div className="command-loading" role="status">{t("qsSearching")}</div>}
        <div id={`${listId}-list`} role="listbox" className="command-results" aria-label={t("commandHint")} aria-busy={searching || actionBusy}>
          {groups.map(group => <div className="command-group" key={group.key} role="group" aria-labelledby={`${listId}-group-${group.key}`}>
            <div id={`${listId}-group-${group.key}`} className="command-group-label">{group.label} <span aria-hidden>{group.items.length}</span></div>
            {group.items.map(item => <button id={`${listId}-${item.key}`} key={item.key} type="button" role="option" tabIndex={-1}
              aria-selected={item === activeItem} className={`command-item ${item === activeItem ? "command-item-active" : ""}`}
              onClick={() => activate(item)}>
              <span className="command-item-icon" aria-hidden>{item.kind === "project" ? <FolderSimple size={18} /> : item.kind === "session" ? <AgentBrandIcon agent={item.hit.session.adapter} /> : <Lightning size={18} />}</span>
              <span className="command-item-copy">
                <span className="command-item-title" title={item.title}><SearchHighlight text={item.title} query={q} /></span>
                {item.kind === "session" ? <>
                  <span className="command-session-meta"><strong title={item.hit.session.cwd}>{item.hit.session.projectName || item.hit.session.cwd.split(/[\\/]/).filter(Boolean).pop() || t("ahUnlinked")}</strong><span>{sessionAgentName(item.hit.session.adapter)}</span>
                    <time dateTime={item.hit.session.updatedAt} title={new Date(item.hit.session.updatedAt).toLocaleString()}>{sessionTime(item.hit.session.updatedAt, chinese)}</time>
                    {item.hit.session.archived && <span>{t("archived")}</span>}
                  </span>
                  <TokenBadge usage={item.hit.session.usage} t={t} />
                  {q && sessionExcerpt(item.hit.snippets[0]?.content || item.hit.session.lastUserExcerpt) && <span className="command-session-excerpt"><SessionSnippet content={item.hit.snippets[0]?.content || item.hit.session.lastUserExcerpt} query={q} /></span>}
                </> : item.hint && <span className="command-item-hint" title={item.hint}><SearchHighlight text={item.hint} query={q} /></span>}
              </span>
            </button>)}
          </div>)}
        </div>
        {!searching && items.length === 0 && <p className="command-status" role="status">{t("paletteEmpty")}</p>}
        {sources?.every(source => !source.enabled) && <div className="command-empty-source"><strong>{t("ahNoSources")}</strong><p>{t("ahNoSourcesHint")}</p></div>}
      </div>
      {activeItem?.kind === "session" && <div ref={preview} className="command-preview" tabIndex={-1} aria-label={t("ahPreviewAction")} onKeyDown={event => {
        if (event.key === "ArrowLeft" && event.target === event.currentTarget) { event.preventDefault(); input.current?.focus(); }
      }}>
        <SessionSearchPreview key={activeItem.hit.session.id} hit={activeItem.hit} query={q} t={t} displayTitle={activeItem.title}
          onProject={onProject ? id => perform(() => onProject(id)) : undefined}
          onSession={(session, messageIndex) => perform(() => onSession ? onSession(session, messageIndex) : openSessionHistory(undefined, session, messageIndex))} />
      </div>}
    </div>
    <footer className="command-footer">
      <span className="command-shortcut-status">{shortcutHint}</span>
      <span className="command-footer-hint"><ArrowUp size={14} /><ArrowDown size={14} />{t("navigate")}</span>
      <span className="command-footer-hint"><ArrowElbowDownLeft size={14} />{t(activeItem?.kind === "session" ? "ahPreviewAction" : "openItem")}</span>
      <span className="command-footer-hint"><kbd>Tab</kbd>{t("qsActions")}</span>
      <span className="command-footer-hint"><kbd>Esc</kbd>{t("close")}</span>
    </footer>
  </div>;

  if (standalone) return open ? <main id="main-content" className="command-standalone" aria-label={t("qsTitle")} onKeyDown={event => {
    if (event.key === "Escape" && !event.defaultPrevented) { event.preventDefault(); onClose(); }
  }}>{content}</main> : null;
  return <MotionConfig reducedMotion="user"><Dialog.Root open={open} onOpenChange={next => { if (!next) onClose(); }}>
    <Dialog.Portal><Dialog.Backdrop className="command-backdrop" /><Dialog.Popup className="command-workbench" initialFocus={input}>
      <Dialog.Title className="command-visually-hidden">{t("qsTitle")}</Dialog.Title>
      <Dialog.Description className="command-visually-hidden">{t("commandHint")}</Dialog.Description>
      {content}
    </Dialog.Popup></Dialog.Portal>
  </Dialog.Root></MotionConfig>;
}
