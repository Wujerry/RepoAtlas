import {
  ArrowClockwise,
  ArrowRight,
  CheckCircle,
  Clock,
  FolderSimple,
  GitBranch,
  WarningCircle,
  XCircle,
} from "@phosphor-icons/react";
import { useEffect, useMemo, useState } from "react";
import { api } from "../lib/api";
import { formatTime, stackOf } from "../lib/format";
import { LanguageGlyph, languageFallback } from "../lib/project-identity";
import type { DashboardSnapshot, ProjectIcon } from "../types";
import type { MessageKey } from "../i18n";
import { Button } from "./ui/button";
import { EmptyState } from "./ui/feedback";

type Translator = (key: MessageKey) => string;

export interface HomeDashboardProps {
  t: Translator;
  refreshKey: string;
  onOpenProject: (projectId: string) => void;
  onOpenCollection: (collectionId: string) => void;
  onOpenRun: (runId: string) => void;
  onOpenAttention: () => void;
  onOpenAttentionItem: (item: DashboardSnapshot["attentionPreview"][number]) => void;
}

function statusLabel(status: string, t: Translator): string {
  if (status === "succeeded") return t("succeeded");
  if (status === "failed") return t("failed");
  if (status === "cancelled") return t("cancelled");
  if (status === "running" || status === "starting") return t("running");
  return status;
}

function statusClass(status: string): string {
  if (status === "succeeded") return "is-success";
  if (status === "failed") return "is-danger";
  if (status === "cancelled") return "is-muted";
  if (status === "running" || status === "starting") return "is-running";
  return "is-muted";
}

