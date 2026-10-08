import { Dialog } from "@base-ui/react/dialog";
import { open as chooseDirectory } from "@tauri-apps/plugin-dialog";
import { ArrowClockwise, ArrowRight, Copy, FolderOpen, ChatCircleText, ClockCounterClockwise, MagnifyingGlass, SlidersHorizontal, X } from "@phosphor-icons/react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { MessageKey } from "../i18n";
import type { ProjectIcon, ProjectSummary } from "../types";
import { api } from "../lib/api";
import { openSessionHistory, sessionApi, sessionTime } from "../lib/sessions";
import type { AgentSession, SessionMessage, SessionRefreshJob, SessionSearchHit, SessionSource } from "../lib/sessions";
import { Button } from "./ui/button";
import { ConfirmDialog } from "./ui/confirm";
import { SearchPicker } from "./ui/search-picker";
import { SessionAgentLabel, SessionAgentSelect } from "./ui/session-agent";

import { SessionResumeButton as ResumeButton, sessionResumeReason as reason } from "./ui/session-resume";
import { sessionDisplayText, sessionDisplayTitle, sessionExcerpt } from "../lib/session-presentation";
import { SessionContent, SessionSnippet } from "./ui/session-content";
import { DetailPopover } from "./ui/detail-popover";
import { TokenBadge, TokenBreakdown } from "./ui/token-usage";

