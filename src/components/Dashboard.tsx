import { motion } from "framer-motion";
import { BookOpenText, CaretDown, Code, Copy, DotsThree, Files as FilesIcon, FolderOpen, GitBranch, PencilSimple, Play, Sparkle, Star, TerminalWindow } from "@phosphor-icons/react";
import { Dialog } from "@base-ui/react/dialog";
import { save } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import type { MessageKey } from "../i18n";
import { api, onTaskExited, onTaskPersistenceFailed } from "../lib/api";
import { stackOf } from "../lib/format";
import type { GitOp, GitStatus, ProjectDetail, ProjectTab, ReadmeDocument, TaskRun, ToastTone } from "../types";
import type { EnvironmentInspection, ExternalTool, ProjectFile } from "../types";
import { Button } from "./ui/button";
import { ConfirmDialog } from "./ui/confirm";
import { Skeleton } from "./ui/feedback";
import { ActionMenu } from "./ui/menu";
import { DescriptionDialog } from "./ui/description-dialog";
import { AgentGlyph, IdeGlyph, TerminalGlyph } from "../lib/project-identity";
import { OverviewWorkspace } from "./OverviewWorkspace";
import { GitWorkspace } from "./GitWorkspace";
import { TaskWorkspace } from "./TaskWorkspace";
import { FilesWorkspace } from "./FilesWorkspace";
import { InlineLoadError, MarkdownDocument, statusKey, taskConfirmation } from "./DashboardShared";
import { getTaskLog, rememberStartedRun, subscribeTaskLog } from "../lib/task-runs";

