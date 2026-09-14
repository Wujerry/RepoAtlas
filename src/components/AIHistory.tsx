import { Dialog } from "@base-ui/react/dialog";
import { open as chooseDirectory } from "@tauri-apps/plugin-dialog";
import { ArrowClockwise, ArrowRight, Copy, FolderOpen, ChatCircleText, ClockCounterClockwise, MagnifyingGlass, Play, SlidersHorizontal, X } from "@phosphor-icons/react";
import { useCallback, useEffect, useId, useRef, useState } from "react";
import type { MessageKey } from "../i18n";
import type { ProjectIcon, ProjectSummary } from "../types";
import { api } from "../lib/api";
import { openSessionHistory, sessionAgentName, sessionAgents, sessionApi, sessionTime } from "../lib/sessions";
import type { AgentSession, SessionMessage, SessionRefreshJob, SessionResumeSpec, SessionSearchHit, SessionSource } from "../lib/sessions";
import { Button } from "./ui/button";
import { ConfirmDialog } from "./ui/confirm";
import { SearchPicker } from "./ui/search-picker";

import { sessionDisplayTitle, sessionExcerpt } from "../lib/session-presentation";

type T = (key: MessageKey) => string;
function reason(key: string | null, t: T) {
  if (key === "session_cli_unavailable") return t("ahCliUnavailable");
  if (key === "session_cli_probe_timeout") return t("ahCliProbeTimeout");
  if (key === "session_source_layout_unknown") return t("ahSourceLayout");
  return key === "session_cwd_missing" ? t("ahCwdMissing") : key === "session_agent_missing" ? t("ahAgentMissing") : key === "session_source_missing" ? t("ahSourceMissing") : key === "invalid_session_id" ? t("ahInvalidId") : key ?? "";
}
function Stamp({ value, t }: { value: string; t: T }) {
  return <time dateTime={value} title={new Date(value).toLocaleString()}>{sessionTime(value, t("ahUser") === "用户")}</time>;
}
function Highlight({ text, query }: { text: string; query: string }) {
  const terms = query.trim().split(/\s+/).filter(Boolean).map(s => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
  if (!terms.length) return <>{text}</>;
  const pattern = new RegExp(`(${terms.join("|")})`, "gi");
  return <>{text.split(pattern).map((part, i) => i % 2 ? <mark key={i}>{part}</mark> : part)}</>;
}
function ResumeButton({ session, t, onError }: { session: AgentSession; t: T; onError?: (error: string) => void }) {
  const targetName = useId();
  const [open, setOpen] = useState(false); const [busy, setBusy] = useState(false);
  const [spec, setSpec] = useState<SessionResumeSpec>(); const [target, setTarget] = useState<"cli" | "app">("cli");
  const [error, setError] = useState(""); const [notice, setNotice] = useState(""); const guard = useRef(false);
  const command = target === "app" ? `codex://threads/${session.externalId}` : spec?.command ?? "";
  return <span className="ah-resume" title={reason(session.resumeReason, t)}>
    <Button variant="primary" disabled={!session.capabilities.directResume || busy} loading={busy} onClick={async () => {
      if (guard.current) return; guard.current = true; setBusy(true); setError(""); setNotice(""); setSpec(undefined); setTarget("cli"); setOpen(true);
      try { setSpec(await sessionApi.resumeSpec(session.id)); } catch (e) { setError(reason(String(e), t)); onError?.(reason(String(e), t)); }
      finally { guard.current = false; setBusy(false); }
    }}><Play weight="fill" aria-hidden="true" />{t("ahResume")}</Button>
    <Dialog.Root open={open} onOpenChange={value => { if (!busy) setOpen(value); }}>
      <Dialog.Portal><Dialog.Backdrop className="dialog-backdrop ah-launch-backdrop" /><Dialog.Popup className="dialog-popup ah-launch-dialog">
        <Dialog.Title className="dialog-title">{t("ahResume")}</Dialog.Title>
        <Dialog.Description className="dialog-description">{t("ahLaunchHint")}</Dialog.Description>
        <strong>{sessionDisplayTitle(session.title, t("ahUntitled"))}</strong><p>{sessionAgentName(session.adapter)} · <code>{session.externalId}</code></p>
        {spec && <><fieldset disabled={busy} className="ah-launch-targets"><legend>{t("ahLaunchTarget")}</legend>
          <label><input type="radio" name={targetName} checked={target === "cli"} onChange={() => setTarget("cli")} /> CLI / Terminal</label>
          {spec.app && <label><input type="radio" name={targetName} disabled={!spec.app.canResume} checked={target === "app"} onChange={() => setTarget("app")} /> {spec.app.name} / App</label>}
        </fieldset>{spec.app && !spec.app.canResume && <p className="ah-launch-note">{t("ahAppUnsupported")}</p>}
        <small>{t("ahLaunchCwd")}</small><code className="ah-launch-path">{spec.cwd}</code>
        <small>{t("ahLaunchCommand")}</small><pre className="ah-launch-command">{command}</pre>
        <Button variant="quiet" onClick={() => void navigator.clipboard.writeText(command).then(() => setNotice(t("ahCopied"))).catch(e => setError(String(e)))}><Copy />{t("ahCopy")}</Button></>}
        {(busy || error || notice) && <p role="status">{error || (busy ? t("ahPending") : notice)}</p>}
        <div className="dialog-actions"><Dialog.Close render={<Button disabled={busy}>{t("close")}</Button>} />
          <Button variant="primary" disabled={busy || !spec} loading={busy} onClick={async () => {
            if (guard.current) return; guard.current = true; setBusy(true); setError(""); setNotice("");
            try { await sessionApi.resume(session.id, target); setNotice(t("ahStarted")); } catch (e) { setError(reason(String(e), t)); }
            finally { guard.current = false; setBusy(false); }
          }}><Play weight="fill" />{t("ahLaunchNow")}</Button></div>
      </Dialog.Popup></Dialog.Portal>
    </Dialog.Root>
  </span>;
}

/** Mount independently of the project tree so opening/closing preserves its navigation. */
export function AIHistory({ t, onVisibilityChange }: { t: T; onVisibilityChange?: (open: boolean) => void }) {
  const [isOpen, setOpen] = useState(false); const [sourcesOpen, setSourcesOpen] = useState(false);
  const [query, setQuery] = useState(""); const [projectId, setProjectId] = useState(""); const [adapter, setAdapter] = useState("");
  const [after, setAfter] = useState(""); const [before, setBefore] = useState(""); const [archived, setArchived] = useState(false);
  const [offset, setOffset] = useState(0); const [hits, setHits] = useState<SessionSearchHit[]>([]); const [total, setTotal] = useState(0);
  const [selected, setSelected] = useState<AgentSession>(); const [messages, setMessages] = useState<SessionMessage[]>([]);
  const [messageOffset, setMessageOffset] = useState(0); const [messageBusy, setMessageBusy] = useState(false);
  const [sources, setSources] = useState<SessionSource[]>([]); const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [busy, setBusy] = useState(false); const [error, setError] = useState(""); const [notice, setNotice] = useState("");
  const [revision, setRevision] = useState(0); const [job, setJob] = useState<SessionRefreshJob>();
  useEffect(() => { onVisibilityChange?.(isOpen); }, [isOpen, onVisibilityChange]);
  const [bulkSources, setBulkSources] = useState<SessionSource[]>([]);
  const [pendingSource, setPendingSource] = useState<SessionSource>(); const [sourceBusy, setSourceBusy] = useState(false);
  const [sourceError, setSourceError] = useState("");
  const [customAdapter, setCustomAdapter] = useState("claude"); const [customPath, setCustomPath] = useState("");
  const [rebuild, setRebuild] = useState(false); const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [linking, setLinking] = useState(false);
  const input = useRef<HTMLInputElement>(null); const resultPane = useRef<HTMLDivElement>(null); const transcript = useRef<HTMLDivElement>(null);
  const sequence = useRef(0); const messageSequence = useRef(0); const opener = useRef<HTMLElement | null>(null);
  const resultScroll = useRef(0); const messageScroll = useRef(new Map<string, number>()); const jump = useRef<number | null>(null);
  const messagePages = useRef(new Map<string, number>());
  const poll = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const openRef = useRef(isOpen); openRef.current = isOpen;

  const reloadSources = useCallback(async () => { const items = await sessionApi.sources(); setSources(items); return items; }, []);
  useEffect(() => {
    const open = (event: Event) => {
      const detail = (event as CustomEvent<{ projectId?: string; sources?: boolean; session?: AgentSession }>).detail;
      opener.current = document.activeElement as HTMLElement;
      if (detail?.projectId !== undefined) { setProjectId(detail.projectId); setOffset(0); }
      if (detail?.session) { setSelected(detail.session); setMessageOffset(0); setMessages([]); jump.current = null; }
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
        else { setRevision(v => v + 1); await reloadSources(); window.dispatchEvent(new Event("repoatlas:sessions-updated")); }
      } catch (e) { if (active) setError(String(e)); }
    };
    void sessionApi.refresh().then(() => { if (active) return tick(); }).catch(e => active && setError(String(e)));
    return () => { active = false; clearTimeout(poll.current); };
  }, [isOpen, reloadSources]);

  useEffect(() => {
    if (!isOpen) return;
    const ticket = ++sequence.current; setBusy(true);
    const timer = setTimeout(() => {
      const until = before ? new Date(`${before}T00:00:00`) : undefined;
      until?.setDate(until.getDate() + 1);
      void sessionApi.search({ query, projectId: projectId || undefined, adapter: adapter || undefined, archived, offset, limit: 50,
        after: after ? new Date(`${after}T00:00:00`).toISOString() : undefined, before: until?.toISOString() }).then(result => {
        if (ticket !== sequence.current) return; setHits(result.items); setTotal(result.total); setError("");
        requestAnimationFrame(() => { if (resultPane.current) resultPane.current.scrollTop = resultScroll.current; });
      }).catch(e => ticket === sequence.current && setError(String(e))).finally(() => ticket === sequence.current && setBusy(false));
    }, 200);
    return () => { clearTimeout(timer); sequence.current++; };
  }, [isOpen, query, projectId, adapter, after, before, archived, offset, revision]);

  useEffect(() => {
    if (!isOpen || !selected) return;
    const ticket = ++messageSequence.current; setMessageBusy(true);
    void sessionApi.messages(selected.id, messageOffset).then(page => {
      if (ticket !== messageSequence.current) return; setMessages(page.items);
      requestAnimationFrame(() => {
        if (jump.current !== null) { document.getElementById(`ah-message-${jump.current}`)?.scrollIntoView({ block: "center" }); jump.current = null; }
        else if (transcript.current) transcript.current.scrollTop = messageScroll.current.get(`${selected.id}:${messageOffset}`) ?? 0;
      });
    }).catch(e => { if (ticket === messageSequence.current) { setMessages([]); setError(reason(String(e), t)); } }).finally(() => ticket === messageSequence.current && setMessageBusy(false));
    return () => { messageSequence.current++; };
  }, [isOpen, selected?.id, selected?.revision, messageOffset, revision]);

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
  const emptyTitle = !enabled.length ? "ahNoSources" : enabled.every(s => !s.lastScannedAt) ? "ahUnindexed" : "ahEmpty";
  return <Dialog.Root modal={false} open={isOpen} onOpenChange={setOpen}>
    <Dialog.Portal><Dialog.Backdrop className="ah-backdrop" />
      <Dialog.Popup className="ah-dialog" initialFocus={input} finalFocus={() => opener.current} onKeyDown={e => {
        if ((e.key === "/" || ((e.ctrlKey || e.metaKey) && e.key === "f")) && !(e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement)) { e.preventDefault(); input.current?.focus(); }
      }}>
        <header className="ah-header"><div><Dialog.Title>{t("ahTitle")}</Dialog.Title><Dialog.Description>{t("ahCached")}</Dialog.Description></div>
          <Button variant="quiet" onClick={() => setSourcesOpen(v => !v)} aria-pressed={sourcesOpen}><SlidersHorizontal />{t("ahSources")}</Button>
          <Button variant="quiet" disabled={job?.running || !enabled.length} onClick={() => void refresh()}><ArrowClockwise />{t("refresh")}</Button>
          <Dialog.Close render={<Button variant="quiet" size="icon" aria-label={t("close")}><X /></Button>} />
        </header>
        {(error || notice || job?.running) && <div className="ah-status" role="status">
          {error || (job?.running ? `${t("ahUpdating")} · ${job.processed}` : notice)}
          {job?.running && <Button variant="quiet" onClick={() => void sessionApi.cancel().catch(e => setError(String(e)))}>{t("cancel")}</Button>}
          {error && <Button variant="quiet" onClick={() => { setRevision(v => v + 1); void reloadSources().catch(e => setError(String(e))); }}>{t("retry")}</Button>}
        </div>}
        {sourcesOpen ? <div className="ah-sources">
          <div className="ah-source-heading"><h2>{t("ahSources")}</h2><Button variant="primary" disabled={sourceBusy || !sources.some(s => !s.enabled)} onClick={() => { setSourceError(""); setBulkSources(sources.filter(s => !s.enabled)); }}>{t("ahAuthorizeAll")} ({sources.filter(s => !s.enabled).length})</Button></div><p>{t("ahSourcesHint")}</p>
          {sources.map(source => <div className="ah-source-row" key={`${source.adapter}:${source.path}`}>
            <div><strong>{sessionAgentName(source.adapter)}</strong><code title={source.path}>{source.path}</code>
              {source.lastScannedAt && <small>{t("ahCached")} · <Stamp value={source.lastScannedAt} t={t} /></small>}
              {source.lastError && <p role="status">{t("ahPartial")} <code>{source.lastError}</code></p>}
            </div><Button variant="quiet" onClick={() => { setSourceError(""); setPendingSource(source); }}>{t(source.enabled ? "ahDisable" : "ahAuthorize")}</Button>
          </div>)}
          <form className="ah-source-form" onSubmit={e => { e.preventDefault(); setSourceError(""); setPendingSource({ id: "", adapter: customAdapter, path: customPath.trim(), enabled: false, lastScannedAt: null, lastError: null }); }}>
            <label>{t("ahSource")}<select value={customAdapter} onChange={e => setCustomAdapter(e.target.value)}>{Object.entries(sessionAgents).map(([id, name]) => <option key={id} value={id}>{name}</option>)}</select></label>
            <label className="ah-grow">{t("ahCustomPath")}<input required value={customPath} onChange={e => setCustomPath(e.target.value)} placeholder={t("ahCustomPath")} /></label>
            <Button variant="quiet" type="button" aria-label={t("ahCustomPath")} onClick={() => void chooseDirectory({ directory: true, multiple: false }).then(p => { if (typeof p === "string") setCustomPath(p); }).catch(e => setError(String(e)))}><FolderOpen /></Button>
            <Button type="submit" disabled={!customPath.trim()}>{t("ahAdd")}</Button>
          </form>
          <Button variant="quiet" disabled={job?.running || !enabled.length} onClick={() => setRebuild(true)}>{t("ahRebuild")}</Button>
        </div> : <>
          <div className="ah-filters"><label className="ah-search"><MagnifyingGlass aria-hidden="true" /><input ref={input} value={query} maxLength={512} placeholder={t("ahSearch")} aria-label={t("ahSearch")} onChange={e => filter(() => setQuery(e.target.value))} />{query && <button aria-label={t("clear")} onClick={() => filter(() => setQuery(""))}><X /></button>}</label>
            <SearchPicker items={projects.map(p => ({ id: p.id, name: p.displayName }))} value={projectId} onChange={id => filter(() => setProjectId(id))} allLabel={t("ahAllProjects")} searchLabel={t("ahLink")} emptyLabel={t("ahEmpty")} />
            <select aria-label={t("ahAllAgents")} value={adapter} onChange={e => filter(() => setAdapter(e.target.value))}><option value="">{t("ahAllAgents")}</option>{Object.entries(sessionAgents).map(([id, name]) => <option key={id} value={id}>{name}</option>)}</select>
            <label>{t("ahFrom")}<input type="date" value={after} max={before || undefined} onChange={e => filter(() => setAfter(e.target.value))} /></label>
            <label>{t("ahTo")}<input type="date" value={before} min={after || undefined} onChange={e => filter(() => setBefore(e.target.value))} /></label>
            <label className="ah-check"><input type="checkbox" checked={archived} onChange={e => filter(() => setArchived(e.target.checked))} />{t("ahArchived")}</label>
          </div>
          {enabled.some(s => s.lastError) && <div className="ah-status">{t("ahPartial")}<Button variant="quiet" onClick={() => setSourcesOpen(true)}>{t("ahSources")}</Button></div>}
          <div className="ah-body"><section className="ah-results"><div className="ah-result-count" aria-live="polite">{total} {t("ahResults")}{busy && <span>{t("ahPending")}</span>}</div>
            <div ref={resultPane} className="ah-result-scroll" aria-busy={busy} onScroll={e => { resultScroll.current = e.currentTarget.scrollTop; }} onKeyDown={e => {
              if (!["ArrowUp", "ArrowDown"].includes(e.key)) return;
              const buttons = Array.from(e.currentTarget.querySelectorAll<HTMLButtonElement>("button.ah-result"));
              const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
              buttons[Math.max(0, Math.min(buttons.length - 1, index + (e.key === "ArrowDown" ? 1 : -1)))]?.focus(); e.preventDefault();
            }}>
              {!hits.length && !busy && <div className="ah-empty"><MagnifyingGlass /><h3>{t(emptyTitle)}</h3>{!enabled.length && <><p>{t("ahNoSourcesHint")}</p><Button onClick={() => setSourcesOpen(true)}>{t("ahSources")}</Button></>}</div>}
              {hits.map(hit => <button className={`ah-result${selected?.id === hit.session.id ? " is-selected" : ""}`} key={hit.session.id} onClick={() => select(hit)} aria-pressed={selected?.id === hit.session.id}>
                <span className="ah-result-meta"><span>{sessionAgentName(hit.session.adapter)}</span><Stamp value={hit.session.updatedAt} t={t} /></span>
                <strong title={hit.session.title}><Highlight text={sessionDisplayTitle(hit.session.title, t("ahUntitled"))} query={query} /></strong><span className="ah-project-name">{hit.session.projectName ?? t("ahUnlinked")}</span>
                {hit.snippets.map(m => <p key={m.index}><Highlight text={m.content} query={query} /></p>)}
                {!hit.snippets.length && hit.session.lastUserExcerpt && <p>{sessionExcerpt(hit.session.lastUserExcerpt)}</p>}
              </button>)}
            </div><footer className="ah-pagination"><Button variant="quiet" disabled={offset === 0 || busy} onClick={() => { setOffset(v => Math.max(0, v - 50)); resultScroll.current = 0; }}>{t("ahPrevious")}</Button><span>{total ? `${offset + 1}–${Math.min(offset + 50, total)}` : "0"}</span><Button variant="quiet" disabled={offset + 50 >= total || busy} onClick={() => { setOffset(v => v + 50); resultScroll.current = 0; }}>{t("ahNext")}</Button></footer>
          </section><section className="ah-detail" aria-busy={messageBusy}>
            {!selected ? <div className="ah-empty"><h3>{t("ahPreview")}</h3></div> : <>
              <header className="ah-detail-header"><span>{sessionAgentName(selected.adapter)} · <Stamp value={selected.updatedAt} t={t} /></span><h2>{sessionDisplayTitle(selected.title, t("ahUntitled"))}</h2>
                <code title={selected.cwd}>{selected.cwd || t("ahCwdMissing")}</code><small>{selected.messageCount} {t("ahMessages")} · {selected.projectName ?? t("ahUnlinked")}</small>
                <div className="ah-detail-actions"><ResumeButton key={selected.id} session={selected} t={t} onError={setError} /><Button variant="quiet" disabled={!selected.capabilities.directResume} onClick={() => void sessionApi.resumeSpec(selected.id).then(spec => navigator.clipboard.writeText(spec.command)).then(() => setNotice(t("ahCopied"))).catch(e => setError(reason(String(e), t)))}><Copy />{t("ahCopy")}</Button></div>
                {selected.resumeReason && <p role="status">{reason(selected.resumeReason, t)}</p>}
                <details><summary>{t("ahLink")} · {t("ahWorkspace")}</summary><div className="ah-link-controls">
                  <SearchPicker items={projects.map(p => ({ id: p.id, name: p.displayName }))} value={selected.projectId ?? ""} allLabel={t("ahUnlinked")} searchLabel={t("ahLink")} emptyLabel={t("ahEmpty")} onChange={id => {
                    if (linking) return; setLinking(true); void sessionApi.link(selected.id, id || null).then(() => sessionApi.get(selected.id)).then(s => { setSelected(s); setRevision(v => v + 1); setNotice(t("ahSaved")); }).catch(e => setError(String(e))).finally(() => setLinking(false));
                  }} />
                  <Button variant="quiet" disabled={linking} onClick={() => void chooseDirectory({ directory: true, multiple: false }).then(async p => {
                    if (typeof p !== "string") return; setLinking(true); try { await sessionApi.link(selected.id, selected.projectId, p); setSelected(await sessionApi.get(selected.id)); setNotice(t("ahSaved")); } finally { setLinking(false); }
                  }).catch(e => setError(String(e)))}><FolderOpen />{t("ahRelocate")}</Button>
                </div><code title={selected.sourceLocator}>{selected.sourceLocator}</code><code>{selected.externalId}</code></details>
              </header>
              <div className="ah-transcript" ref={transcript} onScroll={e => messageScroll.current.set(`${selected.id}:${messageOffset}`, e.currentTarget.scrollTop)}>
                {messageBusy ? <p role="status">{t("ahPending")}</p> : messages.map(message => {
                  const key = `${selected.id}:${message.index}`; const full = expanded.has(key); const long = message.content.length > 2000;
                  return <article id={`ah-message-${message.index}`} key={key} className={`ah-message ${message.role}`}><header><strong>{t(message.role === "user" ? "ahUser" : "ahAssistant")}</strong>{message.timestamp && <Stamp value={message.timestamp} t={t} />}</header>
                    <div><Highlight text={long && !full ? message.content.slice(0, 2000) + "…" : message.content} query={query} /></div>
                    {long && <Button variant="quiet" onClick={() => setExpanded(old => { const next = new Set(old); if (full) next.delete(key); else next.add(key); return next; })}>{t(full ? "ahCollapse" : "ahExpand")}</Button>}
                  </article>;
                })}
              </div><footer className="ah-pagination"><Button variant="quiet" disabled={!messageOffset || messageBusy} onClick={() => setMessageOffset(v => Math.max(0, v - 40))}>{t("ahPrevious")}</Button><span>{selected.messageCount ? `${messageOffset + 1}–${Math.min(messageOffset + 40, selected.messageCount)}` : "0"} / {selected.messageCount}</span><Button variant="quiet" disabled={messageOffset + 40 >= selected.messageCount || messageBusy} onClick={() => setMessageOffset(v => v + 40)}>{t("ahNext")}</Button></footer>
            </>}
          </section></div>
        </>}
        <ConfirmDialog confirmVariant="primary" className="ah-confirm ah-bulk-confirm" open={bulkSources.length > 0} title={t("ahAuthorizeAll")} body={`${t("ahSourcesHint")}\n\n${bulkSources.map(s => `${sessionAgentName(s.adapter)}\n${s.path}`).join("\n\n")}\n\n${sourceError}`} confirmLabel={t("ahAuthorizeAll")} cancelLabel={t("cancel")} busy={sourceBusy} onOpenChange={value => { if (!value && !sourceBusy) setBulkSources([]); }} onConfirm={async () => {
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

export function ContinueCoding({ t, projectId, onOpenProject, compact = false }: { t: T; projectId?: string; onOpenProject?: (id: string) => void; compact?: boolean }) {
  const [icons, setIcons] = useState<Record<string, ProjectIcon>>({});
  const [sessions, setSessions] = useState<AgentSession[]>([]); const [error, setError] = useState(""); const [loaded, setLoaded] = useState(false);
  useEffect(() => {
    let active = true; let timer: ReturnType<typeof setTimeout> | undefined;
    setSessions([]); setLoaded(false); setError("");
    const load = async () => {
      try {
        const items = await sessionApi.recent(projectId);
        if (active) { setSessions(items); setError(""); setLoaded(true); }
      } catch (e) { if (active) { setError(String(e)); setLoaded(true); } }
    };
    const tick = async () => { try { const status = await sessionApi.status(); if (!active) return; if (status.running) timer = setTimeout(tick, 1000); else await load(); } catch (e) { if (active) setError(String(e)); } };
    const changed = () => { void load(); }; window.addEventListener("repoatlas:sessions-updated", changed);
    void load(); void sessionApi.refresh().then(() => { if (active) return tick(); }).catch(e => active && setError(String(e)));
    return () => { active = false; clearTimeout(timer); window.removeEventListener("repoatlas:sessions-updated", changed); };
  }, [projectId]);
  const ids = sessions.map(s => s.projectId).filter((id): id is string => !!id).join("|");
  useEffect(() => {
    let active = true;
    if (!ids) { setIcons({}); return; }
    void api.readProjectIcons(ids.split("|")).then(items => { if (active) setIcons(Object.fromEntries(items.map(icon => [icon.projectId, icon]))); }).catch(() => { if (active) setIcons({}); });
    return () => { active = false; };
  }, [ids]);
  const visible = compact ? sessions.slice(0, 3) : sessions;
  return <section className={`ah-continue${projectId ? " is-project" : ""}${compact ? " is-compact" : ""}`} aria-label={t("ahContinue")}>
    <header className="ah-continue-header"><div className="ah-continue-heading"><span className="ah-section-icon"><ClockCounterClockwise weight="duotone" aria-hidden="true" /></span><div><h2>{t("ahContinue")}</h2><p>{t("ahContinueHint")}</p></div>{loaded && sessions.length > 0 && <span className="ah-session-count">{visible.length}</span>}</div>
      <Button variant="quiet" onClick={() => openSessionHistory(projectId)}>{t("ahMore")}<ArrowRight aria-hidden="true" /></Button>
    </header>
    {!loaded && <p className="ah-continue-empty" role="status">{t("ahPending")}</p>}
    {loaded && !sessions.length && <p className="ah-continue-empty">{t("ahNoResume")}</p>}
    {error && <p role="status">{t("ahReadFailed")}: {error}</p>}
    <div className="ah-continue-items">{visible.map(session => {
      const title = sessionDisplayTitle(session.title, t("ahUntitled"));
      const excerpt = sessionExcerpt(session.lastUserExcerpt);
      const preview = () => openSessionHistory(session.projectId ?? undefined, session);
      return <article className="ah-session-card" key={session.id}>
        <header className="ah-card-header"><button className="ah-card-project" onClick={() => { if (session.projectId && onOpenProject) onOpenProject(session.projectId); else preview(); }} title={session.cwd}><span className="ah-project-symbol">{session.projectId && icons[session.projectId]?.dataUrl ? <img src={icons[session.projectId].dataUrl!} alt="" /> : <FolderOpen aria-hidden="true" />}</span><h3>{session.projectName || session.cwd}</h3></button><span className="ah-agent-badge">{sessionAgentName(session.adapter)}</span></header>
        <button className="ah-card-content" onClick={preview} aria-label={`${t("ahPreviewAction")}: ${title}`}>
          <h4 title={title}>{title}</h4><span className="ah-excerpt-label">{t("ahLatestMessage")}</span><p>{excerpt || t("ahNoMessage")}</p>
        </button>
        <footer className="ah-card-footer"><span className="ah-card-time"><ClockCounterClockwise aria-hidden="true" /><Stamp value={session.updatedAt} t={t} /></span><div className="ah-card-actions"><Button variant="quiet" onClick={preview}><ChatCircleText aria-hidden="true" />{t("ahPreviewAction")}</Button><ResumeButton session={session} t={t} onError={setError} /></div></footer>
      </article>;
    })}</div>
  </section>;
}
