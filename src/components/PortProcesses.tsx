import { ArrowClockwise, ArrowsDownUp, CaretDown, CaretUp, Info, LinkSimple, MagnifyingGlass, Square, X } from "@phosphor-icons/react";
import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../lib/api";
import type { MessageKey } from "../i18n";
import type { PortProcess, ProcessInventory, ProcessStopMode, ProcessStopPreview, ProjectSummary } from "../types";
import type { PortConflictContext } from "../lib/port-processes";
import { refreshTaskRuns } from "../lib/task-runs";
import { formatTime } from "../lib/format";
import { EmptyState } from "./ui/feedback";
import { Button } from "./ui/button";
import { ConfirmDialog } from "./ui/confirm";

type ProcessSort = "ports" | "name" | "pid" | "project";
const nameCollator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });

export function processError(error: unknown, t: (key: MessageKey) => string): string {
  const messages: Record<string, MessageKey> = {
    permission_denied: "processPermissionDenied", identity_changed: "processIdentityChanged",
    identity_unavailable: "processIdentityUnavailable", process_exited: "processExited",
    scope_changed: "processScopeChanged", confirmation_expired: "processConfirmationExpired",
    protected_process: "processProtected", graceful_unavailable: "processGracefulUnavailable",
    inspection_unavailable: "portInspectionUnavailable", stop_pending: "processStopPending",
    unsupported_platform: "processWindowsOnly",
  };
  return messages[String(error)] ? t(messages[String(error)]) : String(error);
}