export function Dashboard({ detail, t, notify, onFavorite, onArchive, onRefresh, onRemove, onOpenExplorer, onOpenTerminal, onOpenIde, onOpenAgent, onDescription, onNotes, onTags, onOpenProject, onRelocate }: {
  detail: ProjectDetail;
  t: (key: MessageKey) => string;
  notify: (tone: ToastTone, title: string, detail?: string) => void;
  onFavorite: () => void | Promise<void>;
  onArchive: () => void | Promise<void>;
  onRefresh: () => void | Promise<void>;
  onRemove: () => void | Promise<void>;
  onOpenExplorer: () => void;
  onOpenTerminal: (terminal?: string) => void;
  onOpenIde: (ide: string) => void;
  onOpenAgent: (agent: string) => void;
  onDescription: (description: string | null) => void | Promise<void>;
  onNotes: (notes: string) => void;
  onTags: (tags: string[]) => void;
  onOpenProject?: (id: string) => void;
  onRelocate?: () => void;
}) {
  const project = detail.project;
  const [tab, setTab] = useState<ProjectTab>("overview");
  const [filesActivated, setFilesActivated] = useState(false);
  const [tagDraft, setTagDraft] = useState("");
  const [git, setGit] = useState<GitStatus | null>(null);
  const [gitLoading, setGitLoading] = useState(false);
  const [gitError, setGitError] = useState<string>();
  const [runs, setRuns] = useState<TaskRun[]>([]);
  const [runsLoading, setRunsLoading] = useState(false);
  const [runsError, setRunsError] = useState<string>();
  const [activeRunId, setActiveRunId] = useState<string>();
  const [logs, setLogs] = useState<Record<string, string>>({});
  const [commitMessage, setCommitMessage] = useState("");
  const [pendingGit, setPendingGit] = useState<GitOp | null>(null);
  const [gitBusy, setGitBusy] = useState(false);
  const [pendingTaskId, setPendingTaskId] = useState<string>();
  const [taskBusy, setTaskBusy] = useState(false);
  const [removeOpen, setRemoveOpen] = useState(false);
  const [reportOpen, setReportOpen] = useState(false);
  const [report, setReport] = useState<{ markdown: string; generatedAt: string } | null>(null);
  const [reportLoading, setReportLoading] = useState(false);
  const [selectedDiff, setSelectedDiff] = useState<{ path: string; staged: boolean }>();
  const [diff, setDiff] = useState("");
  const [diffLoading, setDiffLoading] = useState(false);
  const [readme, setReadme] = useState<ReadmeDocument>();
  const [readmeLoading, setReadmeLoading] = useState(false);
  const [readmeError, setReadmeError] = useState<string>();
  const [agents, setAgents] = useState<ReadmeDocument>();
  const [agentsLoading, setAgentsLoading] = useState(false);
  const [agentsMissing, setAgentsMissing] = useState(false);
  const [descriptionOpen, setDescriptionOpen] = useState(false);
  const [descriptionDraft, setDescriptionDraft] = useState(project.description ?? "");
  const [descriptionSaving, setDescriptionSaving] = useState(false);
  const [ides, setIdes] = useState<ExternalTool[]>([]);
  const [terminals, setTerminals] = useState<ExternalTool[]>([]);
  const [agentTools, setAgentTools] = useState<ExternalTool[]>([]);
  const [overviewSection, setOverviewSection] = useState<"status" | "environment" | "readme" | "agents" | "profile" | "notes">("status");
  const [environment, setEnvironment] = useState<EnvironmentInspection>();
  const [environmentLoading, setEnvironmentLoading] = useState(false);
  const [environmentError, setEnvironmentError] = useState<string>();
  const [previewFile, setPreviewFile] = useState<ProjectFile | null>(null);
  const [previewDoc, setPreviewDoc] = useState<ReadmeDocument>();
  const [previewLoading, setPreviewLoading] = useState(false);
  const [previewError, setPreviewError] = useState<string>();
  const contentRef = useRef<HTMLDivElement>(null);
  const readmeSequence = useRef(0);
  const agentsSequence = useRef(0);
  const agentsRequested = useRef<string | undefined>(undefined);

  async function loadGit() {
    if (project.vcsKind !== "git") { setGit(null); setGitError(undefined); return; }
    setGitLoading(true);
    setGitError(undefined);
    try { setGit(await api.gitStatus(project.id)); }
    catch (error) { setGit(null); setGitError(String(error)); notify("error", t("gitLoadFailed"), String(error)); }
    finally { setGitLoading(false); }
  }

  async function loadRuns() {
    setRunsLoading(true);
    setRunsError(undefined);
    try {
      const next = await api.listTaskRuns(project.id); setRuns(next);
      const runningRuns = next.filter((run) => run.status === "running");
      if (runningRuns.length > 0) {
        setActiveRunId((current) => (current && runningRuns.some((run) => run.id === current) ? current : runningRuns[0].id));
        const loaded = await Promise.all(runningRuns.map(async (run) => [run.id, await api.readTaskLog(run.id)] as const));
        setLogs((current) => ({ ...current, ...Object.fromEntries(loaded) }));
      }
    } catch (error) { setRunsError(String(error)); notify("error", t("tasksLoadFailed"), String(error)); }
    finally { setRunsLoading(false); }
  }

  useEffect(() => {
    setTab("overview"); setFilesActivated(false); setOverviewSection("status"); setGit(null); setGitError(undefined); setRuns([]); setRunsError(undefined); setLogs({}); setCommitMessage(""); setSelectedDiff(undefined); setDiff("");
    setDescriptionDraft(project.description ?? "");
    setAgents(undefined); setAgentsMissing(false);
    setEnvironment(undefined); setEnvironmentError(undefined);
    setPreviewFile(null); setPreviewDoc(undefined); setPreviewError(undefined);
    agentsRequested.current = undefined;
    void loadRuns();
  }, [project.id]);

  useEffect(() => {
    let cancelled = false;
    void api.listExternalTools().then((tools) => {
      if (cancelled) return;
      setIdes(tools.ides);
      setTerminals(tools.terminals);
      setAgentTools(tools.agents ?? []);
    }).catch((error) => {
      if (!cancelled) notify("error", t("openFailed"), String(error));
    });
    return () => { cancelled = true; };
  }, [notify, t]);

  useEffect(() => {
    const sequence = ++readmeSequence.current;
    setReadme(undefined);
    setReadmeError(undefined);
    if (!detail.readmePath) { setReadmeLoading(false); return; }
    setReadmeLoading(true);
    api.readProjectReadme(project.id).then((value) => { if (sequence === readmeSequence.current) setReadme(value); }).catch((error) => {
      if (sequence === readmeSequence.current) { setReadmeError(String(error)); notify("error", t("readmeLoadFailed"), String(error)); }
    }).finally(() => { if (sequence === readmeSequence.current) setReadmeLoading(false); });
  }, [detail.readmePath, notify, project.id, t]);

  useEffect(() => {
    const unsubs = Promise.all([
      onTaskExited((run) => {
        if (run.projectId !== project.id) return;
        if (run.status === "succeeded") {
          notify("success", t("taskFinished"), `${run.kind} · ${t(statusKey(run.status))}`);
          void api.readTaskLog(run.id).then((output) => {
            setActiveRunId(run.id);
            setLogs((current) => ({ ...current, [run.id]: output }));
          });
        } else {
          void api.readTaskLog(run.id).then((output) => {
            setActiveRunId(run.id);
            setLogs((current) => ({ ...current, [run.id]: output }));
            const tail = output.trim().split(/\r?\n/u).slice(-8).join("\n");
            const exit = run.exitCode == null ? t(statusKey(run.status)) : `${t("taskExitCode")} ${run.exitCode}`;
            notify("warning", t("taskFinished"), [run.kind, exit, tail].filter(Boolean).join("\n"));
          }).catch(() => {
            notify("warning", t("taskFinished"), `${run.kind} · ${t(statusKey(run.status))}`);
          });
        }
        void loadRuns();
      }),
      onTaskPersistenceFailed((failure) => {
        const run = failure.run;
        if (run && run.projectId !== project.id) return;
        notify("error", t("taskPersistenceFailed"), failure.error);
        if (run) {
          setActiveRunId(run.id);
          void loadRuns();
        }
      }),
    ]);
    return () => { void unsubs.then((functions) => functions.forEach((unsubscribe) => unsubscribe())); };
  }, [notify, project.id, t]);

  useEffect(() => {
    if (!activeRunId) return;
    const syncLog = (log: string) => setLogs((current) => (
      current[activeRunId] === log ? current : { ...current, [activeRunId]: log }
    ));
    syncLog(getTaskLog(activeRunId));
    return subscribeTaskLog(activeRunId, syncLog);
  }, [activeRunId]);

  const stagedCount = git?.files.filter((file) => file.staged).length ?? 0;

  async function runTask(taskId: string) {
    const task = detail.tasks.find((item) => item.id === taskId); if (!task) return;
    setTaskBusy(true);
    try {
      const conflicts = (task.expectedPorts?.length ?? 0) > 0
        ? await api.preflightTaskPorts(project.id, task.id)
        : [];
      const allowPortConflicts = conflicts.length > 0 && window.confirm(`${t("portConflictConfirm")}\n${conflicts.map((item) => `${item.port}${item.processName ? ` · ${item.processName}` : ""}${item.pid ? ` · PID ${item.pid}` : ""}`).join("\n")}`);
      if (conflicts.length > 0 && !allowPortConflicts) return;
      const run = await api.startTask(project.id, task.id, allowPortConflicts);
      setPendingTaskId(undefined); setActiveRunId(run.id); setLogs((current) => ({ ...current, [run.id]: "" })); rememberStartedRun(run); await loadRuns(); notify("info", t("taskStarted"), task.name);
    } catch (error) { notify("error", t("taskStartFailed"), String(error)); }
    finally { setTaskBusy(false); }
  }

  async function confirmGit() {
    if (!pendingGit) return; setGitBusy(true);
    try {
      const result = await api.gitExecute(project.id, pendingGit);
      if (result.ok) notify("success", t("gitOperationComplete"), result.stdout.trim() || t("operationCompleted"));
      else notify("error", t("gitOperationFailed"), result.stderr.trim() || result.stdout.trim());
      setPendingGit(null); setCommitMessage(""); await loadGit(); await onRefresh();
    } catch (error) { notify("error", t("gitOperationFailed"), String(error)); }
    finally { setGitBusy(false); }
  }

  async function loadDiff(path: string, staged: boolean) {
    setSelectedDiff({ path, staged }); setDiffLoading(true);
    try { setDiff((await api.gitDiff(project.id, path, staged)).patch || t("noDiff")); }
    catch (error) { setDiff(""); notify("error", t("diffLoadFailed"), String(error)); }
    finally { setDiffLoading(false); }
  }

  const overviewLinks = [
    { id: "status" as const, index: "01", label: t("continueWork") },
    { id: "environment" as const, index: "02", label: t("environment") },
    { id: "readme" as const, index: "03", label: t("readme") },
    { id: "agents" as const, index: "04", label: t("agentsGuide") },
    { id: "profile" as const, index: "05", label: t("projectProfile") },
    { id: "notes" as const, index: "06", label: t("tagsAndNotes") },
  ];

  function scrollToOverview(section: "status" | "environment" | "readme" | "agents" | "profile" | "notes") {
    setOverviewSection(section);
    const root = contentRef.current;
    if (!root) return;
    const scroll = () => {
      const target = root.querySelector(`#overview-${section}`);
      if (!(target instanceof HTMLElement)) return;
      const rootRect = root.getBoundingClientRect();
      const nextTop = root.scrollTop + target.getBoundingClientRect().top - rootRect.top;
      root.scrollTo({ top: Math.max(0, section === "status" ? 0 : nextTop - 12), behavior: "smooth" });
    };
    requestAnimationFrame(scroll);
  }

  useEffect(() => {
    if (tab !== "overview") return;
    const root = contentRef.current;
    if (!root) return;
    const sections = overviewLinks.map((item) => root.querySelector(`#overview-${item.id}`)).filter((node): node is HTMLElement => node instanceof HTMLElement);
    const onScroll = () => {
      const rootRect = root.getBoundingClientRect();
      const marker = rootRect.top + 18;
      let current = overviewLinks[0]?.id ?? "status";
      const firstTop = sections[0]?.getBoundingClientRect().top ?? marker;
      for (const section of sections) {
        const top = section.getBoundingClientRect().top;
        if (Math.abs(top - firstTop) <= 8) continue;
        if (top - marker > 12) break;
        current = section.id.replace("overview-", "") as typeof current;
      }
      setOverviewSection(current);
    };
    onScroll();
    root.addEventListener("scroll", onScroll, { passive: true });
    return () => root.removeEventListener("scroll", onScroll);
  }, [tab, project.id, readmeLoading, agentsLoading]);

  useEffect(() => {
    if (project.vcsKind !== "git") return;
    if (tab !== "git" && tab !== "overview") return;
    void loadGit();
  }, [project.id, project.vcsKind, tab]);

  useEffect(() => {
    if (tab !== "overview") return;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      if (cancelled) return;
      setEnvironmentLoading(true);
      setEnvironmentError(undefined);
      void api.inspectProjectEnvironment(project.id).then((value) => {
        if (!cancelled) setEnvironment(value);
      }).catch((error) => {
        if (!cancelled) setEnvironmentError(String(error));
      }).finally(() => {
        if (!cancelled) setEnvironmentLoading(false);
      });
    }, 180);
    return () => { cancelled = true; window.clearTimeout(timer); };
  }, [project.id, tab]);

  async function openAtlasReport() {
    setReportOpen(true);
    setReportLoading(true);
    try {
      const next = await api.atlasReport(project.id);
      setReport({ markdown: next.markdown, generatedAt: next.generatedAt });
    } catch (error) {
      notify("error", t("atlasReportFailed"), String(error));
      setReportOpen(false);
    } finally {
      setReportLoading(false);
    }
  }

  async function exportAtlasReport() {
    const path = await save({
      defaultPath: `${project.displayName.replace(/[\\/:*?\"<>|]+/g, "-")}-atlas-report.md`,
      filters: [{ name: "Markdown", extensions: ["md"] }],
    });
    if (!path) return;
    try {
      await api.exportAtlasReportTo(project.id, path);
      notify("success", t("atlasReportExported"), path);
    } catch (error) {
      notify("error", t("atlasReportExportFailed"), String(error));
    }
  }

  const tabs = [
    { id: "overview" as const, label: t("overview"), icon: BookOpenText },
    { id: "files" as const, label: t("files"), icon: FilesIcon },
    ...(project.vcsKind === "git" ? [{ id: "git" as const, label: t("git"), icon: GitBranch }] : []),
    { id: "tasks" as const, label: t("tasks"), icon: Play },
  ];

  return (
    <main id="main-content" className={tab === "tasks" ? "main-pane main-pane-tasks" : tab === "files" ? "main-pane main-pane-files" : "main-pane"}>
      <header className="project-header">
        <div className="atlas-texture" aria-hidden="true" />
        <div className="project-hero">
          <div className="project-identity">
            <h1>{project.displayName}</h1>
            <button className="hero-path" title={project.canonicalPath} onClick={() => void navigator.clipboard.writeText(project.canonicalPath).then(() => notify("success", t("copied"), project.canonicalPath)).catch((error) => notify("error", t("copyFailed"), String(error)))}><code>{project.canonicalPath}</code><Copy aria-label={t("copyPath")} /></button>
            <button className={`project-description ${project.description ? "has-description" : "is-empty"}`} onClick={() => { setDescriptionDraft(project.description ?? ""); setDescriptionOpen(true); }}><span>{project.description || t("addDescription")}</span><PencilSimple aria-hidden="true" /></button>
          </div>
          <div className="project-hero-side">
            <dl className="project-quick-facts" aria-label={t("projectBasics")}>
              <div><dt>{t("projectStatus")}</dt><dd className={project.availability === "ready" ? "is-ready" : "is-warning"}>{t(project.availability === "ready" ? "ready" : "unavailable")}</dd></div>
              <div><dt>{project.vcsKind === "git" ? "Git" : project.vcsKind.toUpperCase()}</dt><dd>{git?.snapshot.branch ?? detail.git?.branch ?? "—"}</dd></div>
              <div><dt>{t("projectStack")}</dt><dd title={stackOf(project).join(" · ")}>{stackOf(project).slice(0, 3).join(" · ") || "—"}</dd></div>
              <div><dt>{t("projectTaskCount")}</dt><dd>{detail.tasks.length}</dd></div>
            </dl>
            <div className="hero-actions">
            <ActionMenu trigger={<Button variant="primary" size="md"><Sparkle weight="bold" />{t("openAgent")}<CaretDown /></Button>} items={[
              ...(agentTools.length
                ? agentTools.map((agent) => ({
                    label: agent.name,
                    icon: <AgentGlyph agent={agent.id} />,
                    onClick: () => onOpenAgent(agent.id),
                  }))
                : [{
                    label: t("noAgentDetected"),
                    onClick: () => notify("warning", t("noAgentDetected"), t("agentNotInstalled")),
                  }]),
            ]} />
            {terminals.length > 1 ? (
              <ActionMenu trigger={<Button><TerminalWindow />{t("openTerminal")}<CaretDown /></Button>} items={terminals.map((terminal) => ({
                label: terminal.name,
                icon: <TerminalGlyph terminal={terminal.id} />,
                onClick: () => onOpenTerminal(terminal.id),
              }))} />
            ) : (
              <Button onClick={() => onOpenTerminal(terminals[0]?.id)}><TerminalWindow />{t("openTerminal")}</Button>
            )}
            <ActionMenu trigger={<Button><Code weight="bold" />{t("openIde")}<CaretDown /></Button>} items={[
              ...(ides.length
                ? ides.map((ide) => ({
                    label: ide.name,
                    icon: <IdeGlyph ide={ide.id} />,
                    onClick: () => onOpenIde(ide.id),
                  }))
                : [{
                    label: t("noIdeDetected"),
                    onClick: () => notify("warning", t("noIdeDetected"), t("ideNotInstalled")),
                  }]),
            ]} />
            <Button onClick={onOpenExplorer}><FolderOpen />{t("openExplorer")}</Button>
            <Button size="icon" aria-label={project.favorite ? t("unfavorite") : t("favorite")} onClick={() => void onFavorite()}><Star weight={project.favorite ? "fill" : "regular"} /></Button>
            <ActionMenu trigger={<Button size="icon" aria-label={t("moreActions")}><DotsThree weight="bold" /></Button>} items={[
              { label: t("refresh"), onClick: () => void onRefresh() },
              { label: t("atlasReport"), onClick: () => void openAtlasReport() },
              { label: t("relocateProject"), onClick: () => onRelocate?.() },
              { label: project.archived ? t("unarchive") : t("archive"), onClick: () => void onArchive() },
              { label: t("removeRecord"), onClick: () => setRemoveOpen(true), danger: true },
            ]} />
            </div>
          </div>
        </div>
        <div className="project-tabs-row">
          <nav className="project-tabs" aria-label={t("projectSections")} role="tablist">
            {tabs.map((item) => { const Icon = item.icon; const active = tab === item.id; return <button id={`project-tab-${item.id}`} role="tab" aria-selected={active} aria-controls={`project-panel-${item.id}`} tabIndex={active ? 0 : -1} key={item.id} className={active ? "active" : ""} onKeyDown={moveTabFocus} onClick={() => { setTab(item.id); if (item.id === "files") setFilesActivated(true); }}>{active && <motion.span className="tab-indicator" layoutId="project-tab" />}<Icon weight={active ? "fill" : "regular"} /><span>{item.label}</span></button>; })}
          </nav>
          {tab === "overview" && (
            <div className="overview-switcher">
              <ActionMenu
                trigger={<Button variant="quiet" aria-label={t("overview")}><span className="overview-switcher-index">{String(Math.max(overviewLinks.findIndex((item) => item.id === overviewSection) + 1, 1)).padStart(2, "0")}{t("overviewSectionOf")}{String(overviewLinks.length).padStart(2, "0")}</span><span>{overviewLinks.find((item) => item.id === overviewSection)?.label ?? t("overview")}</span><CaretDown /></Button>}
                items={overviewLinks.map((item) => ({ label: item.index + " · " + item.label, onClick: () => scrollToOverview(item.id) }))}
              />
            </div>
          )}
        </div>
      </header>
      <div id="project-panel-files" role="tabpanel" aria-labelledby="project-tab-files" className="workspace-content workspace-content-files" hidden={tab !== "files"}>
        {filesActivated && <FilesWorkspace project={project} active={tab === "files"} t={t} notify={notify} />}
      </div>
      {tab === "tasks" ? (
        <div id="project-panel-tasks" role="tabpanel" aria-labelledby="project-tab-tasks" className="workspace-content workspace-content-tasks">
          <TaskWorkspace tasks={detail.tasks} runs={runs} loading={runsLoading} error={runsError} activeRunId={activeRunId} log={(activeRunId ? logs[activeRunId] : "") ?? ""} t={t} notify={notify} onRetry={() => void loadRuns()} onSaveTasks={async (tasks) => { await api.updateProject(project.id, { tasks }); await onRefresh(); }} onRun={setPendingTaskId} onStopRun={async (runId) => { try { await api.stopTask(runId); await loadRuns(); } catch (error) { notify("error", t("taskStopFailed"), String(error)); } }} onClearLog={() => activeRunId && setLogs((current) => ({ ...current, [activeRunId]: "" }))} onHistory={async (run) => { setActiveRunId(run.id); try { const output = await api.readTaskLog(run.id); setLogs((current) => ({ ...current, [run.id]: output })); } catch (error) { notify("error", t("logLoadFailed"), String(error)); } }} />
        </div>
      ) : tab !== "files" ? (
        <div id={`project-panel-${tab}`} role="tabpanel" aria-labelledby={`project-tab-${tab}`} className="workspace-scroll" ref={contentRef}>
          <div className="workspace-content" key={tab}>
            {tab === "overview" && <OverviewWorkspace detail={detail} git={git} readme={readme} readmeLoading={readmeLoading} readmeError={readmeError} onRetryReadme={() => { readmeSequence.current += 1; setReadmeError(undefined); setReadmeLoading(true); api.readProjectReadme(project.id).then(setReadme).catch((error) => setReadmeError(String(error))).finally(() => setReadmeLoading(false)); }} agents={agents} agentsLoading={agentsLoading} agentsMissing={agentsMissing} onNeedAgents={() => {
              if (agentsRequested.current === project.id || agents || agentsLoading || agentsMissing) return;
              agentsRequested.current = project.id;
              const sequence = ++agentsSequence.current;
              setAgentsLoading(true); setAgentsMissing(false);
              api.readProjectDocument(project.id, "AGENTS.md").then((value) => { if (sequence === agentsSequence.current) setAgents(value); }).catch(() => { if (sequence === agentsSequence.current) setAgentsMissing(true); }).finally(() => { if (sequence === agentsSequence.current) setAgentsLoading(false); });
            }} t={t} tagDraft={tagDraft} setTagDraft={setTagDraft} onNotes={onNotes} onTags={onTags} tasks={detail.tasks} runs={runs} environment={environment} environmentLoading={environmentLoading} environmentError={environmentError} onRetryEnvironment={() => { setEnvironmentError(undefined); setEnvironmentLoading(true); api.inspectProjectEnvironment(project.id).then(setEnvironment).catch((error) => setEnvironmentError(String(error))).finally(() => setEnvironmentLoading(false)); }} onRunTask={setPendingTaskId} onOpenProject={onOpenProject} onPreviewFile={(file) => { setPreviewFile(file); setPreviewError(undefined); setPreviewLoading(true); api.readProjectFile(project.id, file.path).then(setPreviewDoc).catch((error) => { setPreviewDoc(undefined); setPreviewError(String(error)); }).finally(() => setPreviewLoading(false)); }} />}
            {tab === "git" && <GitWorkspace git={git} loading={gitLoading} error={gitError} busy={gitBusy} commitMessage={commitMessage} setCommitMessage={setCommitMessage} stagedCount={stagedCount} selectedDiff={selectedDiff} diff={diff} diffLoading={diffLoading} t={t} onRetry={() => void loadGit()} onGit={setPendingGit} onDiff={loadDiff} />}
          </div>
        </div>
      ) : null}
      <ConfirmDialog open={Boolean(pendingGit)} title={t("confirmGit")} body={t("confirmGitBody")} confirmLabel={t("run")} cancelLabel={t("cancel")} busy={gitBusy} onOpenChange={(open) => !open && !gitBusy && setPendingGit(null)} onConfirm={confirmGit} />
      <ConfirmDialog open={Boolean(pendingTaskId)} title={t("confirmTaskRun")} body={pendingTaskId ? taskConfirmation(detail.tasks.find((item) => item.id === pendingTaskId), project.canonicalPath, t) : ""} confirmLabel={t("run")} cancelLabel={t("cancel")} busy={taskBusy} onOpenChange={(open) => !open && !taskBusy && setPendingTaskId(undefined)} onConfirm={() => pendingTaskId ? runTask(pendingTaskId) : undefined} />
      <ConfirmDialog open={removeOpen} title={t("confirmRemove")} body={t("removeRecordHint")} confirmLabel={t("removeRecord")} cancelLabel={t("cancel")} onOpenChange={setRemoveOpen} onConfirm={() => void onRemove()} />
      <Dialog.Root open={Boolean(previewFile)} onOpenChange={(open) => { if (!open) { setPreviewFile(null); setPreviewDoc(undefined); setPreviewError(undefined); } }}>
        <Dialog.Portal>
          <Dialog.Backdrop className="dialog-backdrop" />
          <Dialog.Popup className="dialog-popup file-preview-dialog">
            <Dialog.Title className="dialog-title">{previewFile?.path ?? t("previewFile")}</Dialog.Title>
            {previewFile && <p className="muted-copy">{previewFile.source}</p>}
            {previewLoading ? <Skeleton className="skeleton-code" /> : previewError ? <InlineLoadError title={t("filePreviewFailed")} detail={previewError} retryLabel={t("retry")} onRetry={() => previewFile && api.readProjectFile(project.id, previewFile.path).then(setPreviewDoc).catch((error) => setPreviewError(String(error)))} /> : <pre className="file-preview-code">{previewDoc?.content ?? ""}</pre>}
            {previewDoc?.truncated && <span className="status-warning">{t("fileTruncated")}</span>}
            <div className="dialog-actions">
              <Button onClick={() => previewFile && void navigator.clipboard.writeText(previewFile.path).then(() => notify("success", t("copied"), previewFile.path))}>{t("copyPath")}</Button>
              <Button onClick={() => previewFile && void api.revealProjectFile(project.id, previewFile.path).catch((error) => notify("error", t("openFailed"), String(error)))}>{t("revealInExplorer")}</Button>
              <Button onClick={() => previewFile && void api.openProjectFile(project.id, previewFile.path).catch((error) => notify("error", t("openFailed"), String(error)))}>{t("openExternally")}</Button>
              <Dialog.Close render={<Button>{t("close")}</Button>} />
            </div>
          </Dialog.Popup>
        </Dialog.Portal>
      </Dialog.Root>
      <DescriptionDialog open={descriptionOpen} value={descriptionDraft} saving={descriptionSaving} title={t("editDescription")} label={t("projectDescription")} placeholder={t("descriptionHint")} saveLabel={t("save")} cancelLabel={t("cancel")} onValue={setDescriptionDraft} onOpenChange={setDescriptionOpen} onSave={async () => { setDescriptionSaving(true); try { await onDescription(descriptionDraft.trim() || null); setDescriptionOpen(false); } finally { setDescriptionSaving(false); } }} />
      <Dialog.Root open={reportOpen} onOpenChange={setReportOpen}>
        <Dialog.Portal>
          <Dialog.Backdrop className="dialog-backdrop" />
          <Dialog.Popup className="dialog-popup file-preview-dialog">
            <Dialog.Title className="dialog-title">{t("atlasReport")}</Dialog.Title>
            {reportLoading ? <Skeleton className="skeleton-code" /> : <div className="atlas-report-preview"><MarkdownDocument content={report?.markdown ?? ""} /></div>}
            <div className="dialog-actions">
              <Button onClick={() => report && void navigator.clipboard.writeText(report.markdown).then(() => notify("success", t("copied")))}>{t("copyConfig")}</Button>
              <Button variant="primary" disabled={!report || reportLoading} onClick={() => void exportAtlasReport()}>{t("exportAtlasReport")}</Button>
              <Dialog.Close render={<Button>{t("close")}</Button>} />
            </div>
          </Dialog.Popup>
        </Dialog.Portal>
      </Dialog.Root>
    </main>
  );
}

function moveTabFocus(event: KeyboardEvent<HTMLButtonElement>) {
  if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
  const tabs = Array.from(event.currentTarget.parentElement?.querySelectorAll<HTMLButtonElement>('[role="tab"]') ?? []);
  if (tabs.length === 0) return;
  event.preventDefault();
  const current = tabs.indexOf(event.currentTarget);
  const next = event.key === "Home" ? 0 : event.key === "End" ? tabs.length - 1 : (current + (event.key === "ArrowRight" ? 1 : -1) + tabs.length) % tabs.length;
  tabs[next]?.focus();
  tabs[next]?.click();
}

export { OverviewWorkspace, GitWorkspace, TaskWorkspace, MarkdownDocument };
