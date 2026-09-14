import { Dialog } from "@base-ui/react/dialog";
import { ArrowClockwise, ArrowSquareOut, CalendarBlank, CaretLeft, CaretRight, Check, CircleNotch, Code, Copy, Footprints, FolderOpen, GitCommit, MagnifyingGlass, Robot, TerminalWindow, Wrench, X } from "@phosphor-icons/react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { motion, useReducedMotion } from "framer-motion";
import { memo, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
import type { MessageKey } from "../i18n";
import { api } from "../lib/api";
import { localDate, offsetDate, parseLocalDate, timelineRows } from "../lib/footprints";
import { LanguageGlyph } from "../lib/project-identity";
import type { ActivityHistoryItem, ProjectIcon } from "../types";
import { SearchPicker } from "./ui/search-picker";
import { Button } from "./ui/button";
import { useFootprints } from "./footprints/useFootprints";

type Props = {
  open: boolean; onOpenChange: (open: boolean) => void; t: (key: MessageKey) => string;
  notify: (tone: "success" | "error" | "info" | "warning", title: string, detail?: string) => void;
  onJumpToProject: (id: string) => void; onOpenTaskRun: (id: string) => void;
};
const categories: { id: string; key: MessageKey }[] = [
  { id: "", key: "filterCategoryAll" }, { id: "git", key: "filterCategoryGit" }, { id: "task", key: "filterCategoryTask" },
  { id: "tool", key: "filterCategoryTool" }, { id: "open", key: "filterCategoryOpen" }, { id: "maintenance", key: "filterCategoryMaintenance" },
];
function statusKey(status: string): MessageKey {
  return (status === "running" || status === "starting") ? "fpRunning" : status === "succeeded" ? "fpSucceeded" : status === "failed" ? "fpFailed" : "fpStopped";
}
function EventGlyph({ item }: { item: ActivityHistoryItem }) {
  const Icon = item.source === "git_history" ? GitCommit : item.category === "task" ? TerminalWindow : item.kind === "agent" ? Robot : item.kind === "ide" ? Code : item.kind === "terminal" ? TerminalWindow : item.kind === "explorer" ? FolderOpen : item.category === "open" ? ArrowSquareOut : Wrench;
  return <span className="fp-glyph" aria-hidden="true"><Icon weight="regular" /><Icon weight="duotone" /></span>;
}
/** Project identity mark: the stored thumbnail when one exists, otherwise the same language glyph the project list uses. */
function ProjectMark({ icon }: { icon?: ProjectIcon }) {
  return <span className="fp-project-mark" aria-hidden="true">
    {icon?.dataUrl ? <img src={icon.dataUrl} alt="" /> : <LanguageGlyph language={icon?.source ?? "generic"} />}
  </span>;
}
const EventRow = memo(function EventRow({ item, selected, time, t, icon, onSelect }: {
  item: ActivityHistoryItem; selected: boolean; time: string; t: Props["t"]; icon?: ProjectIcon; onSelect: (id: string) => void;
}) {
  return <button type="button" id={`fp-row-${item.id}`} role="option" aria-selected={selected} tabIndex={-1}
    className={`fp-event${selected ? " is-selected" : ""}`} onClick={() => onSelect(item.id)}>
    <time>{time}</time><EventGlyph item={item} />
    <span className="fp-event-copy"><span className="fp-event-title">{item.title}</span><span className="fp-event-meta">
      <span className="fp-project"><ProjectMark icon={icon} /><span className="fp-project-name" title={item.projectName}>{item.projectName}</span></span><i />
      <span>{t(item.source === "git_history" ? "gitHistorySource" : "repoatlasSource")}</span>
    </span></span>
    {item.commitShortSha && <code className="fp-sha">{item.commitShortSha}</code>}
    {item.taskStatus && <span className={`fp-status status-${item.taskStatus}`}>{t(statusKey(item.taskStatus))}</span>}
    <ArrowSquareOut className="fp-row-arrow" aria-hidden="true" />
  </button>;
});

export function FootprintsDialog(props: Props) {
  const { open, onOpenChange, t, notify, onJumpToProject, onOpenTaskRun } = props;
  const f = useFootprints(open);
  const prefersReduced = useReducedMotion();
  const [hidden, setHidden] = useState(document.hidden);
  const reduced = prefersReduced || hidden;
  const [copied, setCopied] = useState(false);
  const [icons, setIcons] = useState<Record<string, ProjectIcon>>({});
  const iconCache = useRef(new Map<string, ProjectIcon>());
  const scroll = useRef<HTMLDivElement>(null);
  const previousScroll = useRef(0);
  const lastDayNavigation = useRef(0);
  const locale = t("footprints") === "足迹" ? "zh-CN" : "en-US";
  const timeFormat = useMemo(() => new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit", hour12: false }), [locale]);
  const hourFormat = useMemo(() => new Intl.DateTimeFormat(locale, { hour: "2-digit", hour12: false }), [locale]);
  const fullFormat = useMemo(() => new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "medium" }), [locale]);
  const rows = useMemo(() => timelineRows(f.page.items, hourFormat), [f.page.items, hourFormat]);
  const virtual = useVirtualizer({ count: rows.length, getScrollElement: () => scroll.current, estimateSize: i => rows[i].type === "hour" ? 32 : 68, overscan: 8, getItemKey: i => rows[i].id, enabled: open });
  const virtualRows = virtual.getVirtualItems();
  const selectedIndex = rows.findIndex(row => row.type === "item" && row.id === f.selectedId);
  const selectedVisible = virtualRows.some(row => row.index === selectedIndex);
  const selectedSummary = f.summary?.days.find(day => day.date === f.date);
  const selectedDayIndex = f.summary?.days.findIndex(day => day.date === f.date) ?? -1;
  const dayIndex = f.days.findIndex(day => day.date === f.date);
  const maxCount = Math.max(1, ...f.summary?.days.map(day => day.totalCount) ?? []);
  const running = f.refresh?.state === "running";
  const selectedEntry = f.page.items.find(item => item.id === f.selectedId);
  const selected = selectedEntry ? (f.detail?.id === selectedEntry.id ? f.detail : selectedEntry) : undefined;
  const projectIds = useMemo(() => {
    const ids = new Set<string>();
    for (const entry of virtualRows) { const row = rows[entry.index]; if (row?.type === "item") ids.add(row.item.projectId); }
    for (const project of (f.summary?.projects ?? []).slice(0, 96)) ids.add(project.id);
    if (selected?.projectId) ids.add(selected.projectId);
    return [...ids];
  }, [virtualRows, rows, f.summary?.projects, selected?.projectId]);
  const wantedIds = useRef(projectIds);
  wantedIds.current = projectIds;
  const iconKey = projectIds.join("|");
  useEffect(() => {
    if (!open || !iconKey) return;
    const merge = (list: ProjectIcon[]) => setIcons(current => {
      let next = current; let changed = false;
      for (const icon of list) if (next[icon.projectId] !== icon) { if (!changed) { next = { ...current }; changed = true; } next[icon.projectId] = icon; }
      return changed ? next : current;
    });
    const wanted = wantedIds.current;
    merge(wanted.flatMap(id => { const icon = iconCache.current.get(id); return icon ? [icon] : []; }));
    const missing = wanted.filter(id => !iconCache.current.has(id));
    if (!missing.length) return;
    let stopped = false;
    const batches: string[][] = [];
    for (let index = 0; index < missing.length; index += 48) batches.push(missing.slice(index, index + 48));
    void Promise.all(batches.map(batch => api.readProjectIcons(batch).catch(() => [] as ProjectIcon[]))).then(results => {
      if (stopped) return;
      const loaded = results.flat();
      for (const icon of loaded) iconCache.current.set(icon.projectId, icon);
      merge(loaded);
    });
    return () => { stopped = true; };
  }, [open, iconKey]);
  useEffect(() => {
    if (!open) return;
    const update = () => setHidden(document.hidden); update();
    document.addEventListener("visibilitychange", update);
    return () => document.removeEventListener("visibilitychange", update);
  }, [open]);
  useEffect(() => { if (open) virtual.measure(); }, [open]);
  useLayoutEffect(() => {
    const top = f.landing.current === "end" ? Math.max(0, virtual.getTotalSize() - (scroll.current?.clientHeight ?? 0)) : 0;
    scroll.current?.scrollTo({ top }); previousScroll.current = top;
    f.landing.current = "start";
  }, [f.key]);
  const navigateAtEdge = (direction: -1 | 1) => {
    const element = scroll.current;
    if (!element || f.loading || Date.now() - lastDayNavigation.current < 600) return;
    const atEdge = direction === 1 ? element.scrollTop <= 1 : element.scrollTop + element.clientHeight >= element.scrollHeight - 1;
    if (!atEdge) return;
    if (direction === -1 && f.page.nextCursor) {
      if (f.page.pages >= 10) void f.nextWindow().then(() => scroll.current?.scrollTo({ top: 0 }));
      else void f.loadMore();
      return;
    }
    lastDayNavigation.current = Date.now();
    void f.loadAdjacentDay(direction);
  };
  useEffect(() => { setCopied(false); }, [f.selectedId]);
  useEffect(() => {
    if (!open || f.error || f.loading || f.page.pages >= 10) return;
    const last = virtualRows[virtualRows.length - 1];
    if (last && last.index >= rows.length - 8) void f.loadMore();
  }, [open, virtualRows[virtualRows.length - 1]?.index, rows.length, f.page.nextCursor, f.loading, f.error]);
  const onListKey = (event: KeyboardEvent<HTMLDivElement>) => {
    if (!["ArrowDown", "ArrowUp", "Home", "End", "Enter"].includes(event.key)) return;
    event.preventDefault();
    if (event.key === "Enter") { if (selected) { onOpenChange(false); onJumpToProject(selected.projectId); } return; }
    const indices = rows.flatMap((row, index) => row.type === "item" ? [index] : []);
    let index = event.key === "Home" ? indices[0] : event.key === "End" ? indices[indices.length - 1] : event.key === "ArrowDown" ? indices.find(i => i > selectedIndex) : [...indices].reverse().find(i => i < selectedIndex);
    if (index === undefined) return;
    const row = rows[index]; if (row.type !== "item") return;
    f.setSelectedId(row.id); virtual.scrollToIndex(index, { align: "auto" });
  };
  const jump = () => { if (selected) { onOpenChange(false); onJumpToProject(selected.projectId); } };
  const copy = async () => {
    if (!selected?.commitSha) return;
    try { await navigator.clipboard.writeText(selected.commitSha); setCopied(true); notify("success", t("shaCopied")); }
    catch (error) { notify("error", t("fpCopyFailed"), String(error)); }
  };
  const latest = f.summary?.latestAt ? localDate(new Date(f.summary.latestAt * 1000)) : undefined;
  const coverage = f.summary?.coverage.filter(c => !c.error && c.startAt <= f.days[0].startAt && c.endAt >= f.days[29].endAt);
  return <Dialog.Root modal={false} open={open} onOpenChange={onOpenChange}>
    <Dialog.Portal><Dialog.Backdrop className="fp-backdrop" />
      <Dialog.Popup className={`fp-workspace${hidden ? " fp-paused" : ""}`} aria-labelledby="fp-title">
        <header className="fp-header">
          <div className="fp-brand"><span className="fp-brand-icon"><Footprints weight="duotone" /></span><div>
            <Dialog.Title id="fp-title">{t("footprints")}<span>{t("fpSubtitle")}</span></Dialog.Title>
            <Dialog.Description>{t("fpRefreshHint")}</Dialog.Description>
          </div></div>
          <div className="fp-sync" role="status" aria-live="polite">
            {running ? <><svg className="fp-progress" viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9" /><circle cx="12" cy="12" r="9" pathLength="100" strokeDasharray={`${100 * f.refresh!.completed / Math.max(1, f.refresh!.total)} 100`} /></svg><span>{t("fpUpdating")}<small>{f.refresh!.completed} / {f.refresh!.total}</small></span></>
              : <><span className="fp-sync-dot" /><span>{t(f.refresh?.state === "partial" ? "fpPartial" : f.refresh?.state === "canceled" ? "fpCanceled" : f.refresh?.state === "completed" && f.refresh.completed > 0 ? "fpCompleted" : "fpCached")}<small>{f.summary?.coverage.length ? fullFormat.format(new Date(Math.max(...f.summary.coverage.map(c => c.checkedAt)) * 1000)) : t("fpUnknown")}</small></span></>}
          </div>
          {running ? <Button variant="quiet" size="sm" onClick={() => void f.cancelRefresh()}>{t("fpCancel")}</Button> : <Button variant="quiet" size="sm" loading={f.starting} onClick={() => void f.startRefresh()} title={t("refreshGitHistory")}><ArrowClockwise />{t("refreshGitHistory")}</Button>}
          <Dialog.Close render={<Button variant="quiet" size="icon" aria-label={t("close")} title={t("close")}><X /></Button>} />
        </header>
        <section className="fp-navigation" aria-label={t("fpDate")}>
          <div className="fp-date-identity"><div className="fp-date-caption"><CalendarBlank />{new Intl.DateTimeFormat(locale, { weekday: "long" }).format(parseLocalDate(f.date))}</div>
            <h2 key={f.date}>{f.date.slice(5).replace("-", ".")}<span>{f.date.slice(0, 4)}</span></h2>
            <p>{selectedSummary ? <><strong>{selectedSummary.totalCount}</strong> {t("fpRecords")}<i />{f.summary?.projectCounts[selectedDayIndex] ?? 0} {t("fpProjects")}</> : "—"}</p>
          </div>
          <div className="fp-calendar">
            <div className="fp-calendar-heading"><span>{f.days[0].date} — {f.calendarEnd}</span><div>
              <button type="button" title={t("fpPrevious")} aria-label={t("fpPrevious")} onClick={() => f.setCalendarEnd(offsetDate(f.calendarEnd, -30))}><CaretLeft /></button>
              <button type="button" onClick={() => f.selectDate(localDate())}>{t("fpToday")}</button>
              <button type="button" title={t("fpNext")} aria-label={t("fpNext")} onClick={() => f.setCalendarEnd(offsetDate(f.calendarEnd, 30))}><CaretRight /></button>
              <input aria-label={t("fpDate")} type="date" value={f.date} onChange={e => { if (e.target.value) f.selectDate(e.target.value); }} />
            </div></div>
            <div className="fp-track" role="tablist" aria-label={t("fpDate")}>
              {dayIndex >= 0 && <motion.div className="fp-date-cursor" aria-hidden="true" initial={false} animate={{ x: `${dayIndex * 100}%` }} transition={{ duration: reduced ? 0 : .18, ease: "easeOut" }} />}
              {f.days.map((day, index) => {
                const count = f.summary?.days.find(value => value.date === day.date)?.totalCount ?? 0;
                return <button key={day.date} role="tab" aria-selected={day.date === f.date} tabIndex={day.date === f.date || (dayIndex < 0 && index === 29) ? 0 : -1}
                  title={`${day.date} · ${count} ${t("fpRecords")}`} aria-label={`${day.date}, ${count} ${t("fpRecords")}`} className={day.date === f.date ? "is-active" : ""}
                  onClick={() => f.selectDate(day.date)} onKeyDown={event => {
                    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
                    event.preventDefault(); const next = Math.max(0, Math.min(29, index + (event.key === "ArrowRight" ? 1 : -1)));
                    f.selectDate(f.days[next].date); (event.currentTarget.parentElement?.querySelectorAll<HTMLButtonElement>("button")[next])?.focus();
                  }}>
                  <span className="fp-bar-space"><span className="fp-bar" style={{ height: `${count / maxCount * 100}%` }} /></span><span className="fp-tick">{day.date.slice(8)}</span>
                </button>;
              })}
            </div>
          </div>
        </section>
        <div className="fp-toolbar">
          <label className="fp-search"><MagnifyingGlass aria-hidden="true" /><input aria-label={t("fpSearch")} placeholder={t("fpSearch")} value={f.search} onChange={e => f.setSearch(e.target.value)} />
            {f.search && <button aria-label={t("fpClear")} title={t("fpClear")} onClick={() => f.setSearch("")}><X /></button>}</label>
          <SearchPicker items={f.summary?.projects ?? []} value={f.project} onChange={f.setProject} allLabel={t("fpAllProjects")} searchLabel={t("fpProjectSearch")} emptyLabel={t("fpNoProjects")} renderMark={id => <ProjectMark icon={icons[id]} />} />
          <div className="fp-categories" aria-label={t("filterCategoryAll")}>{categories.map(cat => <button key={cat.id} aria-pressed={f.category === cat.id} onClick={() => f.setCategory(cat.id)}>
            {f.category === cat.id && <motion.span layoutId="fp-category-active" className="fp-category-active" transition={{ duration: reduced ? 0 : .16 }} />}
            <span>{t(cat.key)}</span></button>)}</div>
        </div>
        {(f.hasUpdates || f.error || f.summaryError || f.refreshError) && <div className="fp-notice" role="status">
          <span>{f.error || f.summaryError || f.refreshError || t("fpNew")}</span><Button size="sm" variant="quiet" onClick={() => f.refreshError ? void f.startRefresh() : f.reload()}>{t(f.hasUpdates ? "fpNew" : "fpRetry")}</Button>
        </div>}
        <div className="fp-body">
          <section className="fp-feed">
            <div className="fp-feed-heading"><span>{t("fpTimeline")}</span><span>{f.loading ? t("loading") : `${f.page.items.length} ${t("fpRecords")}`}</span></div>
            <div ref={scroll} className="fp-scroll" role="listbox" aria-label={t("fpTimeline")} aria-busy={f.loading} tabIndex={0} aria-activedescendant={selectedVisible && f.selectedId ? `fp-row-${f.selectedId}` : undefined}
              onScroll={event => { const top = event.currentTarget.scrollTop; const prior = previousScroll.current; previousScroll.current = top; if (top !== prior) navigateAtEdge(top > prior ? -1 : 1); }}
              onWheel={event => { if (event.deltaY) navigateAtEdge(event.deltaY > 0 ? -1 : 1); }}
              onKeyDown={event => {
                if (["PageDown", "PageUp"].includes(event.key)) navigateAtEdge(event.key === "PageDown" ? -1 : 1);
                else if (event.key === "ArrowDown" && selectedIndex === rows.length - 1) navigateAtEdge(-1);
                else if (event.key === "ArrowUp" && selectedIndex <= 1) navigateAtEdge(1);
                onListKey(event);
              }}>
              {!f.page.items.length ? <div className="fp-empty">{f.loading ? <CircleNotch className="fp-spinner" /> : <Footprints weight="thin" />}<h3>{t(f.loading ? "loading" : f.search || f.category || f.project ? "fpNoMatch" : "noFootprintsForDate")}</h3><p>{t("fpEmptyHint")}</p>
                {!f.loading && latest && latest !== f.date && <Button variant="quiet" onClick={() => f.selectDate(latest)}>{t("fpLatest")}<ArrowSquareOut /></Button>}
              </div> : <div className="fp-virtual" style={{ height: virtual.getTotalSize() }}>{virtualRows.map(v => {
                const row = rows[v.index];
                return <div key={row.id} className="fp-virtual-row" style={{ height: v.size, transform: `translateY(${v.start}px)` }}>
                  {row.type === "hour" ? <div className="fp-hour" aria-hidden="true"><span>{row.label}</span><i /><span /></div> : <EventRow item={row.item} selected={row.id === f.selectedId} onSelect={f.setSelectedId} time={timeFormat.format(new Date(row.item.occurredAt))} t={t} icon={icons[row.item.projectId]} />}
                </div>;
              })}</div>}
              {f.page.nextCursor && <div className="fp-more"><Button size="sm" variant="quiet" loading={f.loading} onClick={() => { if (f.page.pages >= 10) { void f.nextWindow().then(() => scroll.current?.scrollTo({ top: 0 })); } else void f.loadMore(); }}>{t(f.page.pages >= 10 ? "fpNextWindow" : "fpLoadMore")}</Button></div>}
            </div>
            <footer className="fp-feed-footer"><span title={t("fpCoverage")}>{t("fpCoverage")} · {coverage?.length ? `${coverage.length} ${t("fpProjects")}` : t("fpUnknown")}</span>
              <Button size="sm" variant="quiet" disabled={!f.project || running || f.starting} title={t("fpHistoryHint")} onClick={() => void f.startRefresh(false, true)}><ArrowClockwise />{t("fpHistory")}</Button></footer>
          </section>
          <aside className="fp-inspector" aria-label={t("fpInspect")}>
            {selected ? <div className="fp-detail" key={selected.id}>
              <div className="fp-detail-heading"><EventGlyph item={selected} /><span>{t(categories.find(c => c.id === selected.category)?.key ?? "fpInspect")}</span><span className="fp-detail-index">{String(f.page.items.findIndex(i => i.id === selected.id) + 1).padStart(2, "0")}</span></div>
              <time>{fullFormat.format(new Date(selected.occurredAt))}</time><h3>{selected.title}</h3>
              <dl><dt>{t("projects")}</dt><dd className="fp-project-value"><ProjectMark icon={icons[selected.projectId]} />{selected.projectName}</dd><dt>{t("path")}</dt><dd className="fp-path">{selected.canonicalPath}</dd>
                <dt>{t("fpSource")}</dt><dd>{t(selected.source === "git_history" ? "gitHistorySource" : "repoatlasSource")}</dd>
                {selected.authorName && <><dt>{t("fpAuthor")}</dt><dd>{selected.authorName}</dd></>}
                {selected.taskStatus && <><dt>{t("status")}</dt><dd className={`fp-status status-${selected.taskStatus}`}>{t(statusKey(selected.taskStatus))}{selected.taskDurationMs != null && ` · ${(selected.taskDurationMs / 1000).toFixed(1)}s`}</dd></>}
              </dl>
              {selected.commitSha && <div className="fp-commit"><span>{t("fpCommit")}</span><code>{selected.commitSha}</code><button onClick={() => void copy()} aria-label={t("copyCommitSha")} title={t("copyCommitSha")}>{copied ? <Check /> : <Copy />}{t(copied ? "fpCopied" : "copyCommitSha")}</button></div>}
              {f.detailError ? <p className="fp-detail-error">{f.detailError}<button onClick={f.reload}>{t("fpRetry")}</button></p> : !f.detail ? <p className="fp-detail-loading">{t("fpDetailsLoading")}</p> : f.detail.detail && <pre>{f.detail.detail}</pre>}
              <div className="fp-detail-actions"><Button variant="primary" onClick={jump}><ArrowSquareOut />{t("jumpToProject")}</Button>{selected.runId && <Button variant="quiet" onClick={() => { onOpenChange(false); onOpenTaskRun(selected.runId!); }}><TerminalWindow />{t("jumpToRunOutput")}</Button>}</div>
            </div> : <div className="fp-empty fp-inspector-empty"><GitCommit weight="thin" /><p>{t("fpChoose")}</p></div>}
          </aside>
        </div>
      </Dialog.Popup>
    </Dialog.Portal>
  </Dialog.Root>;
}