export function HomeDashboard({ t, refreshKey, onOpenProject, onOpenCollection, onOpenRun, onOpenAttention, onOpenAttentionItem }: HomeDashboardProps) {
  const [snapshot, setSnapshot] = useState<DashboardSnapshot>();
  const [icons, setIcons] = useState<Record<string, ProjectIcon>>({});
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string>();
  const [reloadToken, setReloadToken] = useState(0);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    api.getDashboardSnapshot().then((next) => {
      if (cancelled) return;
      setSnapshot(next);
      setError(undefined);
    }).catch((reason) => {
      if (!cancelled) setError(String(reason));
    }).finally(() => {
      if (!cancelled) setLoading(false);
    });
    return () => { cancelled = true; };
  }, [refreshKey, reloadToken]);

  const recentProjectIds = useMemo(() => snapshot?.recentProjects.map((item) => item.project.id) ?? [], [snapshot]);
  useEffect(() => {
    if (recentProjectIds.length === 0) { setIcons({}); return; }
    let cancelled = false;
    api.readProjectIcons(recentProjectIds).then((items) => {
      if (!cancelled) setIcons(Object.fromEntries(items.map((item) => [item.projectId, item])));
    }).catch(() => {
      if (!cancelled) setIcons({});
    });
    return () => { cancelled = true; };
  }, [recentProjectIds.join("|")]);

  if (loading && !snapshot) {
    return <main id="main-content" className="home-dashboard" aria-busy="true">
      <div className="home-dashboard-header"><div><span className="eyebrow">RepoAtlas</span><h1>{t("dashboard")}</h1><p>{t("dashboardHint")}</p></div></div>
      <div className="dashboard-metric-grid" aria-hidden="true">{[0, 1, 2, 3].map((item) => <div className="dashboard-metric skeleton-block" key={item} />)}</div>
      <div className="dashboard-skeleton-grid" aria-hidden="true"><div className="skeleton-block" /><div className="skeleton-block" /></div>
    </main>;
  }

  if (!snapshot) {
    return <main id="main-content" className="home-dashboard"><EmptyState title={t("dashboardLoadFailed")} body={error ?? t("dashboardLoadFailed")} actions={<Button variant="primary" onClick={() => setReloadToken((value) => value + 1)}>{t("retry")}</Button>} /></main>;
  }

  const featured = snapshot.recentProjects.find(({ project }) => project.availability === "ready" && !project.archived);

  const metrics = [
    { label: t("projects"), value: snapshot.projectCount, note: `${snapshot.availableProjectCount} ${t("available")} · ${snapshot.unavailableProjectCount} ${t("unavailable")}` },
    { label: t("collections"), value: snapshot.collectionCount, note: t("dashboardCollectionNote") },
    { label: t("activeTasks"), value: snapshot.activeRunCount, note: t("dashboardRunningNote") },
    { label: t("attentionCenter"), value: snapshot.attentionCount, note: t("dashboardAttentionNote") },
  ];

  return <main id="main-content" className="home-dashboard">
    <header className="home-dashboard-header">
      <div><span className="eyebrow">RepoAtlas</span><h1>{t("dashboard")}</h1><p>{t("dashboardHint")}</p></div>
      <div className="home-dashboard-updated" title={`${t("dashboardGeneratedAt")} ${formatTime(snapshot.generatedAt)}`}><span>{t("dashboardGeneratedAt")} {formatTime(snapshot.generatedAt)}</span><Button size="icon" variant="quiet" loading={loading} aria-label={t("refresh")} onClick={() => setReloadToken((value) => value + 1)}><ArrowClockwise aria-hidden="true" /></Button></div>
    </header>

    {error && <div className="dashboard-inline-error" role="status"><WarningCircle aria-hidden="true" />{t("dashboardRefreshFailed")}<Button variant="quiet" onClick={() => setReloadToken((value) => value + 1)}>{t("retry")}</Button></div>}

    <div className="dashboard-launchpad">
      <section className="dashboard-feature" aria-label={t("continueWork")}>
        <div className="dashboard-feature-heading"><span className="eyebrow">{t(featured ? "continueWork" : "dashboardWelcome")}</span><FolderSimple aria-hidden="true" /></div>
        <h2 title={featured?.project.displayName}>{featured?.project.displayName ?? t("dashboardWelcomeTitle")}</h2>
        <p>{featured?.project.description || t(featured ? "dashboardResumeHint" : "dashboardNoRecentProjects")}</p>
        {featured && <>
          <div className="dashboard-feature-stack">{stackOf(featured.project).slice(0, 3).map(value => <span key={value}>{value}</span>)}{featured.git?.branch && <span><GitBranch aria-hidden="true" />{featured.git.branch}</span>}</div>
          <div className="dashboard-feature-footer"><code title={featured.project.canonicalPath}>{featured.project.canonicalPath}</code><Button variant="primary" onClick={() => onOpenProject(featured.project.id)}>{t("dashboardResumeProject")}<ArrowRight aria-hidden="true" /></Button></div>
        </>}
      </section>
      <section className="dashboard-metric-grid" aria-label={t("dashboardStatusSummary")}>
        {metrics.map((metric) => <article className="dashboard-metric" key={metric.label}><div><span>{metric.label}</span><strong>{metric.value}</strong><small>{metric.note}</small></div></article>)}
      </section>
    </div>

    <div className="dashboard-content-grid dashboard-recent-grid">
      <section className="dashboard-section">
        <div className="dashboard-section-heading"><div><span className="eyebrow">{t("quickOpen")}</span><h2>{t("recentProjects")}</h2></div></div>
        {snapshot.recentProjects.length === 0 ? <p className="dashboard-empty-copy">{t("dashboardNoRecentProjects")}</p> : <div className="dashboard-project-list">{snapshot.recentProjects.map(({ project, git, latestRun }) => <button key={project.id} className="dashboard-project" onClick={() => onOpenProject(project.id)}>
          <span className="project-identity-mark" aria-hidden="true">{icons[project.id]?.dataUrl ? <img src={icons[project.id].dataUrl ?? undefined} alt="" /> : <LanguageGlyph language={languageFallback(project)} />}</span>
          <span className="dashboard-project-copy"><strong>{project.displayName}</strong><code title={project.canonicalPath}>{project.canonicalPath}</code><span>{stackOf(project).slice(0, 3).map((value) => <small className="badge" key={value}>{value}</small>)}</span></span>
          <span className="dashboard-project-state">{project.availability !== "ready" ? <small className="is-danger">{t("unavailable")}</small> : git?.branch ? <small><GitBranch aria-hidden="true" />{git.branch}{git.dirty === true ? ` · ${t("dirty")}` : ""}</small> : null}{latestRun && <small className={statusClass(latestRun.status)}>{statusLabel(latestRun.status, t)} · {formatTime(latestRun.finishedAt ?? latestRun.startedAt)}</small>}</span>
          <ArrowRight aria-hidden="true" />
        </button>)}</div>}
      </section>

      <section className="dashboard-section">
        <div className="dashboard-section-heading"><div><span className="eyebrow">{t("taskHistory")}</span><h2>{t("recentRuns")}</h2></div></div>
        {snapshot.recentRuns.length === 0 ? <p className="dashboard-empty-copy">{t("dashboardNoRecentRuns")}</p> : <div className="dashboard-run-list">{snapshot.recentRuns.map(({ run, projectName }) => <button key={run.id} className="dashboard-run" onClick={() => onOpenRun(run.id)}><span className={`dashboard-run-status ${statusClass(run.status)}`} aria-hidden="true" /><span><strong>{projectName}</strong><small>{run.kind} · {statusLabel(run.status, t)}</small></span><time>{formatTime(run.finishedAt ?? run.startedAt)}</time><ArrowRight aria-hidden="true" /></button>)}</div>}
      </section>
    </div>

    <div className="dashboard-content-grid">
      <section className="dashboard-section dashboard-collections">
        <div className="dashboard-section-heading"><div><span className="eyebrow">{t("projectOrganization")}</span><h2>{t("collections")}</h2></div></div>
        {snapshot.collections.length === 0 ? <p className="dashboard-empty-copy">{t("dashboardNoCollections")}</p> : <div className="dashboard-collection-list">{snapshot.collections.map((collection) => <button className="dashboard-collection" key={collection.id} onClick={() => onOpenCollection(collection.id)}>
          <span className="dashboard-collection-top"><span><strong>{collection.name}</strong>{collection.description && <small>{collection.description}</small>}</span><ArrowRight aria-hidden="true" /></span>
          <span className="dashboard-collection-meta"><span>{collection.projectCount} {t("projects")}</span>{collection.archivedProjectCount > 0 && <span>{collection.archivedProjectCount} {t("archived")}</span>}{collection.activeRunCount > 0 && <span className="is-running">{collection.activeRunCount} {t("running")}</span>}{collection.attentionCount > 0 && <span className="is-danger">{collection.attentionCount} {t("attentionShort")}</span>}</span>
          <time>{collection.lastActivityAt ? `${t("lastActivity")} ${formatTime(collection.lastActivityAt)}` : t("noRecentActivity")}</time>
        </button>)}</div>}
      </section>

      <section className="dashboard-section dashboard-attention">
        <div className="dashboard-section-heading"><div><span className="eyebrow">{t("needsAction")}</span><h2>{t("attentionCenter")}</h2></div>{snapshot.attentionCount > snapshot.attentionPreview.length && <Button variant="quiet" onClick={onOpenAttention}>{t("viewAll")}</Button>}</div>
        {snapshot.attentionPreview.length === 0 ? <p className="dashboard-empty-copy is-clear"><CheckCircle weight="fill" aria-hidden="true" />{t("dashboardNoAttention")}</p> : <div className="dashboard-attention-list">{snapshot.attentionPreview.map((item) => <button key={item.id} className={`dashboard-attention-item severity-${item.severity}`} onClick={() => onOpenAttentionItem(item)}><WarningCircle weight="fill" aria-hidden="true" /><span><strong>{item.title}</strong><small>{item.detail}</small><time>{formatTime(item.occurredAt)}</time></span><ArrowRight aria-hidden="true" /></button>)}</div>}
      </section>
    </div>

    <section className="dashboard-section dashboard-results">
      <div className="dashboard-section-heading"><div><span className="eyebrow">{t("lastSevenDays")}</span><h2>{t("taskRunResults")}</h2></div><strong>{snapshot.sevenDayRuns.total}</strong></div>
      <div className="dashboard-result-grid">
        <div className="dashboard-result is-success"><CheckCircle weight="fill" aria-hidden="true" /><span>{t("succeeded")}</span><strong>{snapshot.sevenDayRuns.succeeded}</strong></div>
        <div className="dashboard-result is-danger"><XCircle weight="fill" aria-hidden="true" /><span>{t("failed")}</span><strong>{snapshot.sevenDayRuns.failed}</strong></div>
        <div className="dashboard-result is-muted"><Clock weight="fill" aria-hidden="true" /><span>{t("cancelled")}</span><strong>{snapshot.sevenDayRuns.cancelled}</strong></div>
      </div>
    </section>
  </main>;
}