type T = (key: MessageKey) => string;
function Stamp({ value, t }: { value: string; t: T }) {
  return <time dateTime={value} title={new Date(value).toLocaleString()}>{sessionTime(value, t("ahUser") === "用户")}</time>;
}
function Highlight({ text, query }: { text: string; query: string }) {
  const terms = query.trim().split(/\s+/).filter(Boolean).map(s => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
  if (!terms.length) return <>{text}</>;
  const pattern = new RegExp(`(${terms.join("|")})`, "gi");
  return <>{text.split(pattern).map((part, i) => i % 2 ? <mark key={i}>{part}</mark> : part)}</>;
}

function groupMessages(messages:SessionMessage[]) {
  const groups:{technical:boolean;messages:SessionMessage[]}[]=[];
  for(const message of messages){
    const technical=!sessionDisplayText(message.content).trim();
    const last=groups[groups.length-1];
    if(last?.technical===technical)last.messages.push(message);else groups.push({technical,messages:[message]});
  }
  return groups;
}

/** Mount independently of the project tree so opening/closing preserves its navigation. */
export function AIHistory({ t, initialOpen = false, onVisibilityChange, onOpenProject }: { t: T; initialOpen?: boolean; onVisibilityChange?: (open: boolean) => void; onOpenProject?: (id: string) => void | Promise<void> }) {
  const [isOpen, setOpen] = useState(initialOpen); const [sourcesOpen, setSourcesOpen] = useState(false);
  const [query, setQuery] = useState(""); const [projectId, setProjectId] = useState(""); const [adapter, setAdapter] = useState("");
  const [after, setAfter] = useState(""); const [before, setBefore] = useState(""); const [archived, setArchived] = useState(false);
  const [offset, setOffset] = useState(0); const [hits, setHits] = useState<SessionSearchHit[]>([]); const [total, setTotal] = useState(0);
  const [selected, setSelected] = useState<AgentSession>(); const [messages, setMessages] = useState<SessionMessage[]>([]);
  const [messageOffset, setMessageOffset] = useState(0); const [messageBusy, setMessageBusy] = useState(false);
  const [sources, setSources] = useState<SessionSource[]>([]); const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [searchError, setSearchError] = useState("");
  const [busy, setBusy] = useState(false); const [error, setError] = useState(""); const [notice, setNotice] = useState("");
  const [revision, setRevision] = useState(0); const [job, setJob] = useState<SessionRefreshJob>();
  useEffect(() => { onVisibilityChange?.(isOpen); }, [isOpen, onVisibilityChange]);
  const [bulkSources, setBulkSources] = useState<SessionSource[]>([]);
  const [pendingSource, setPendingSource] = useState<SessionSource>(); const [sourceBusy, setSourceBusy] = useState(false);
  const [sourceError, setSourceError] = useState("");
  const [customAdapter, setCustomAdapter] = useState("claude"); const [customPath, setCustomPath] = useState("");
  const [rebuild, setRebuild] = useState(false);
  const [linking, setLinking] = useState(false);
  const [openingProject, setOpeningProject] = useState(false);
  const [filtersOpen, setFiltersOpen] = useState(false);
  const projectSettings = useRef<HTMLButtonElement>(null);
  const input = useRef<HTMLInputElement>(null); const resultPane = useRef<HTMLDivElement>(null); const transcript = useRef<HTMLDivElement>(null);
  const sequence = useRef(0); const messageSequence = useRef(0); const opener = useRef<HTMLElement | null>(null);
  const resultScroll = useRef(0); const messageScroll = useRef(new Map<string, number>()); const jump = useRef<number | null>(null);
  const messagePages = useRef(new Map<string, number>());
  const poll = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const openRef = useRef(isOpen); openRef.current = isOpen;

  const reloadSources = useCallback(async () => { const items = await sessionApi.sources(); setSources(items); return items; }, []);
  useEffect(() => {
    const open = (event: Event) => {
      const detail = (event as CustomEvent<{ projectId?: string; sources?: boolean; session?: AgentSession; messageIndex?: number }>).detail;
      opener.current = document.activeElement as HTMLElement;
      if (detail?.projectId !== undefined) { setProjectId(detail.projectId); setOffset(0); }
      if (detail?.session) {
        const index = Math.max(0, detail.messageIndex ?? 0);
        setSelected(detail.session); setMessageOffset(Math.floor(index / 40) * 40); setMessages([]); jump.current = detail.messageIndex ?? null;
      }
      setSourcesOpen(Boolean(detail?.sources)); setOpen(true);
    };
    const close = () => setOpen(false);
    window.addEventListener("repoatlas:close-sessions", close);
    window.addEventListener("repoatlas:session-history", open);
    return () => { window.removeEventListener("repoatlas:session-history", open); window.removeEventListener("repoatlas:close-sessions", close); };
  }, []);
  useEffect(() => {
    if (!isOpen) return;
    let active = true;
    void reloadSources().catch(e => active && setError(String(e)));
    void api.listProjects({ includeArchived: true, limit: 5000 }).then(p => active && setProjects(p)).catch(e => active && setError(String(e)));
    const tick = async () => {
      try {
        const next = await sessionApi.status(); if (!active) return; setJob(next);
        if (next.running) { poll.current = setTimeout(tick, 900); }
        else { if (next.errors || next.canceled) setNotice(t(next.canceled ? "ahCanceled" : "ahPartial")); setRevision(v => v + 1); await reloadSources(); window.dispatchEvent(new Event("repoatlas:sessions-updated")); }
      } catch (e) { if (active) setError(String(e)); }
    };
    void sessionApi.refresh().then(() => { if (active) return tick(); }).catch(e => active && setError(String(e)));
    return () => { active = false; clearTimeout(poll.current); };
  }, [isOpen, reloadSources]);

  useEffect(() => {
    if (!isOpen) return;
    const ticket = ++sequence.current; setBusy(true); setSearchError("");
    const timer = setTimeout(() => {
      const until = before ? new Date(`${before}T00:00:00`) : undefined;
      until?.setDate(until.getDate() + 1);
      void sessionApi.search({ query, projectId: projectId || undefined, adapter: adapter || undefined, archived, offset, limit: 50,
        after: after ? new Date(`${after}T00:00:00`).toISOString() : undefined, before: until?.toISOString() }).then(result => {
        if (ticket !== sequence.current) return; setHits(result.items); setTotal(result.total); setSearchError("");
        requestAnimationFrame(() => { if (resultPane.current) resultPane.current.scrollTop = resultScroll.current; });
      }).catch(e => ticket === sequence.current && setSearchError(String(e))).finally(() => ticket === sequence.current && setBusy(false));
    }, 200);
    return () => { clearTimeout(timer); sequence.current++; };
  }, [isOpen, query, projectId, adapter, after, before, archived, offset, revision]);

  useEffect(() => {
    if (!isOpen || !selected) return;
    const ticket = ++messageSequence.current; setMessageBusy(true);
    void sessionApi.messages(selected.id, messageOffset).then(page => {
      if (ticket !== messageSequence.current) return; setMessages(page.items);
    }).catch(e => { if (ticket === messageSequence.current) { setMessages([]); setError(reason(String(e), t)); } }).finally(() => ticket === messageSequence.current && setMessageBusy(false));
    return () => { messageSequence.current++; };
  }, [isOpen, selected?.id, selected?.revision, messageOffset, revision]);

  // Wait for React to commit the requested message page before resolving the hit anchor.
  useEffect(() => {
    if (!isOpen || !selected || messageBusy || !messages.length) return;
    const frame = requestAnimationFrame(() => {
      if (jump.current !== null) {
        const target = document.getElementById(`ah-message-${jump.current}`);
        if (!target) return;
        const group = target.closest<HTMLDetailsElement>("details.ah-technical-group");
        if (group) group.open = true;
        target.scrollIntoView({ block: "start" });
        if (transcript.current) messageScroll.current.set(`${selected.id}:${messageOffset}`, transcript.current.scrollTop);
        jump.current = null;
      } else if (transcript.current) transcript.current.scrollTop = messageScroll.current.get(`${selected.id}:${messageOffset}`) ?? 0;
    });
    return () => cancelAnimationFrame(frame);
  }, [isOpen, selected?.id, messageOffset, messages, messageBusy]);

  async function refresh(force = true) {
    setError(""); setNotice("");
    try {
      let next = await sessionApi.refresh(force); if (!openRef.current) return; setJob(next);
      const tick = async () => {
        try { next = await sessionApi.status(); if (!openRef.current) return; setJob(next);
          if (next.running) poll.current = setTimeout(tick, 900);
          else { setRevision(v => v + 1); await reloadSources(); setNotice(t(next.canceled ? "ahCanceled" : next.errors ? "ahPartial" : "ahUpdated")); window.dispatchEvent(new Event("repoatlas:sessions-updated")); }
        } catch (e) { setError(String(e)); }
      };
      clearTimeout(poll.current); poll.current = setTimeout(tick, 500);
    } catch (e) { setError(String(e)); }
  }
  function filter(update: () => void) { update(); setOffset(0); resultScroll.current = 0; }
  function select(hit: SessionSearchHit) {
    if (selected) messagePages.current.set(selected.id, messageOffset);
    setSelected(hit.session); const index = hit.snippets[0]?.index;
    jump.current = index ?? null; setMessageOffset(index === undefined ? messagePages.current.get(hit.session.id) ?? 0 : Math.floor(index / 40) * 40); setNotice("");
  }
  const enabled = sources.filter(s => s.enabled);
  const messageGroups = useMemo(()=>groupMessages(messages),[messages]);
  const emptyTitle = !enabled.length ? "ahNoSources" : enabled.every(s => !s.lastScannedAt) ? "ahUnindexed" : "ahEmpty";
  return <Dialog.Root modal={false} open={isOpen} onOpenChange={(open, details) => { if(open || (details.reason!=="outside-press" && details.reason!=="focus-out"))setOpen(open); }}>
    <Dialog.Portal><Dialog.Backdrop className="ah-backdrop" />
      <Dialog.Popup className="ah-dialog" initialFocus={input} finalFocus={() => opener.current} onKeyDown={e => {
        if ((e.key === "/" || ((e.ctrlKey || e.metaKey) && e.key === "f")) && !(e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement)) { e.preventDefault(); input.current?.focus(); }
      }}>
        <header className="ah-header"><Dialog.Title className="sr-only">{t("ahTitle")}</Dialog.Title><Dialog.Description className="sr-only">{t("ahCached")}</Dialog.Description>
          {!sourcesOpen && <>
            <label className="ah-search"><MagnifyingGlass aria-hidden="true" /><input ref={input} value={query} maxLength={512} placeholder={t("ahSearch")} aria-label={t("ahSearch")} onChange={e => filter(() => setQuery(e.target.value))} />{query && <button aria-label={t("clear")} onClick={() => filter(() => setQuery(""))}><X /></button>}</label>
            <SearchPicker items={projects.map(p => ({ id: p.id, name: p.displayName }))} value={projectId} onChange={id => filter(() => setProjectId(id))} allLabel={t("ahAllProjects")} searchLabel={t("ahLink")} emptyLabel={t("ahEmpty")} />
            <SessionAgentSelect value={adapter} onChange={value => filter(() => setAdapter(value))} label={t("ahAllAgents")} allLabel={t("ahAllAgents")} />
            <Button variant="quiet" aria-expanded={filtersOpen} onClick={()=>setFiltersOpen(v=>!v)}><SlidersHorizontal/>{t("ahMoreFilters")}{(after||before||archived) && <span className="ah-filter-dot"/>}</Button>
          </>}
          <Button className={enabled.some(s => s.lastError)?"ah-source-warning":undefined} title={enabled.some(s => s.lastError)?t("ahPartial"):undefined} variant="quiet" onClick={() => setSourcesOpen(v => !v)} aria-pressed={sourcesOpen}><SlidersHorizontal />{t("ahSources")}</Button>
          <Button variant="quiet" disabled={job?.running || !enabled.length} onClick={() => void refresh()}><ArrowClockwise />{t("refresh")}</Button>
          <Dialog.Close render={<Button variant="quiet" size="icon" aria-label={t("close")}><X /></Button>} />
        </header>
        {(error || searchError || notice || job?.running) && <div className="ah-status" role="status">
          {error || searchError || (job?.running ? `${t("ahUpdating")} · ${job.processed}` : notice)}
          {job?.running && <Button variant="quiet" onClick={() => void sessionApi.cancel().catch(e => setError(String(e)))}>{t("cancel")}</Button>}
          {(error || searchError) && <Button variant="quiet" onClick={() => { setError(""); setRevision(v => v + 1); void refresh(); }}>{t("retry")}</Button>}
        </div>}
        {sourcesOpen ? <div className="ah-sources">
          <div className="ah-source-heading"><h2>{t("ahSources")}</h2><Button variant="primary" disabled={sourceBusy || !sources.some(s => !s.enabled)} onClick={() => { setSourceError(""); setBulkSources(sources.filter(s => !s.enabled)); }}>{t("ahAuthorizeAll")} ({sources.filter(s => !s.enabled).length})</Button></div><p>{t("ahSourcesHint")}</p>
          {sources.map(source => <div className="ah-source-row" key={`${source.adapter}:${source.path}`}>
            <div><strong><SessionAgentLabel adapter={source.adapter} /></strong><code title={source.path}>{source.path}</code>
              {source.lastScannedAt && <small>{t("ahCached")} · <Stamp value={source.lastScannedAt} t={t} /></small>}
              {source.lastError && <p role="status">{t("ahPartial")} <code>{source.lastError}</code></p>}
            </div><Button variant="quiet" onClick={() => { setSourceError(""); setPendingSource(source); }}>{t(source.enabled ? "ahDisable" : "ahAuthorize")}</Button>
          </div>)}
          <form className="ah-source-form" onSubmit={e => { e.preventDefault(); setSourceError(""); setPendingSource({ id: "", adapter: customAdapter, path: customPath.trim(), enabled: false, lastScannedAt: null, lastError: null }); }}>
            <label>{t("ahSource")}<SessionAgentSelect value={customAdapter} onChange={setCustomAdapter} label={t("ahSource")} /></label>
            <label className="ah-grow">{t("ahCustomPath")}<input required value={customPath} onChange={e => setCustomPath(e.target.value)} placeholder={t("ahCustomPath")} /></label>
            <Button variant="quiet" type="button" aria-label={t("ahCustomPath")} onClick={() => void chooseDirectory({ directory: true, multiple: false }).then(p => { if (typeof p === "string") setCustomPath(p); }).catch(e => setError(String(e)))}><FolderOpen /></Button>
            <Button type="submit" disabled={!customPath.trim()}>{t("ahAdd")}</Button>
          </form>
          <Button variant="quiet" disabled={job?.running || !enabled.length} onClick={() => setRebuild(true)}>{t("ahRebuild")}</Button>
        </div> : <>
          {filtersOpen && <div className="ah-advanced-filters"><label>{t("ahFrom")}<input type="date" value={after} max={before || undefined} onChange={e => filter(() => setAfter(e.target.value))} /></label>
            <label>{t("ahTo")}<input type="date" value={before} min={after || undefined} onChange={e => filter(() => setBefore(e.target.value))} /></label>
            <label className="ah-check"><input type="checkbox" checked={archived} onChange={e => filter(() => setArchived(e.target.checked))} />{t("ahArchived")}</label>
            {(after||before||archived) && <Button variant="quiet" onClick={()=>filter(()=>{setAfter("");setBefore("");setArchived(false);})}>{t("clear")}</Button>}
          </div>}
          <div className="ah-body"><section className="ah-results"><div className="ah-result-count" aria-live="polite"><span>{total} {t("ahResults")} · {t("ahCached")}</span>{busy && <span>{t("ahPending")}</span>}</div>
            <div ref={resultPane} className="ah-result-scroll" aria-busy={busy} onScroll={e => { resultScroll.current = e.currentTarget.scrollTop; }} onKeyDown={e => {
              if (!["ArrowUp", "ArrowDown"].includes(e.key)) return;
              const buttons = Array.from(e.currentTarget.querySelectorAll<HTMLButtonElement>("button.ah-result"));
              const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
              buttons[Math.max(0, Math.min(buttons.length - 1, index + (e.key === "ArrowDown" ? 1 : -1)))]?.focus(); e.preventDefault();
            }}>
              {(busy || error || searchError) && hits.length > 0 && <p className="muted-copy" role="status">{t("stateCached")}</p>}
              {!hits.length && !busy && !error && !searchError && <div className="ah-empty"><MagnifyingGlass /><h3>{t(emptyTitle)}</h3>{!enabled.length && <><p>{t("ahNoSourcesHint")}</p><Button onClick={() => setSourcesOpen(true)}>{t("ahSources")}</Button></>}{enabled.length > 0 && <Button onClick={() => { if (query || projectId || adapter || after || before || archived) filter(() => { setQuery(""); setProjectId(""); setAdapter(""); setAfter(""); setBefore(""); setArchived(false); }); else void refresh(); }}>{t(query || projectId || adapter || after || before || archived ? "clear" : "refresh")}</Button>}</div>}
              {hits.map(hit => <button className={`ah-result${selected?.id === hit.session.id ? " is-selected" : ""}`} key={hit.session.id} onClick={() => select(hit)} aria-pressed={selected?.id === hit.session.id}>
                <span className="ah-result-meta"><span><SessionAgentLabel adapter={hit.session.adapter} /></span><Stamp value={hit.session.updatedAt} t={t} /></span>
                <strong title={hit.session.title}><Highlight text={sessionDisplayTitle(hit.session.title, sessionExcerpt(hit.session.lastUserExcerpt) || t("ahUntitled"))} query={query} /></strong>
                <span className="ah-result-location"><span className="ah-project-name">{hit.session.projectName ?? t("ahUnlinked")}</span><TokenBadge usage={hit.session.usage} t={t} /></span>
                {hit.snippets.slice(0, 1).map(m => <p key={m.index}><SessionSnippet content={m.content} query={query} /></p>)}
                {!hit.snippets.length && hit.session.lastUserExcerpt && <p><SessionSnippet content={hit.session.lastUserExcerpt} /></p>}
              </button>)}
            </div><footer className="ah-pagination"><Button variant="quiet" disabled={offset === 0 || busy} onClick={() => { setOffset(v => Math.max(0, v - 50)); resultScroll.current = 0; }}>{t("ahPrevious")}</Button><span>{total ? `${offset + 1}–${Math.min(offset + 50, total)}` : "0"}</span><Button variant="quiet" disabled={offset + 50 >= total || busy} onClick={() => { setOffset(v => v + 50); resultScroll.current = 0; }}>{t("ahNext")}</Button></footer>
          </section><section className="ah-detail" aria-busy={messageBusy}>
            {!selected ? <div className="ah-empty"><h3>{t("ahPreview")}</h3></div> : <>
              <header className="ah-detail-header"><div className="ah-detail-eyebrow"><SessionAgentLabel adapter={selected.adapter}/><Stamp value={selected.updatedAt} t={t}/><span>{selected.messageCount} {t("ahMessages")}</span></div>
                <h2>{sessionDisplayTitle(selected.title, sessionExcerpt(selected.lastUserExcerpt) || t("ahUntitled"))}</h2>
                <div className="ah-detail-actions">
                  {selected.projectId && onOpenProject ? <Button variant="quiet" className="ah-open-project" loading={openingProject} title={selected.cwd} onClick={()=>{setOpeningProject(true);setError("");void Promise.resolve().then(()=>onOpenProject(selected.projectId!)).then(()=>setOpen(false)).catch(()=>setError(t("projectLoadFailed"))).finally(()=>setOpeningProject(false));}}><FolderOpen/><span>{selected.projectName??t("ahOpenProject")}</span><ArrowRight/>{t("ahOpenProject")}</Button> : <Button variant="quiet" onClick={()=>{projectSettings.current?.click();}}><FolderOpen/>{t("ahLink")}</Button>}
                  <ResumeButton key={selected.id} session={selected} t={t} onError={setError}/>
                </div>
                {selected.resumeReason && <p role="status">{reason(selected.resumeReason, t)}</p>}
                <div className="ah-detail-secondary" key={selected.id}><TokenBreakdown floating usage={selected.usage} t={t}/>
                <DetailPopover triggerRef={projectSettings} title={t("ahSessionDetails")} closeLabel={t("close")}><div className="ah-settings-content"><code title={selected.cwd}>{selected.cwd || t("ahCwdMissing")}</code><div className="ah-link-controls">
                  <SearchPicker items={projects.map(p => ({ id: p.id, name: p.displayName }))} value={selected.projectId ?? ""} allLabel={t("ahUnlinked")} searchLabel={t("ahLink")} emptyLabel={t("ahEmpty")} onChange={id => {
                    if (linking) return; setLinking(true); void sessionApi.link(selected.id, id || null).then(() => sessionApi.get(selected.id)).then(s => { setSelected(s); setRevision(v => v + 1); setNotice(t("ahSaved")); }).catch(e => setError(String(e))).finally(() => setLinking(false));
                  }} />
                  <Button variant="quiet" disabled={linking} onClick={() => void chooseDirectory({ directory: true, multiple: false }).then(async p => {
                    if (typeof p !== "string") return; setLinking(true); try { await sessionApi.link(selected.id, selected.projectId, p); setSelected(await sessionApi.get(selected.id)); setNotice(t("ahSaved")); } finally { setLinking(false); }
                  }).catch(e => setError(String(e)))}><FolderOpen />{t("ahRelocate")}</Button>
                </div><Button variant="quiet" disabled={!selected.capabilities.directResume} onClick={() => void sessionApi.resumeSpec(selected.id).then(spec => navigator.clipboard.writeText(spec.command)).then(() => setNotice(t("ahCopied"))).catch(e => setError(reason(String(e), t)))}><Copy />{t("ahCopy")}</Button><code title={selected.sourceLocator}>{selected.sourceLocator}</code><code>{selected.externalId}</code></div></DetailPopover></div>
              </header>
              <div className="ah-transcript" ref={transcript} onScroll={e => messageScroll.current.set(`${selected.id}:${messageOffset}`, e.currentTarget.scrollTop)}>
                {messageBusy ? <p role="status">{t("ahPending")}</p> : messageGroups.map(group=>{
                  const renderMessage=(message:SessionMessage)=><article id={`ah-message-${message.index}`} key={`${selected.id}:${message.index}`} className={`ah-message ${message.role}`}><header><strong>{t(message.role === "user" ? "ahUser" : "ahAssistant")}</strong>{message.timestamp && <Stamp value={message.timestamp} t={t}/>}</header><SessionContent content={message.content} query={query} t={t} copy/></article>;
                  return group.technical ? <details className="ah-technical-group" key={`${selected.id}:technical:${group.messages[0].index}`}><summary><SlidersHorizontal size={13}/>{t("ahSystemContext")}<span>{group.messages.length}</span></summary>{group.messages.map(renderMessage)}</details> : group.messages.map(renderMessage);
                })}
              </div><footer className="ah-pagination"><Button variant="quiet" disabled={!messageOffset || messageBusy} onClick={() => setMessageOffset(v => Math.max(0, v - 40))}>{t("ahPrevious")}</Button><span>{selected.messageCount ? `${messageOffset + 1}–${Math.min(messageOffset + 40, selected.messageCount)}` : "0"} / {selected.messageCount}</span><Button variant="quiet" disabled={messageOffset + 40 >= selected.messageCount || messageBusy} onClick={() => setMessageOffset(v => v + 40)}>{t("ahNext")}</Button></footer>
            </>}
          </section></div>
        </>}
        <ConfirmDialog confirmVariant="primary" className="ah-confirm ah-bulk-confirm" open={bulkSources.length > 0} title={t("ahAuthorizeAll")} body={<span className="session-source-confirm"><span>{t("ahSourcesHint")}</span>{bulkSources.map(source => <span key={source.id}><SessionAgentLabel adapter={source.adapter} /><code>{source.path}</code></span>)}{sourceError && <span>{sourceError}</span>}</span>} confirmLabel={t("ahAuthorizeAll")} cancelLabel={t("cancel")} busy={sourceBusy} onOpenChange={value => { if (!value && !sourceBusy) setBulkSources([]); }} onConfirm={async () => {
          if (sourceBusy) return; setSourceBusy(true); setSourceError(""); const failed: SessionSource[] = []; const errors: string[] = [];
          for (const source of bulkSources) { try { await sessionApi.setSource(source.adapter, source.path, true); } catch (e) { failed.push(source); errors.push(`${source.path}: ${String(e)}`); } }
          setBulkSources(failed); setSourceError(errors.join("\n"));
          try { await reloadSources(); setRevision(v => v + 1); window.dispatchEvent(new Event("repoatlas:sessions-updated")); await refresh(); } catch (e) { setError(String(e)); }
          finally { setSourceBusy(false); }
        }} />
        <ConfirmDialog confirmVariant={pendingSource?.enabled ? "danger" : "primary"} className="ah-confirm" open={!!pendingSource} title={t(pendingSource?.enabled ? "ahDisableConfirm" : "ahEnableConfirm")} body={`${pendingSource?.path ?? ""}\n\n${t(pendingSource?.enabled ? "ahDisableHint" : "ahSourcesHint")}\n${sourceError}`} confirmLabel={t(pendingSource?.enabled ? "ahDisable" : "ahAuthorize")} cancelLabel={t("cancel")} busy={sourceBusy} onOpenChange={open => { if (!open && !sourceBusy) setPendingSource(undefined); }} onConfirm={async () => {
          if (!pendingSource || sourceBusy) return; setSourceBusy(true);
          try { await sessionApi.setSource(pendingSource.adapter, pendingSource.path, !pendingSource.enabled); sequence.current++; messageSequence.current++; setSelected(undefined); setMessages([]); setHits([]); setPendingSource(undefined); setCustomPath(""); await reloadSources(); setRevision(v => v + 1); window.dispatchEvent(new Event("repoatlas:sessions-updated")); if (!pendingSource.enabled) await refresh(); }
          catch (e) { setSourceError(String(e)); setError(String(e)); } finally { setSourceBusy(false); }
        }} />
        <ConfirmDialog className="ah-confirm" open={rebuild} title={t("ahRebuild")} body={t("ahRebuildHint")} confirmLabel={t("ahRebuild")} cancelLabel={t("cancel")} busy={sourceBusy} onOpenChange={setRebuild} onConfirm={async () => {
          setSourceBusy(true); try { await sessionApi.rebuild(); setMessages([]); setHits([]); setSelected(undefined); setRebuild(false); await refresh(); } catch (e) { setSourceError(String(e)); setError(String(e)); } finally { setSourceBusy(false); }
        }} />
      </Dialog.Popup>
    </Dialog.Portal>
  </Dialog.Root>;
}

export function ContinueCoding({ t, projectId, onOpenProject, onStart, compact = false }: { t: T; projectId?: string; onOpenProject?: (id: string) => void; onStart?: () => void; compact?: boolean }) {
  const [icons, setIcons] = useState<Record<string, ProjectIcon>>({});
  const [sessions, setSessions] = useState<AgentSession[]>([]);
  const [error, setError] = useState("");
  const [refreshError, setRefreshError] = useState("");
  const [loaded, setLoaded] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [revision, setRevision] = useState(0);
  useEffect(() => {
    let active = true;
    let sequence = 0;
    let timer: ReturnType<typeof setTimeout> | undefined;
    setError(""); setRefreshError("");
    const load = async () => {
      const ticket = ++sequence;
      try {
        const items = await sessionApi.recent(projectId);
        if (active && ticket === sequence) { setSessions(items); setError(""); setLoaded(true); }
      } catch (e) {
        if (active && ticket === sequence) { setError(String(e)); setLoaded(true); }
      }
    };
    const failRefresh = (e: unknown) => {
      if (active) { setRefreshError(String(e)); setRefreshing(false); }
    };
    const tick = async () => {
      try {
        const status = await sessionApi.status();
        if (!active) return;
        setRefreshing(status.running);
        if (status.running) timer = setTimeout(tick, 1000);
        else {
          if (status.errors) setRefreshError(t("ahPartial"));
          if (status.canceled) setRefreshError(t("ahCanceled"));
          await load();
        }
      } catch (e) { failRefresh(e); }
    };
    const changed = () => { void load(); };
    window.addEventListener("repoatlas:sessions-updated", changed);
    void load();
    // Ordinary Project selection only reads cached history; Home and retry may refresh.
    if (!projectId || revision > 0) {
      setRefreshing(true);
      void sessionApi.refresh().then(() => { if (active) return tick(); }).catch(failRefresh);
    } else void tick();
    return () => { active = false; clearTimeout(timer); window.removeEventListener("repoatlas:sessions-updated", changed); };
  }, [projectId, revision, t]);
  const ids = sessions.map(s => s.projectId).filter((id): id is string => !!id).join("|");
  useEffect(() => {
    let active = true;
    if (!ids) { setIcons({}); return; }
    void api.readProjectIcons(ids.split("|")).then(items => { if (active) setIcons(Object.fromEntries(items.map(icon => [icon.projectId, icon]))); }).catch(() => { if (active) setIcons({}); });
    return () => { active = false; };
  }, [ids]);
  const visible = compact ? sessions.slice(0, 3) : sessions;
  return <section className={`ah-continue${projectId ? " is-project" : ""}${compact ? " is-compact" : ""}`} aria-label={t("ahContinue")}>
    <header className="ah-continue-header"><div className="ah-continue-heading"><h2>{t("ahContinue")}</h2>{loaded && sessions.length > 0 && <span className="ah-session-count">{visible.length}</span>}</div>
      <Button variant="quiet" onClick={() => openSessionHistory(projectId)}>{t("ahMore")}<ArrowRight aria-hidden="true" /></Button>
    </header>
    {!loaded && <p className="ah-continue-empty" role="status">{t("ahPending")}</p>}
    {loaded && !sessions.length && !error && <div className="ah-continue-empty"><p>{t("ahNoResume")}</p><div className="empty-actions"><Button onClick={() => window.dispatchEvent(new CustomEvent("repoatlas:session-history", { detail: { projectId, sources: true } }))}>{t("sessionConnect")}</Button>{onStart && <Button variant="primary" onClick={onStart}>{t(projectId ? "openAgent" : "chooseProject")}</Button>}</div></div>}
    {refreshing && <p className="muted-copy" role="status">{t("sessionRefreshing")}</p>}
    {(error || refreshError) && <div className="session-load-error" role="status"><p>{t(error ? "sessionReadFailed" : "sessionRefreshFailed")}{sessions.length > 0 && error ? ` · ${t("stateCached")}` : ""}</p><p>{error || refreshError}</p><Button loading={refreshing} onClick={() => setRevision(value => value + 1)}>{t("retry")}</Button><Button variant="quiet" onClick={() => window.dispatchEvent(new CustomEvent("repoatlas:session-history", { detail: { projectId, sources: true } }))}>{t("sessionConnect")}</Button></div>}
    <div className="ah-continue-items">{visible.map(session => {
      const title = sessionDisplayTitle(session.title, sessionExcerpt(session.lastUserExcerpt) || t("ahUntitled"));
      const preview = () => openSessionHistory(session.projectId ?? undefined, session);
      return <article className="ah-session-card" key={session.id}>
        <header className="ah-card-header"><button className="ah-card-project" onClick={() => { if (session.projectId && onOpenProject) onOpenProject(session.projectId); else preview(); }} title={session.cwd}><span className="ah-project-symbol">{session.projectId && icons[session.projectId]?.dataUrl ? <img src={icons[session.projectId].dataUrl!} alt="" /> : <FolderOpen aria-hidden="true" />}</span><h3>{session.projectName || session.cwd}</h3></button><span className="ah-agent-badge"><SessionAgentLabel adapter={session.adapter} /></span></header>
        <button className="ah-card-content" onClick={preview} aria-label={`${t("ahPreviewAction")}: ${title}`}>
          <h4 title={title}>{title}</h4><TokenBadge usage={session.usage} t={t} /><span className="ah-excerpt-label">{t("ahLatestMessage")}</span><p><SessionSnippet content={session.lastUserExcerpt} fallback={t("ahNoMessage")} /></p>
        </button>
        <footer className="ah-card-footer"><span className="ah-card-time"><ClockCounterClockwise aria-hidden="true" /><Stamp value={session.updatedAt} t={t} /></span><div className="ah-card-actions"><Button variant="quiet" onClick={preview}><ChatCircleText aria-hidden="true" />{t("ahPreviewAction")}</Button><ResumeButton session={session} t={t} onError={setError} /></div></footer>
      </article>;
    })}</div>
  </section>;
}