export function PortProcesses({ t, context, onJump, onClose }: {
  t: (key: MessageKey) => string; context?: PortConflictContext; onJump: (id: string) => void; onClose?: () => void;
}) {
  const [updatedAt, setUpdatedAt] = useState<string>();
  const [inventory, setInventory] = useState<ProcessInventory>();
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<{ key: ProcessSort; direction: "ascending" | "descending" }>({ key: "ports", direction: "ascending" });
  const [attributionFilter, setAttributionFilter] = useState<"all" | "linked" | "unknown">("all");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const [preview, setPreview] = useState<ProcessStopPreview>();
  const [association, setAssociation] = useState<PortProcess>();
  const [projectId, setProjectId] = useState("");
  const [projectQuery, setProjectQuery] = useState("");
  const [conflictPorts, setConflictPorts] = useState<number[] | null>(null);
  const mounted = useRef(true);
  const lock = useRef(false);
  const trigger = useRef<HTMLElement | null>(null);
  const search = useRef<HTMLInputElement | null>(null);
  const returnFocus = () => trigger.current?.isConnected && !trigger.current.matches(":disabled") ? trigger.current : search.current;
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);

  const refresh = useCallback(async () => {
    const [next, library] = await Promise.all([api.listPortProcesses(), api.listProjects({ includeArchived: true })]);
    if (mounted.current) { setInventory(next); setProjects(library); setUpdatedAt(new Date().toISOString()); }
    if (context) {
      const conflicts = await api.preflightTaskPorts(context.projectId, context.taskId);
      if (mounted.current) setConflictPorts(conflicts.map(item => item.port));
    }
  }, [context]);

  const perform = useCallback(async (action: () => Promise<void>) => {
    if (lock.current) return;
    lock.current = true; setBusy(true); setError(""); setMessage("");
    try { await action(); } catch (err) { if (mounted.current) setError(processError(err, t)); }
    finally { lock.current = false; if (mounted.current) setBusy(false); }
  }, [t]);

  useEffect(() => { void perform(refresh); }, [refresh, perform]);

  async function prepare(row: PortProcess, mode: ProcessStopMode) {
    if (!row.identity) return;
    trigger.current = document.activeElement as HTMLElement;
    await perform(async () => { const plan = await api.previewProcessStop(row.identity!, mode); if (mounted.current) setPreview(plan); });
  }
  async function stop() {
    if (!preview) return;
    const plan = preview;
    await perform(async () => {
      // Dismiss a consumed plan even after failure: retry must obtain a fresh identity/scope.
      setPreview(undefined);
      const result = await api.confirmProcessStop(plan.token);
      setMessage(t(result === "signal_sent" ? "processSignalSent" : "processStopped"));
      await refresh(); await refreshTaskRuns();
    });
  }

  const scopedRows = (inventory?.processes ?? []).filter(row => !context || row.ports.some(port => context.ports.includes(port)));
  const linkedCount = scopedRows.filter(row => row.projectId).length;
  const projectNames = new Map(projects.map(project => [project.id, project.displayName]));
  const rows = scopedRows.filter(row => {
    if (attributionFilter === "linked" && !row.projectId) return false;
    if (attributionFilter === "unknown" && row.projectId) return false;
    const name = projects.find(project => project.id === row.projectId)?.displayName ?? t("processUnknown");
    return `${row.pid} ${row.name} ${row.ports.join(" ")} ${name} ${row.identity?.executable ?? ""}`.toLowerCase().includes(query.trim().toLowerCase());
  }).sort((a, b) => {
    let order = 0;
    if (sort.key === "pid") order = a.pid - b.pid;
    else if (sort.key === "ports") order = Math.min(...a.ports) - Math.min(...b.ports);
    else {
      const left = sort.key === "name" ? a.name : projectNames.get(a.projectId ?? "");
      const right = sort.key === "name" ? b.name : projectNames.get(b.projectId ?? "");
      // Missing values stay at the end in either direction.
      if (!left !== !right) return left ? -1 : 1;
      order = nameCollator.compare(left ?? "", right ?? "");
    }
    return (sort.direction === "ascending" ? order : -order) || a.pid - b.pid;
  });
  function sortHeader(key: ProcessSort, label: string) {
    const active = sort.key === key;
    const nextDirection = active && sort.direction === "ascending" ? "descending" : "ascending";
    const Icon = active ? (sort.direction === "ascending" ? CaretUp : CaretDown) : ArrowsDownUp;
    return <th scope="col" aria-sort={active ? sort.direction : undefined}>
      <Button variant="quiet" className="port-process-sort" title={`${label} · ${t(nextDirection === "ascending" ? "sortAscending" : "sortDescending")}`} onClick={() => setSort({ key, direction: nextDirection })}>
        {label}<Icon aria-hidden />
      </Button>
    </th>;
  }
  return <section className="port-processes" aria-label={t("portProcesses")} aria-busy={busy}>
    <h2 className="sr-only">{t("portProcesses")}</h2>
    <div className="port-process-toolbar">
      {inventory?.supported && <div className="port-process-filters" role="group" aria-label={t("processProjectFilter")}>
        {([
          ["all", "processFilterAll", scopedRows.length],
          ["linked", "processFilterLinked", linkedCount],
          ["unknown", "processFilterUnknown", scopedRows.length - linkedCount],
        ] as const).map(([value, label, count]) => <Button key={value} variant="quiet" aria-pressed={attributionFilter === value} onClick={() => setAttributionFilter(value)}>{t(label)}<span>{count}</span></Button>)}
      </div>}
      <label className="port-process-search-field"><MagnifyingGlass aria-hidden /><input ref={search} aria-label={t("processSearch")} placeholder={t("processSearch")} value={query} onChange={e => setQuery(e.target.value)} />{query && <Button size="icon" variant="quiet" aria-label={t("clear")} onClick={() => { setQuery(""); search.current?.focus(); }}><X aria-hidden /></Button>}</label>
      <div className="port-process-toolbar-actions"><Button loading={busy} aria-label={t("processRecheck")} title={t("processRecheck")} onClick={() => void perform(async () => { await refresh(); setMessage(t("processRefreshed")); })}><ArrowClockwise aria-hidden />{t("refresh")}</Button>{onClose && <Button variant="quiet" size="icon" aria-label={t("close")} onClick={onClose}><X aria-hidden /></Button>}</div>
    </div>
    {context && <div className="port-conflict-summary"><strong>{t("portConflicts")} · {context.ports.join(", ")}</strong><span>{conflictPorts === null ? t("loading") : conflictPorts.length ? `${t("processStillOccupied")} ${conflictPorts.join(", ")}` : t("processPortsClear")}</span><Button onClick={() => onJump(context.projectId)}>{t("jumpToProject")}</Button></div>}
    {busy && <p role="status" className="port-process-feedback">{t(inventory ? "stateCached" : "stateLoading")}</p>}
    {error && <p role="alert" className="port-process-feedback error-copy">{error}{inventory && <span> · {t("stateCached")}</span>}<Button disabled={busy} variant="quiet" onClick={() => void perform(refresh)}>{t("retry")}</Button></p>}
    {message && <p role="status" className="port-process-feedback">{message}</p>}
    {inventory?.supported === false ? <EmptyState title={t("processWindowsOnly")} body={t("processScopeHint")} /> : inventory?.supported && <>
      <div className="port-process-results"><span title={t("processScopeHint")}>{rows.length} / {scopedRows.length} · {t("processSnapshotHint")}</span>{updatedAt && <time dateTime={updatedAt}>{t("stateObserved")} {formatTime(updatedAt)}</time>}</div>
      <div className="port-process-table-wrap"><table className="port-process-table"><thead><tr>{sortHeader("ports", t("listeningPorts"))}{sortHeader("name", t("processName"))}{sortHeader("pid", "PID")}{sortHeader("project", t("processProject"))}<th scope="col">{t("processActions")}</th></tr></thead>
        <tbody>{rows.map(row => {
          const key = `${row.pid}-${row.identity?.createdAt ?? "unknown"}`;
          const project = projects.find(p => p.id === row.projectId);
          const restriction = row.restriction === "permission_denied" ? t("processStopPermissionDenied") : row.restriction ? processError(row.restriction, t) : !row.identity ? t("processIdentityUnavailable") : undefined;
          const evidence = t(({ task: "processEvidenceTask", manual: "processEvidenceManual", cwd: "processEvidenceCwd", executable: "processEvidenceExe", unknown: "processEvidenceUnknown" } as const)[row.attribution]);
          return <tr key={key}>
          <td><div className="port-process-ports">{[...row.ports].sort((a, b) => a - b).map(port => <code key={port}>{port}</code>)}</div></td>
          <td><div className="port-process-identity"><strong className="port-process-name">{row.name || t("processUnknown")}</strong><span className="port-process-source">{t(row.runId ? "processManaged" : "processExternal")}</span></div><code className="port-process-executable" aria-label={t("processExecutable")}>{row.identity?.executable || t(row.restriction === "permission_denied" ? "processPathPermissionDenied" : "processPathUnavailable")}</code></td>
          <td><code className="port-process-pid">{row.pid}</code></td>
          <td><div className="port-process-project-line">{row.projectId ? <Button className="port-process-project" variant="quiet" title={project?.canonicalPath} onClick={() => onJump(row.projectId!)}>{project?.displayName ?? t("processUnknown")}</Button> : <strong className="port-process-unknown">{t("processUnknown")}</strong>}
            {row.identity && !row.runId && <Button variant="quiet" size="icon" aria-label={t("processAssociate")} title={t("processAssociate")} disabled={busy} onClick={() => { trigger.current = document.activeElement as HTMLElement; setAssociation(row); setProjectId(row.projectId ?? ""); setProjectQuery(""); }}><LinkSimple aria-hidden /></Button>}</div>{project?.canonicalPath && <code className="port-process-executable">{project.canonicalPath}</code>}<small className="port-process-evidence">{evidence}</small></td>
          <td><div className="port-process-actions">
            {restriction ? <span className="port-process-restricted">{restriction}</span> : <>
              {row.runId && <Button title={t("processManagedScope")} disabled={busy} onClick={() => void prepare(row, "graceful")}>{t("processGraceful")}</Button>}
              <Button className="port-process-force" variant="danger" disabled={busy} onClick={() => void prepare(row, "force")}><Square aria-hidden />{t("processForce")}</Button>
            </>}
          </div></td>
        </tr>;
        })}</tbody></table>
        {!rows.length && !busy && <EmptyState title={t("processEmpty")} body={t("processSearch")} actions={(query || attributionFilter !== "all") ? <Button onClick={() => { setQuery(""); setAttributionFilter("all"); search.current?.focus(); }}>{t("clear")}</Button> : undefined} />}
      </div>
      <footer className="port-process-footer"><Info aria-hidden /><span>{t("processGracefulUnavailable")}</span></footer>
    </>}
    <ConfirmDialog finalFocus={returnFocus} className="process-stop-dialog" open={!!preview} title={t(preview?.mode === "force" ? "processForce" : "processGraceful")} busy={busy} cancelLabel={t("cancel")} confirmLabel={t(preview?.mode === "force" ? "processForceConfirm" : "processGracefulConfirm")} onOpenChange={open => { if (!open && !busy) setPreview(undefined); }} onConfirm={stop} body={preview && <>
      <span className="process-confirm-target">{preview.target.name} · PID {preview.target.pid}<br />{preview.target.identity?.executable}<br />{t("processProject")}: {projects.find(p => p.id === preview.target.projectId)?.displayName ?? t("processUnknown")}</span>
      <span className="process-confirm-target">{t("processAffected")}: {preview.processCount}<br />{preview.affected.map(p => `${p.name} · PID ${p.pid} · TCP ${p.ports.join(", ")}`).join("\n")}</span>
      <span className="process-confirm-target">{t(preview.target.runId ? "processManagedScope" : "processExternalScope")}</span>
      <span>{t(preview.mode === "force" ? "processForceWarning" : "processGracefulWarning")}</span><br /><span>{t("processIdentityGuard")}</span>
    </>} />
    <ConfirmDialog finalFocus={returnFocus} open={!!association} title={t("processAssociate")} confirmVariant="primary" busy={busy} cancelLabel={t("cancel")} confirmLabel={t("save")} onOpenChange={open => { if (!open && !busy) setAssociation(undefined); }} onConfirm={() => perform(async () => {
      if (!association?.identity) return;
      await api.associatePortProcess(association.identity, projectId || null); setAssociation(undefined); setMessage(t("processAssociationSaved")); await refresh();
    })} body={<><span>{association?.name} · PID {association?.pid}</span><br /><span>{t("processAssociationHint")}</span>{error && <span role="alert">{error}</span>}
      <input className="port-process-search" aria-label={t("processProjectSearch")} placeholder={t("processProjectSearch")} value={projectQuery} onChange={e => setProjectQuery(e.target.value)} />
      <select className="port-process-select" aria-label={t("processProject")} value={projectId} onChange={e => setProjectId(e.target.value)}><option value="">{t("processAutoAssociation")}</option>{projects.filter(p => p.id === projectId || `${p.displayName} ${p.canonicalPath}`.toLowerCase().includes(projectQuery.toLowerCase())).map(p => <option key={p.id} value={p.id}>{p.displayName} · {p.canonicalPath}</option>)}</select>
    </>} />
  </section>;
}
