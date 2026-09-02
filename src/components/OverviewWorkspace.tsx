import { ArrowDown, ArrowUp, CheckCircle, Code, DesktopTower, GitBranch, Play, WarningCircle } from "@phosphor-icons/react";
import { useEffect } from "react";
import type { MessageKey } from "../i18n";
import { formatTime } from "../lib/format";
import type { EnvironmentInspection, GitStatus, ProjectDetail, ProjectFile, ReadmeDocument, RuntimeStatus, TaskRun } from "../types";
import { Button } from "./ui/button";
import { Skeleton } from "./ui/feedback";
import { InlineLoadError, MarkdownDocument } from "./DashboardShared";

export function OverviewWorkspace(props: {
  detail: ProjectDetail;
  git: GitStatus | null;
  readme?: ReadmeDocument;
  readmeLoading: boolean;
  readmeError?: string;
  onRetryReadme: () => void;
  agents?: ReadmeDocument;
  agentsLoading: boolean;
  agentsMissing: boolean;
  onNeedAgents: () => void;
  t: (key: MessageKey) => string;
  tagDraft: string;
  setTagDraft: (value: string) => void;
  onNotes: (notes: string) => void;
  onTags: (tags: string[]) => void;
  tasks: ProjectDetail["tasks"];
  runs: TaskRun[];
  environment?: EnvironmentInspection;
  environmentLoading: boolean;
  environmentError?: string;
  onRetryEnvironment: () => void;
  onRunTask: (id: string) => void;
  onPreviewFile: (file: ProjectFile) => void;
  onOpenProject?: (id: string) => void;
}) {
  const { detail, git, readme, readmeLoading, readmeError, onRetryReadme, agents, agentsLoading, onNeedAgents, t, tagDraft, setTagDraft, onNotes, onTags, tasks, runs, environment, environmentLoading, environmentError, onRetryEnvironment, onRunTask, onPreviewFile, onOpenProject } = props;
  const project = detail.project;
  useEffect(() => { onNeedAgents(); }, [onNeedAgents, project.id]);
  const factGroup = (kind: string, fallback: string[] = []) => {
    const facts = detail.facts.filter((fact) => fact.kind === kind);
    return facts.length > 0
      ? { values: facts.map((fact) => fact.value), sources: facts.map((fact) => fact.source) }
      : { values: fallback, sources: fallback.map(() => "") };
  };
  const groups = [
    { label: t("languages"), ...factGroup("language", project.languages) },
    { label: t("frameworks"), ...factGroup("framework", project.frameworks) },
    { label: t("packageManagers"), ...factGroup("packageManager", project.packageManagers) },
    { label: t("vcs"), values: project.vcsKind === "none" ? [] : [project.vcsKind], sources: project.vcsKind === "none" ? [] : [project.vcsKind === "git" ? ".git" : ".svn"] },
    { label: t("containers"), ...factGroup("container") },
  ].filter((group) => group.values.length > 0);
  const latestRun = runs[0];
  const quickTasks = tasks.slice(0, 3);
  const environmentSummary = (() => {
    const runtimes = environment?.runtimes ?? [];
    if (!runtimes.length) return t("noEnvironment");
    const match = runtimes.filter((item) => item.matchState === "match").length;
    const mismatch = runtimes.filter((item) => item.matchState === "mismatch").length;
    const missing = runtimes.filter((item) => item.matchState === "missing").length;
    const undeclared = runtimes.filter((item) => item.matchState === "undeclared").length;
    const unknown = runtimes.filter((item) => item.matchState === "unknown").length;
    return [match ? match + " " + t("versionMatch") : "", mismatch ? mismatch + " " + t("versionMismatch") : "", missing ? missing + " " + t("versionMissing") : "", undeclared ? undeclared + " " + t("undeclaredVersion") : "", unknown ? unknown + " " + t("unknownVersion") : ""].filter(Boolean).join(" · ");
  })();
  const files = environment?.files ?? detail.projectFiles ?? [];
  const runtimeCounts = (environment?.runtimes ?? []).reduce((counts, runtime) => {
    counts[runtime.matchState] = (counts[runtime.matchState] ?? 0) + 1;
    return counts;
  }, {} as Record<string, number>);
  const groupedFiles = [
    { id: "manifest", label: t("fileGroupManifest"), items: files.filter((file) => file.kind === "manifest") },
    { id: "packageManager", label: t("fileGroupPackageManager"), items: files.filter((file) => file.kind === "packageManager") },
    { id: "lockfile", label: t("fileGroupLockfile"), items: files.filter((file) => file.kind === "lockfile") },
    { id: "runtime", label: t("fileGroupRuntime"), items: files.filter((file) => file.kind === "runtime") },
  ].filter((group) => group.items.length > 0);
  const matchLabel = (state: string) => state === "match" ? t("versionMatch") : state === "mismatch" ? t("versionMismatch") : state === "missing" ? t("versionMissing") : state === "undeclared" ? t("undeclaredVersion") : t("unknownVersion");
  const attentionCount = git?.snapshot.dirty ? git.files.length : 0;
  const branchName = git?.snapshot.branch ?? (project.vcsKind === "git" ? t("detachedHead") : project.vcsKind);
  const startHere = detail.startHere?.length ? detail.startHere : quickTasks.map((task) => ({ taskId: task.id, kind: task.kind, name: task.name, source: t("inferredTask"), inferred: task.inferred }));
  const recentEvents = (detail.recentEvents ?? []).filter((event) => !(event.kind === "open" && !event.detail));
  return <div className="overview-layout">
    <section id="overview-status" aria-labelledby="overview-status-title" className="content-card continue-card">
      <div className="section-heading overview-card-heading"><div><p className="eyebrow">{t("workspaceState")}</p><h2 id="overview-status-title">{t("continueWork")}</h2></div><span className={`overview-health ${git?.snapshot.dirty ? "is-warning" : "is-ok"}`}>{git?.snapshot.dirty ? <WarningCircle weight="fill" /> : <CheckCircle weight="fill" />}{git?.snapshot.dirty ? t("dirty") : project.vcsKind === "git" ? t("clean") : project.vcsKind}</span></div>
      <div className="continue-command-center">
        <div className="workspace-focus">
          <div className="workspace-focus-icon"><GitBranch weight="duotone" /></div>
          <div className="workspace-focus-copy"><span>{t("currentBranch")}</span><strong>{branchName}</strong><p>{git?.snapshot.dirty ? `${attentionCount} ${t("dirtyFiles")}` : t("cleanWorkspaceHint")}</p></div>
          <div className="sync-pair" aria-label={t("aheadBehind")}><span aria-label={`${t("ahead")} ${git?.snapshot.ahead ?? 0}`}><ArrowUp aria-hidden="true" />{git?.snapshot.ahead ?? 0}</span><span aria-label={`${t("behind")} ${git?.snapshot.behind ?? 0}`}><ArrowDown aria-hidden="true" />{git?.snapshot.behind ?? 0}</span></div>
        </div>
        <div className="workspace-signals">
          <div className="workspace-signal"><span>{t("recentTask")}</span><strong>{latestRun ? latestRun.kind : t("noRecentTask")}</strong><em data-tone={latestRun?.status ?? "idle"}>{latestRun ? t(statusKey(latestRun.status)) : t("history")}</em></div>
          <div className={`workspace-signal ${attentionCount ? "needs-attention" : ""}`}><span>{t("attention")}</span><strong>{attentionCount ? `${attentionCount} ${t("dirtyFiles")}` : t("clean")}</strong><em>{attentionCount ? t("workspaceState") : t("operationCompleted")}</em></div>
        </div>
      </div>
      <div className="start-here-row overview-subsection">
        <div className="overview-subsection-heading"><span>{t("startHere")}</span><small>{String(startHere.length).padStart(2, "0")}</small></div>
        {startHere.length === 0 ? <p className="muted-copy">{t("noQuickTasks")}</p> : <div className="start-here-grid">{startHere.map((task) => <button className="start-here-action" key={task.taskId} onClick={() => onRunTask(task.taskId)}><span className="start-here-icon"><Play weight="fill" /></span><span className="start-here-copy"><strong>{task.name}</strong><em>{task.source}</em></span><span className="start-here-kind">{task.kind}</span></button>)}</div>}
      </div>
      <div className="activity-list overview-subsection">
        <div className="overview-subsection-heading"><span>{t("recentActivity")}</span><small>{String(recentEvents.length).padStart(2, "0")}</small></div>
        {recentEvents.length === 0 ? <p className="muted-copy">{t("noRecentActivity")}</p> : <div className="activity-timeline">{recentEvents.map((event) => <div key={event.id}><i aria-hidden="true" /><span><strong>{event.title}</strong>{event.detail && <em>{event.detail}</em>}</span><time>{formatTime(event.createdAt)}</time></div>)}</div>}
      </div>
      {detail.lineage?.checkouts?.length ? <div className="lineage-row"><span>{t("checkoutLineage")}</span><div>{detail.lineage.checkouts.map((checkout) => <button type="button" key={checkout.projectId} className="file-chip" onClick={() => onOpenProject?.(checkout.projectId)}>{checkout.displayName}</button>)}</div></div> : null}
    </section>
    <section id="overview-environment" aria-labelledby="overview-environment-title" className="content-card environment-card">
      <div className="section-heading overview-card-heading"><div><p className="eyebrow">{t("detectedEvidence")}</p><h2 id="overview-environment-title">{t("environment")}</h2></div><span className="env-summary">{environmentSummary}</span></div>
      {environmentLoading ? <Skeleton className="skeleton-list" /> : environmentError ? <InlineLoadError title={t("environmentLoadFailed")} detail={environmentError} retryLabel={t("retry")} onRetry={onRetryEnvironment} /> : (
        <>
          <div className="environment-overview">
            <div className="environment-overview-copy"><span className="environment-overview-icon"><DesktopTower weight="duotone" /></span><div><strong>{(environment?.runtimes ?? []).length}</strong><span>{t("environment")}</span></div></div>
            <div className="environment-statuses">
              <span data-state="match"><i />{runtimeCounts.match ?? 0} {t("versionMatch")}</span>
              <span data-state="mismatch"><i />{runtimeCounts.mismatch ?? 0} {t("versionMismatch")}</span>
              <span data-state="missing"><i />{runtimeCounts.missing ?? 0} {t("versionMissing")}</span>
              <span data-state="undeclared"><i />{runtimeCounts.undeclared ?? 0} {t("undeclaredVersion")}</span>
              <span data-state="unknown"><i />{runtimeCounts.unknown ?? 0} {t("unknownVersion")}</span>
            </div>
          </div>
          <div className="runtime-table">
            {(environment?.runtimes ?? []).length === 0 ? <p className="muted-copy">{t("noEnvironment")}</p> : environment?.runtimes.map((runtime: RuntimeStatus) => (
              <div className="runtime-card" key={`${runtime.ecosystem}-${runtime.source ?? "inferred"}-${runtime.constraint ?? "undeclared"}`}>
                <div className="runtime-card-heading"><span className="runtime-monogram">{runtime.label.slice(0, 1)}</span><div><strong>{runtime.label}</strong><small>{runtime.ecosystem}</small></div><em data-state={runtime.matchState}><i />{matchLabel(runtime.matchState)}</em></div>
                <div className="runtime-versions"><div><span>{t("requiredVersion")}</span><strong>{runtime.constraint ?? t("undeclaredVersion")}</strong></div><div><span>{t("localVersion")}</span><strong>{runtime.localVersion ?? t("versionMissing")}</strong></div></div>
                {runtime.source ? <button type="button" className="runtime-source" onClick={() => { const source = runtime.source; if (!source) return; onPreviewFile({ kind: "runtime", path: source.split("#")[0], source }); }}><Code />{runtime.source}</button> : <span className="runtime-source is-empty">{t("undeclaredVersion")}</span>}
              </div>
            ))}
          </div>
          <div className="file-groups overview-subsection">
            <div className="overview-subsection-heading"><span>{t("projectFiles")}</span><small>{String(files.length).padStart(2, "0")}</small></div>
            {groupedFiles.length === 0 ? <p className="muted-copy">{t("noProjectFiles")}</p> : groupedFiles.map((group) => (
              <div className="file-group" key={group.id}>
                <span>{group.label}</span>
                <div>{group.items.map((file) => <button type="button" className="file-chip" key={group.id + "-" + file.path} onClick={() => onPreviewFile(file)}>{file.path}</button>)}</div>
              </div>
            ))}
          </div>
        </>
      )}
    </section>
    <section id="overview-readme" className="content-card overview-readme"><div className="section-heading"><div><p className="eyebrow">{readme?.path ?? detail.readmePath ?? "README"}</p><h2>{t("readme")}</h2></div>{readme?.truncated && <span className="status-warning">{t("readmeTruncated")}</span>}</div>
      {readmeLoading ? <Skeleton className="skeleton-code" /> : readmeError ? <InlineLoadError title={t("readmeLoadFailed")} detail={readmeError} retryLabel={t("retry")} onRetry={onRetryReadme} /> : readme?.content || detail.readmeExcerpt ? <MarkdownDocument content={readme?.content ?? detail.readmeExcerpt ?? ""} /> : <p className="muted-copy">{t("noReadme")}</p>}
    </section>
    <section id="overview-agents" className="content-card overview-readme"><div className="section-heading"><div><p className="eyebrow">{agents?.path ?? "AGENTS.md"}</p><h2>{t("agentsGuide")}</h2></div>{agents?.truncated && <span className="status-warning">{t("readmeTruncated")}</span>}</div>
      {agentsLoading ? <Skeleton className="skeleton-code" /> : agents?.content ? <MarkdownDocument content={agents.content} /> : <p className="muted-copy">{t("noAgentsGuide")}</p>}
    </section>
    <section id="overview-profile" className="content-card overview-facts"><div className="section-heading"><div><p className="eyebrow">{t("detectedEvidence")}</p><h2>{t("projectProfile")}</h2></div><span>{groups.length}</span></div>
      {groups.length === 0 ? <p className="muted-copy">{t("noFacts")}</p> : <div className="profile-groups">{groups.map((group) => <div className="profile-group" key={group.label}><span>{group.label}</span><div>{group.values.map((value, index) => <span className="badge" title={group.sources?.[index]} key={group.label + "-" + value}>{value}</span>)}</div></div>)}</div>}
      {detail.dependencies && <div className="dependency-block"><div><strong>{detail.dependencies.packageManager ?? t("dependencies")}</strong><span>{detail.dependencies.declared.length} {t("declaredDependencies")} · {detail.dependencies.lockfile ?? t("noLockfile")}</span></div><div className="tag-cloud">{detail.dependencies.declared.slice(0, 24).map((item) => <span className="badge" key={item}>{item}</span>)}</div></div>}
    </section>
    <section id="overview-notes" className="content-card overview-notes"><div className="section-heading"><div><p className="eyebrow">{t("personalContext")}</p><h2>{t("tagsAndNotes")}</h2></div></div>
      <div className="tag-cloud editable">{project.tags.map((tag) => <button key={tag} onClick={() => onTags(project.tags.filter((item) => item !== tag))}>{tag}<span aria-hidden="true">x</span></button>)}</div>
      <form className="inline-form" onSubmit={(event) => { event.preventDefault(); const value = tagDraft.trim(); if (!value || project.tags.includes(value)) return; onTags([...project.tags, value]); setTagDraft(""); }}><label><span>{t("addTag")}</span><input value={tagDraft} onChange={(event) => setTagDraft(event.target.value)} /></label><Button type="submit">{t("add")}</Button></form>
      <label className="field-group"><span>{t("notes")}</span><textarea key={project.id} defaultValue={project.notes ?? ""} onBlur={(event) => onNotes(event.target.value)} placeholder={t("notesHint")} /></label>
    </section>
  </div>;
}

function statusKey(status: string): MessageKey {
  return status === "succeeded" ? "succeeded" : status === "failed" ? "failed" : status === "cancelled" ? "cancelled" : "running";
}