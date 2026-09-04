import { Tooltip } from "@base-ui/react/tooltip";
import { Dialog } from "@base-ui/react/dialog";
import { open } from "@tauri-apps/plugin-dialog";
import { MotionConfig } from "framer-motion";
import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore, type CSSProperties } from "react";
import { CommandPalette } from "./components/CommandPalette";
import { ProjectList, type ProjectFilters } from "./components/ProjectList";
import { TitleBar } from "./components/TitleBar";
import { OnboardingDialog, type OnboardingStep } from "./components/OnboardingDialog";
import { SplashScreen } from "./components/SplashScreen";
import { TaskWorkbench } from "./components/TaskWorkbench";
import { CollectionControls } from "./components/CollectionControls";
import { setFormatLocale } from "./lib/format";
import { Button } from "./components/ui/button";
import { PaneResizer, clampListWidth, readStoredListWidth } from "./components/ui/pane-resizer";
import { AppSkeleton, EmptyState, ToastViewport } from "./components/ui/feedback";
import { dictionaries, resolveLocale, type Locale, type MessageKey } from "./i18n";
import { api, onAttentionChanged, onScanCompleted, onScanFailed, onScanProgress, onTaskExited, onTaskPersistenceFailed } from "./lib/api";
import { getActiveTaskRunsSnapshot, openTaskRun, refreshTaskRuns, startTaskRunListeners, subscribeActiveTaskRuns } from "./lib/task-runs";
import { isSameOrAncestorPath, scanRootsUnderPath, type ProjectSort } from "./lib/project-tree";
import { updaterService } from "./lib/updater";
import { buildAgentScanInstruction } from "./lib/mcp-setup";
import { readOnboardingState, resolveOnboardingVisibility, useOnboardingProjectWatch, writeOnboardingState } from "./lib/onboarding";
import type { AppSettings, AppView, AttentionItem, McpSetupInfo, PendingApproval, ProjectCollection, ProjectDetail, ProjectQuery, ProjectScope, ProjectSummary, ScanProgress, ScanRoot, ToastMessage, ToastTone } from "./types";

const Dashboard = lazy(() => import("./components/Dashboard").then(({ Dashboard }) => ({ default: Dashboard })));
const HomeDashboard = lazy(() => import("./components/HomeDashboard").then(({ HomeDashboard }) => ({ default: HomeDashboard })));
const LazyHelpPage = lazy(() => import("./components/HelpPage").then(({ HelpPage }) => ({ default: HelpPage })));
const SettingsPane = lazy(() => import("./components/SettingsPane").then(({ SettingsPane }) => ({ default: SettingsPane })));

const systemLocale: Locale = navigator.language.toLowerCase().startsWith("zh") ? "zh" : "en";
const emptyFilters: ProjectFilters = { language: "", tag: "" };

function queryForScope(scope: ProjectScope, search = "", collectionId?: string): ProjectQuery {
  return { section: scope === "projects" ? undefined : scope, search: search || undefined, includeArchived: scope === "archived", collectionId };
}

export default function App() {
  const media = useMemo(() => window.matchMedia("(prefers-color-scheme: light)"), []);
  const [systemTheme, setSystemTheme] = useState<"light" | "dark">(media.matches ? "light" : "dark");
  const [settings, setSettings] = useState<AppSettings>({ theme: "system", locale: "system", uiFont: "", consoleFont: "" });
  const [view, setView] = useState<Exclude<AppView, "settings">>("library");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [scope, setScope] = useState<ProjectScope>("projects");
  const [scanRoots, setScanRoots] = useState<ScanRoot[]>([]);
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [scopeProjects, setScopeProjects] = useState<ProjectSummary[]>([]);
  const [collections, setCollections] = useState<ProjectCollection[]>([]);
  const [selectedCollectionId, setSelectedCollectionId] = useState<string>();
  const [librarySurface, setLibrarySurface] = useState<"dashboard" | "project">("dashboard");
  const [selectedId, setSelectedId] = useState<string>();
  const [detail, setDetail] = useState<ProjectDetail | null>(null);
  const [query, setQuery] = useState("");
  const [filters, setFilters] = useState<ProjectFilters>(emptyFilters);
  const [sort, setSort] = useState<ProjectSort>("default");
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [approvalsOpen, setApprovalsOpen] = useState(false);
  const [approvals, setApprovals] = useState<PendingApproval[]>([]);
  const [attentionItems, setAttentionItems] = useState<AttentionItem[]>([]);
  const [transientAttention, setTransientAttention] = useState<AttentionItem[]>([]);
  const [attentionError, setAttentionError] = useState<string>();
  const [resolvingApproval, setResolvingApproval] = useState<string>();
  const activeTaskRuns = useSyncExternalStore(subscribeActiveTaskRuns, getActiveTaskRunsSnapshot, getActiveTaskRunsSnapshot);
  const [activeTasksOpen, setActiveTasksOpen] = useState(false);
  const [stoppingRunId, setStoppingRunId] = useState<string>();
  const [stoppingAll, setStoppingAll] = useState(false);
  const [nowTick, setNowTick] = useState(() => Date.now());
  const [scanning, setScanning] = useState(false);
  const [progress, setProgress] = useState<ScanProgress | null>(null);
  const [booting, setBooting] = useState(true);
  const [bootError, setBootError] = useState<string>();
  const [splashDone, setSplashDone] = useState(false);
  const [projectLoading, setProjectLoading] = useState(false);
  const [toasts, setToasts] = useState<ToastMessage[]>([]);
  const updateState = useSyncExternalStore(updaterService.subscribe, updaterService.getSnapshot, updaterService.getSnapshot);
  const booted = useRef(false);
  const updaterChecked = useRef(false);
  const onboardingResolved = useRef(false);
  const querySequence = useRef(0);
  const detailSequence = useRef(0);
  const scanningRef = useRef(false);
  const [onboardingOpen, setOnboardingOpen] = useState(false);
  const [onboardingStep, setOnboardingStep] = useState<OnboardingStep>("welcome");
  const onboardingOpenRef = useRef(false);
  const onboardingStepRef = useRef<OnboardingStep>("welcome");
  const [mcpInfo, setMcpInfo] = useState<McpSetupInfo>();
  const [mcpError, setMcpError] = useState<string>();
  const [onboardingRootId, setOnboardingRootId] = useState<string>();
  const [onboardingScanOutcome, setOnboardingScanOutcome] = useState<"success" | "empty" | "cancelled" | "failed">();
  const [onboardingScanError, setOnboardingScanError] = useState<string>();
  const [onboardingBaseline, setOnboardingBaseline] = useState<string[] | null>(null);
  const [foundOnboardingProject, setFoundOnboardingProject] = useState<ProjectSummary | null>(null);
  const [onboardingCheckFailed, setOnboardingCheckFailed] = useState(false);
  const scopeCache = useRef<{ scope: ProjectScope; collectionId?: string; projects: ProjectSummary[] } | undefined>(undefined);
  const locale = resolveLocale(settings.locale, systemLocale);
  const t = useCallback((key: MessageKey) => dictionaries[locale][key], [locale]);
  setFormatLocale(locale);
  const theme: "light" | "dark" = settings.theme === "light" ? "light" : settings.theme === "dark" ? "dark" : systemTheme;
  onboardingOpenRef.current = onboardingOpen;
  onboardingStepRef.current = onboardingStep;

  useEffect(() => {
    const suppressNativeContextMenu = (event: MouseEvent) => {
      const target = event.target;
      if (target instanceof Element && target.closest("[data-repoatlas-context-menu]")) return;
      event.preventDefault();
    };
    document.addEventListener("contextmenu", suppressNativeContextMenu);
    return () => document.removeEventListener("contextmenu", suppressNativeContextMenu);
  }, []);

  const notify = useCallback((tone: ToastTone, title: string, detail?: string) => {
    const id = globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random()}`;
    setToasts((current) => [...current.slice(-3), { id, tone, title, detail }]);
    window.setTimeout(() => setToasts((current) => current.filter((item) => item.id !== id)), tone === "error" ? 8000 : 4800);
  }, []);

  const loadProjectDetail = useCallback(async (id?: string) => {
    const sequence = ++detailSequence.current;
    if (!id) { setSelectedId(undefined); setDetail(null); setProjectLoading(false); return; }
    setSelectedId(id);
    setProjectLoading(true);
    try {
      const next = await api.getProject(id);
      if (sequence === detailSequence.current) setDetail(next);
    } catch (error) {
      if (sequence === detailSequence.current) { setDetail(null); notify("error", t("projectLoadFailed"), String(error)); }
    } finally {
      if (sequence === detailSequence.current) setProjectLoading(false);
    }

  }, [notify, t]);

  const reload = useCallback(async (nextSelected?: string, collectionOverride?: string | null) => {
    const sequence = ++querySequence.current;
    try {
      const effectiveCollectionId = collectionOverride === undefined ? selectedCollectionId : collectionOverride ?? undefined;
      const baseQuery = queryForScope(scope, "", effectiveCollectionId);
      const [nextRoots, nextScopeProjects, nextCollections] = await Promise.all([
        api.listScanRoots(), api.listProjects(baseQuery), api.listCollections(),
      ]);
      const nextProjects = query.trim()
        ? await api.listProjects(queryForScope(scope, query, effectiveCollectionId))
        : nextScopeProjects;
      if (sequence !== querySequence.current) return;
      scopeCache.current = { scope, collectionId: effectiveCollectionId, projects: nextScopeProjects };
      setScanRoots(nextRoots); setScopeProjects(nextScopeProjects); setProjects(nextProjects); setCollections(nextCollections);
      if (nextSelected) {
        setLibrarySurface("project");
        await loadProjectDetail(nextSelected);
      } else if (librarySurface === "project") {
        const id = nextProjects.some((project) => project.id === selectedId) ? selectedId : nextProjects[0]?.id;
        await loadProjectDetail(id);
      }
    } catch (error) { notify("error", t("reloadFailed"), String(error)); }
  }, [librarySurface, loadProjectDetail, notify, query, scope, selectedCollectionId, selectedId, t]);

  const updateCollectionMembership = useCallback(async (projectId: string, collectionId: string, include: boolean) => {
    try {
      const current = await api.collectionMemberIds(collectionId);
      const next = include
        ? [...new Set([...current, projectId])]
        : current.filter((id) => id !== projectId);
      if (next.length !== current.length) await api.setCollectionMembers(collectionId, next);
      notify("success", include ? t("projectAddedToCollection") : t("projectRemovedFromCollection"));
      await reload(include ? projectId : undefined, selectedCollectionId ?? null);
    } catch (error) {
      notify("error", t("collectionMembershipFailed"), String(error));
    }
  }, [notify, reload, selectedCollectionId, t]);

  useEffect(() => {
    const onTheme = (event: MediaQueryListEvent) => setSystemTheme(event.matches ? "light" : "dark");
    media.addEventListener("change", onTheme); return () => media.removeEventListener("change", onTheme);
  }, [media]);

  useEffect(() => {
    setFormatLocale(locale);
    document.documentElement.dataset.theme = theme;
    document.documentElement.lang = locale === "zh" ? "zh-CN" : "en";
    document.documentElement.style.setProperty("--ui-font", settings.uiFont?.trim() ? `"${settings.uiFont.trim()}", var(--font-sans)` : "var(--font-sans)");
    document.documentElement.style.setProperty("--console-font", settings.consoleFont?.trim() ? `"${settings.consoleFont.trim()}", "${settings.consoleFont.trim()} CN", var(--font-console)` : "var(--font-console)");
    const skipLink = document.querySelector<HTMLElement>(".skip-link");
    if (skipLink) skipLink.textContent = t("skipToContent");
    document.querySelector('meta[name="theme-color"]')?.setAttribute("content", theme === "light" ? "#F6F6F6" : "#0D0D0D");
  }, [locale, settings.consoleFont, settings.uiFont, t, theme]);

  const bootstrapApp = useCallback(async () => {
    setBooting(true); setBootError(undefined);
    try {
      const boot = await api.bootstrap();
      setSettings(boot.settings); setScanRoots(boot.scanRoots); setProjects(boot.projects); setScopeProjects(boot.projects); setCollections(boot.collections ?? []);
      scopeCache.current = { scope: "projects", projects: boot.projects };
      setLibrarySurface("dashboard");
      await loadProjectDetail();
      if (!onboardingResolved.current) {
        onboardingResolved.current = true;
        const decision = resolveOnboardingVisibility({
          stored: readOnboardingState(),
          projectCount: boot.projects.length,
          scanRootCount: boot.scanRoots.length,
        });
        if (decision.write) writeOnboardingState(decision.write);
        if (decision.open) {
          setOnboardingOpen(true);
          setOnboardingStep("welcome");
        }
      }
    } catch (error) {
      const detail = String(error); setBootError(detail); notify("error", t("startupFailed"), detail);
    } finally { setBooting(false); }
  }, [loadProjectDetail, notify, t]);

  useEffect(() => {
    if (booted.current) return;
    booted.current = true;
    void bootstrapApp();
  }, [bootstrapApp]);

  useEffect(() => {
    if (booting || bootError || updaterChecked.current) return;
    updaterChecked.current = true;
    // Update discovery is deliberately independent from bootstrap. A failed
    // or offline GitHub request must never prevent the local library opening.
    void updaterService.checkForUpdates();
  }, [bootError, booting]);

  useEffect(() => {
    if (!onboardingOpen) return;
    let active = true;
    api.mcpSetupInfo().then((value) => {
      if (!active) return;
      setMcpInfo(value);
      setMcpError(undefined);
    }).catch((error) => {
      if (!active) return;
      setMcpError(String(error));
    });
    return () => { active = false; };
  }, [onboardingOpen]);

  useEffect(() => {
    if (!onboardingRootId && scanRoots[0]) setOnboardingRootId(scanRoots[0].id);
  }, [onboardingRootId, scanRoots]);

  const loadApprovals = useCallback(async () => {
    try {
      const center = await api.getAttentionCenter();
      setApprovals(center.approvals);
      setAttentionItems(center.items);
      setAttentionError(undefined);
    } catch (error) {
      setAttentionError(String(error));
    }
  }, []);

  const returnToLibrary = useCallback(() => {
    setSettingsOpen(false);
    setView("library");
    setLibrarySurface("dashboard");
  }, []);

  const openSettings = useCallback(() => {
    setView("library");
    setSettingsOpen(true);
  }, []);

  const closeSettings = useCallback(() => {
    setSettingsOpen(false);
  }, []);

  const openHelp = useCallback(() => {
    setSettingsOpen(false);
    setView("help");
  }, []);

  const closeHelp = useCallback(() => {
    setView("library");
  }, []);

  const loadActiveRuns = useCallback(async () => {
    try {
      await refreshTaskRuns();
    } catch { /* the running-tasks badge is non-blocking. */ }
  }, []);

  useEffect(() => {
    if (booting) return;
    void loadApprovals();
    const timer = window.setInterval(() => void loadApprovals(), 30000);
    const onFocus = () => void loadApprovals();
    window.addEventListener("focus", onFocus);
    return () => { window.clearInterval(timer); window.removeEventListener("focus", onFocus); };
  }, [booting, loadApprovals]);

  useEffect(() => {
    if (booting) return;
    void loadActiveRuns();
  }, [booting, loadActiveRuns]);

  useEffect(() => {
    void startTaskRunListeners();
  }, []);

  useEffect(() => {
    const listeners = Promise.all([
      onAttentionChanged(() => void loadApprovals()),
      onTaskExited(() => void loadApprovals()),
      onTaskPersistenceFailed((failure) => {
        setTransientAttention((current) => [{
          id: `persistence:${failure.runId}:${Date.now()}`,
          kind: "system_failure",
          severity: "error",
          projectId: failure.run?.projectId ?? null,
          runId: failure.runId,
          approvalId: null,
          title: t("taskPersistenceFailed"),
          detail: failure.error,
          occurredAt: new Date().toISOString(),
          sourceVersion: new Date().toISOString(),
          acknowledgeable: false,
        }, ...current].slice(0, 10));
      }),
    ]);
    return () => { void listeners.then((items) => items.forEach((dispose) => dispose())); };
  }, [loadApprovals, t]);

  useEffect(() => {
    if (!activeTasksOpen) return;
    void loadActiveRuns();
    const tick = window.setInterval(() => setNowTick(Date.now()), 1000);
    return () => window.clearInterval(tick);
  }, [activeTasksOpen, loadActiveRuns]);

  async function stopRun(runId: string) {
    setStoppingRunId(runId);
    try {
      await api.stopTask(runId);
      notify("success", t("taskStopped"));
      await loadActiveRuns();
    } catch (error) { notify("error", t("taskStopFailed"), String(error)); }
    finally { setStoppingRunId(undefined); }
  }

  async function stopAllRuns() {
    setStoppingAll(true);
    try {
      await Promise.all(activeTaskRuns.map((run) => api.stopTask(run.id)));
      notify("success", t("taskStopped"));
      await loadActiveRuns();
    } catch (error) { notify("error", t("taskStopFailed"), String(error)); }
    finally { setStoppingAll(false); }
  }

  async function resolveApproval(approval: PendingApproval, approved: boolean) {
    if (resolvingApproval) return;
    setResolvingApproval(approval.id);
    try {
      let allowPortConflicts = false;
      if (approved && approval.taskId) {
        const conflicts = await api.preflightTaskPorts(approval.projectId, approval.taskId);
        allowPortConflicts = conflicts.length > 0 && window.confirm(`${t("portConflictConfirm")}\n${conflicts.map((item) => `${item.port}${item.processName ? ` · ${item.processName}` : ""}${item.pid ? ` · PID ${item.pid}` : ""}`).join("\n")}`);
        if (conflicts.length > 0 && !allowPortConflicts) return;
      }
      await api.resolvePendingApproval(approval.id, approved, allowPortConflicts);
      notify(approved ? "success" : "info", approved ? t("approvalStarted") : t("approvalDenied"), approval.title);
      await loadApprovals();
    } catch (error) {
      notify("error", t("approvalFailed"), String(error));
    } finally { setResolvingApproval(undefined); }
  }

  useEffect(() => {
    if (booting) return;
    const sequence = ++querySequence.current;
    const timer = window.setTimeout(() => {
      const baseQuery = queryForScope(scope, "", selectedCollectionId);
      const cached = scopeCache.current;
      const basePromise = cached?.scope === scope && cached.collectionId === selectedCollectionId
        ? Promise.resolve(cached.projects)
        : api.listProjects(baseQuery);
      const nextPromise = query.trim()
        ? api.listProjects(queryForScope(scope, query, selectedCollectionId))
        : basePromise;
      Promise.all([basePromise, nextPromise]).then(async ([base, next]) => {
        if (sequence !== querySequence.current) return;
        scopeCache.current = { scope, collectionId: selectedCollectionId, projects: base };
        setScopeProjects(base); setProjects(next);
        if (librarySurface === "project") {
          const id = next.some((project) => project.id === selectedId) ? selectedId : next[0]?.id;
          await loadProjectDetail(id);
        }
      }).catch((error) => notify("error", t("searchFailed"), String(error)));
    }, 180);
    return () => window.clearTimeout(timer);
  }, [booting, librarySurface, loadProjectDetail, notify, query, scope, selectedCollectionId, selectedId, t]);

  useEffect(() => {
    const unsubs = Promise.all([
      onScanProgress(setProgress),
      onScanCompleted(async (results) => {
        scanningRef.current = false; setScanning(false); setProgress(null);
        const summary = results.reduce((sum, result) => ({ visited: sum.visited + result.visited, discovered: sum.discovered + result.discovered, errors: sum.errors + result.errors.length, cancelled: sum.cancelled || result.cancelled }), { visited: 0, discovered: 0, errors: 0, cancelled: false });
        const errorDetails = results.flatMap((result) => result.errors).slice(0, 3);
        notify(summary.errors ? "warning" : "success", summary.cancelled ? t("scanCancelled") : t("scanComplete"), `${summary.discovered} ${t("found")} · ${summary.visited} ${t("visited")}${summary.errors ? ` · ${summary.errors} ${t("errors")}` : ""}${errorDetails.length ? `\n${errorDetails.join("\n")}` : ""}`);
        await reload();
        if (onboardingOpenRef.current && onboardingStepRef.current === "manual") {
          if (summary.cancelled) setOnboardingScanOutcome("cancelled");
          else if (summary.discovered > 0) {
            setOnboardingScanOutcome("success");
            completeOnboarding();
          } else setOnboardingScanOutcome("empty");
        }
      }),
      onScanFailed((message) => {
        scanningRef.current = false; setScanning(false); setProgress(null); notify("error", t("scanFailed"), message);
        if (onboardingOpenRef.current && onboardingStepRef.current === "manual") {
          setOnboardingScanOutcome("failed");
          setOnboardingScanError(message);
        }
      }),
    ]);
    return () => { void unsubs.then((functions) => functions.forEach((unsubscribe) => unsubscribe())); };
  }, [notify, reload, t]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.ctrlKey && !event.altKey && !event.metaKey && event.code === "Backquote") {
        if (event.repeat) return;
        event.preventDefault();
        setPaletteOpen(false);
        setActiveTasksOpen((open) => !open);
        return;
      }
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") { event.preventDefault(); setPaletteOpen(true); }
      if (event.key === "Escape") setPaletteOpen(false);
    };
    window.addEventListener("keydown", onKey, true); return () => window.removeEventListener("keydown", onKey, true);
  }, []);

  const visibleProjects = useMemo(() => projects.filter((project) => (!filters.language || project.languages.includes(filters.language)) && (!filters.tag || project.tags.includes(filters.tag))), [filters, projects]);
  const visibleAttentionItems = useMemo(() => {
    const all = [...transientAttention, ...attentionItems];
    if (!selectedCollectionId) return all;
    const projectIds = new Set(scopeProjects.map((project) => project.id));
    return all.filter((item) => !item.projectId || projectIds.has(item.projectId));
  }, [attentionItems, scopeProjects, selectedCollectionId, transientAttention]);
  useEffect(() => {
    if (librarySurface === "dashboard") return;
    if (projectLoading) return;
    if (visibleProjects.length === 0) { if (selectedId) void loadProjectDetail(); return; }
    if (!visibleProjects.some((project) => project.id === selectedId)) void loadProjectDetail(visibleProjects[0]?.id);
  }, [librarySurface, loadProjectDetail, projectLoading, selectedId, visibleProjects]);

  async function relocateProject(id: string) {
    const path = await open({ directory: true, multiple: false });
    if (typeof path !== "string") return;
    try {
      await api.relocateProject(id, path);
      notify("success", t("relocateProject"), path);
      await reload(id);
    } catch (error) {
      notify("error", t("saveFailed"), String(error));
    }
  }

  async function chooseDirectory(register: boolean) {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (typeof selected !== "string") return;
      if (register) { const project = await api.registerProject(selected); notify("success", t("projectRegistered"), project.displayName); await reload(project.id); }
      else {
        const root = await api.addScanRoot(selected);
        notify("success", t("rootAdded"), selected);
        await reload();
        if (scanningRef.current) notify("warning", t("scanAlreadyRunning"));
        else void startScan(root.id);
      }
    } catch (error) { notify("error", register ? t("registerFailed") : t("rootAddFailed"), String(error)); }
  }

  async function jumpToProject(id: string) {
    scopeCache.current = undefined;
    setSelectedCollectionId(undefined);
    setScope("projects");
    setQuery("");
    setFilters(emptyFilters);
    setView("library");
    setLibrarySurface("project");
    await loadProjectDetail(id);
  }

  async function openCollectionFromDashboard(collectionId: string) {
    const sequence = ++querySequence.current;
    try {
      const [nextRoots, nextProjects, nextCollections] = await Promise.all([
        api.listScanRoots(),
        api.listProjects(queryForScope("projects", "", collectionId)),
        api.listCollections(),
      ]);
      if (sequence !== querySequence.current) return;
      scopeCache.current = { scope: "projects", collectionId, projects: nextProjects };
      setScope("projects");
      setQuery("");
      setFilters(emptyFilters);
      setSelectedCollectionId(collectionId);
      setView("library");
      setScanRoots(nextRoots);
      setScopeProjects(nextProjects);
      setProjects(nextProjects);
      setCollections(nextCollections);
      const first = nextProjects[0];
      if (!first) {
        setLibrarySurface("dashboard");
        await loadProjectDetail();
        return;
      }
      setLibrarySurface("project");
      await loadProjectDetail(first.id);
      void api.markOpened(first.id).catch(() => undefined);
    } catch (error) {
      notify("error", t("collectionLoadFailed"), String(error));
    }
  }

  async function acknowledgeAttention(item: AttentionItem) {
    if (item.id.startsWith("persistence:")) {
      setTransientAttention((current) => current.filter((candidate) => candidate.id !== item.id));
      return;
    }
    try {
      await api.acknowledgeAttentionItem(item.id, item.sourceVersion);
      await loadApprovals();
    } catch (error) {
      notify("error", t("approvalFailed"), String(error));
    }
  }

  function openDashboardAttentionItem(item: AttentionItem) {
    if (item.approvalId) {
      setApprovalsOpen(true);
      void loadApprovals();
      return;
    }
    if (item.runId) {
      void openTaskRun(item.runId).then(() => setActiveTasksOpen(true)).catch((error) => notify("error", t("tasksLoadFailed"), String(error)));
      return;
    }
    if (item.projectId) {
      void jumpToProject(item.projectId);
      return;
    }
    setApprovalsOpen(true);
    void loadApprovals();
  }
  async function chooseOnboardingRoot() {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (typeof selected !== "string") return;
      const root = await api.addScanRoot(selected);
      notify("success", t("rootAdded"), selected);
      setOnboardingRootId(root.id);
      setOnboardingScanOutcome(undefined);
      setOnboardingScanError(undefined);
      await reload();
    } catch (error) {
      notify("error", t("rootAddFailed"), String(error));
    }
  }
  function completeOnboarding() {
    writeOnboardingState("completed");
    setOnboardingOpen(false);
    setOnboardingBaseline(null);
    setFoundOnboardingProject(null);
    setOnboardingCheckFailed(false);
    setView("library");
  }
  function skipOnboarding() {
    if (scanningRef.current) {
      notify("warning", t("onboardingCloseBlocked"));
      return;
    }
    completeOnboarding();
  }
  function replayOnboarding() {
    if (scanningRef.current) {
      notify("warning", t("onboardingReplayBusy"));
      return;
    }
    setFoundOnboardingProject(null);
    setOnboardingBaseline(null);
    setOnboardingCheckFailed(false);
    setOnboardingScanOutcome(undefined);
    setOnboardingScanError(undefined);
    setOnboardingStep("welcome");
    setOnboardingOpen(true);
    setView("library");
  }
  async function copyOnboardingInstruction() {
    const instruction = buildAgentScanInstruction(mcpInfo, locale);
    if (!instruction) return false;
    try {
      await navigator.clipboard.writeText(instruction);
    } catch {
      return false;
    }
    try {
      const records = await api.listProjects({ includeArchived: true });
      setOnboardingBaseline(records.map((project) => project.id));
    } catch {
      setOnboardingBaseline(projects.map((project) => project.id));
    }
    setFoundOnboardingProject(null);
    setOnboardingCheckFailed(false);
    return true;
  }
  const { checkNow } = useOnboardingProjectWatch({
    enabled: onboardingOpen && onboardingStep === "waiting" && !foundOnboardingProject,
    baselineIds: onboardingBaseline,
    listProjects: () => api.listProjects({ includeArchived: true }),
    onFound: (project) => {
      setFoundOnboardingProject(project);
      setOnboardingCheckFailed(false);
      void reload(project.id);
    },
    onError: () => setOnboardingCheckFailed(true),
  });
  function selectProject(id: string) { if (id === selectedId && librarySurface === "project") return; setView("library"); setLibrarySurface("project"); void loadProjectDetail(id); void api.markOpened(id).catch(() => undefined); }
  async function openProjectFromPalette(id: string) {
    try {
      const next = await api.getProject(id);
      detailSequence.current += 1;
      setView("library"); setScope(next.project.archived ? "archived" : "projects"); setQuery(""); setFilters(emptyFilters);
      setLibrarySurface("project");
      setSelectedId(id); setDetail(next); setProjectLoading(false);
      try { await api.markOpened(id); } catch { /* non-blocking */ }
    } catch (error) { notify("error", t("projectLoadFailed"), String(error)); }
  }
  async function startScan(rootId?: string) { if (scanningRef.current) return; scanningRef.current = true; setScanning(true); setProgress(null); try { await api.startScan(rootId); } catch (error) { scanningRef.current = false; setScanning(false); notify("error", t("scanFailed"), String(error)); } }
  async function cancelScan() { try { await api.cancelScan(); } catch (error) { notify("error", t("cancelFailed"), String(error)); } }
  async function updateSettings(next: AppSettings) { try { setSettings(await api.updateSettings(next)); notify("success", t("settingsSaved")); } catch (error) { notify("error", t("settingsSaveFailed"), String(error)); } }
  async function removeProjectRecord(id: string) {
    await api.removeProject(id);
    notify("success", t("recordRemoved"));
    await reload();
  }
  async function removeFolderRecords(path: string, projectIds: string[]) {
    const records = await api.listProjects({ includeArchived: true });
    const ids = [...new Set([
      ...projectIds,
      ...records.filter((project) => isSameOrAncestorPath(path, project.canonicalPath)).map((project) => project.id),
    ])];
    const rootIds = scanRootsUnderPath(scanRoots, path).map((root) => root.id);
    await api.removeFolderRecords(ids, rootIds);
    notify("success", t("folderRecordsRemoved"), path);
    await reload();
  }

  async function refreshCurrentProject() {
    if (!detail) return;
    try {
      await api.refreshProject(detail.project.id);
      await reload(detail.project.id);
    } catch (error) {
      notify("error", t("openFailed"), String(error));
    }
  }

  async function toggleFavoriteCurrent() {
    if (!detail) return;
    try {
      await api.updateProject(detail.project.id, { favorite: !detail.project.favorite });
      await reload(detail.project.id);
    } catch (error) {
      notify("error", t("saveFailed"), String(error));
    }
  }

  const paletteActions = [
    { id: "open-dashboard", title: t("dashboard"), run: returnToLibrary },
    { id: "open-help", title: t("openHelp"), run: openHelp }, { id: "open-settings", title: t("openSettings"), run: openSettings },
    { id: "scan-all", title: t("scanAll"), run: () => void startScan() }, { id: "add-root", title: t("addRoot"), run: () => void chooseDirectory(false) }, { id: "register", title: t("register"), run: () => void chooseDirectory(true) },
    { id: "open-tasks", title: t("activeTasks"), run: () => { setActiveTasksOpen(true); void loadActiveRuns(); } },
    { id: "open-approvals", title: t("attentionCenter"), run: () => { setApprovalsOpen(true); void loadApprovals(); } },
    { id: "clear-filters", title: t("clear"), run: () => { setQuery(""); setFilters(emptyFilters); } },
    { id: "check-updates", title: t("checkForUpdates"), run: () => { openSettings(); void updaterService.checkForUpdates(); } },
    ...(detail ? [
      { id: "refresh-project", title: t("refresh") + " · " + detail.project.displayName, run: () => void refreshCurrentProject() },
      { id: "reveal-project", title: t("revealInExplorer") + " · " + detail.project.displayName, run: () => void api.openInExplorer(detail.project.canonicalPath).catch((error) => notify("error", t("openFailed"), String(error))) },
      { id: "favorite-project", title: (detail.project.favorite ? t("unfavorite") : t("favorite")) + " · " + detail.project.displayName, run: () => void toggleFavoriteCurrent() },
    ] : []),
    ...visibleProjects.map((project) => ({ id: `jump:${project.id}`, title: `${t("jump")} · ${project.displayName}`, run: () => void selectProject(project.id) })),
  ];
  const dashboardRefreshKey = [
    ...projects.map((project) => `${project.id}:${project.updatedAt}`),
    ...collections.map((collection) => `${collection.id}:${collection.updatedAt}`),
    ...activeTaskRuns.map((run) => `${run.id}:${run.status}:${run.finishedAt ?? ""}`),
    ...attentionItems.map((item) => `${item.id}:${item.sourceVersion}`),
  ].join("|");
  const emptyMessage = scope === "favorites" ? t("noFavorites") : scope === "recent" ? t("noRecent") : scope === "archived" ? t("noArchived") : t("emptyBody");
  const [listWidth, setListWidth] = useState(() => readStoredListWidth(typeof window === "undefined" ? undefined : window.localStorage));
  const commitListWidth = useCallback((width: number) => {
    const next = clampListWidth(width);
    setListWidth(next);
    try {
      window.localStorage.setItem("repoatlas.sidebarWidth", String(next));
    } catch {
      // Storage may be unavailable; resizing still works for the session.
    }
  }, []);
  const workspaceStyle = { "--list-pane-width": `${listWidth}px` } as CSSProperties;

  return <MotionConfig reducedMotion="user" transition={{ duration: 0.18, ease: [0.2, 0.8, 0.2, 1] }}><Tooltip.Provider delay={300}><div className="app-shell">
    {!splashDone && <SplashScreen ready={!booting} onFinished={() => setSplashDone(true)} />}
    <TitleBar title={t("appName")} subtitle={t("appTag")} commandLabel={t("commandShort")} minimizeLabel={t("minimizeWindow")} maximizeLabel={t("maximizeWindow")} closeLabel={t("close")} helpLabel={t("help")} settingsLabel={t("settings")} approvalsLabel={t("attentionCenter")} approvalCount={attentionItems.length} tasksLabel={t("activeTasks")} tasksShortcut="Ctrl + `" tasksCount={activeTaskRuns.length} onTasks={() => { setActiveTasksOpen(true); void loadActiveRuns(); }} activeView={settingsOpen ? "settings" : view} onCommand={() => setPaletteOpen(true)} onLibrary={returnToLibrary} onHelp={openHelp} onSettings={openSettings} onApprovals={() => { setApprovalsOpen(true); void loadApprovals(); }} />
    <div className="workspace workspace-library" style={workspaceStyle}>
      <PaneResizer width={listWidth} label={t("resizeSidebar")} onWidthChange={setListWidth} onWidthCommit={commitListWidth} />
      <>
        <ProjectList projects={visibleProjects} allProjects={scopeProjects} selectedId={librarySurface === "project" ? selectedId : undefined} onSelect={(id) => void selectProject(id)} query={query} onQuery={setQuery} filters={filters} onFilters={setFilters} sort={sort} onSort={setSort} scope={scope} onScope={setScope} scanning={scanning} progress={progress} t={t} empty={emptyMessage} onAddRoot={() => void chooseDirectory(false)} onRegister={() => void chooseDirectory(true)} onScan={() => void startScan()} onCancelScan={() => void cancelScan()} collectionControls={<CollectionControls collections={collections} selectedId={selectedCollectionId} t={t} notify={notify} onSelect={(id) => { scopeCache.current = undefined; setSelectedCollectionId(id); }} onChanged={async (id) => { scopeCache.current = undefined; setSelectedCollectionId(id); await reload(undefined, id ?? null); }} />} collections={collections} selectedCollectionId={selectedCollectionId} onAddToCollection={(projectId, collectionId) => updateCollectionMembership(projectId, collectionId, true)} onRemoveFromCollection={(projectId, collectionId) => updateCollectionMembership(projectId, collectionId, false)} onRename={async (id, displayName) => { try { await api.updateProject(id, { displayName }); await reload(id); } catch (error) { notify("error", t("saveFailed"), String(error)); throw error; } }} onDescription={async (id, description) => { try { await api.updateProject(id, { description }); await reload(id); } catch (error) { notify("error", t("saveFailed"), String(error)); throw error; } }} scanRoots={scanRoots} onRemoveProject={async (id) => { try { await removeProjectRecord(id); } catch (error) { notify("error", t("removeRecordFailed"), String(error)); } }} onRemoveFolder={async (path, projectIds) => { try { await removeFolderRecords(path, projectIds); } catch (error) { notify("error", t("removeFolderFailed"), String(error)); } }} onIconError={(error) => notify("error", t("projectIconFailed"), String(error))} onReveal={(path) => void api.openInExplorer(path).catch((error) => notify("error", t("openFailed"), String(error)))} onRelocate={(id) => void relocateProject(id)} />
        <div className={"detail-pane" + (librarySurface === "project" && projectLoading && detail ? " is-switching" : "")}>
          <Suspense fallback={<div className="detail-pane-shell"><AppSkeleton /></div>}>
          {booting ? <div className="detail-pane-shell"><AppSkeleton /></div> : bootError ? <div className="detail-pane-shell"><main id="main-content" className="main-pane"><EmptyState title={t("startupFailed")} body={bootError} actions={<Button variant="primary" onClick={() => void bootstrapApp()}>{t("retry")}</Button>} /></main></div> : librarySurface === "dashboard" ? <div className="detail-pane-shell"><HomeDashboard t={t} refreshKey={dashboardRefreshKey} onOpenProject={(id) => selectProject(id)} onOpenCollection={(id) => void openCollectionFromDashboard(id)} onOpenRun={(runId) => void openTaskRun(runId).then(() => setActiveTasksOpen(true)).catch((error) => notify("error", t("tasksLoadFailed"), String(error)))} onOpenAttention={() => { setApprovalsOpen(true); void loadApprovals(); }} onOpenAttentionItem={openDashboardAttentionItem} /></div> : detail ? <div className="detail-pane-shell"><Dashboard detail={detail} t={t} notify={notify} onFavorite={async () => { await api.updateProject(detail.project.id, { favorite: !detail.project.favorite }); await reload(detail.project.id); }} onArchive={async () => { await api.updateProject(detail.project.id, { archived: !detail.project.archived }); notify("success", detail.project.archived ? t("projectRestored") : t("projectArchived")); await reload(); }} onRefresh={async () => { await api.refreshProject(detail.project.id); await reload(detail.project.id); }} onRemove={async () => { try { await removeProjectRecord(detail.project.id); } catch (error) { notify("error", t("removeRecordFailed"), String(error)); } }} onOpenExplorer={() => void api.openInExplorer(detail.project.canonicalPath).catch((error) => notify("error", t("openFailed"), String(error)))} onOpenTerminal={(terminal) => void api.openInTerminal(detail.project.canonicalPath, terminal).catch((error) => notify("error", t("openFailed"), String(error)))} onOpenIde={(ide) => void api.openInIde(detail.project.canonicalPath, ide).then(() => reload(detail.project.id)).catch((error) => notify("error", t("openFailed"), String(error)))} onOpenAgent={(agent) => void api.openInAgent(detail.project.canonicalPath, agent).then(() => reload(detail.project.id)).catch((error) => notify("error", t("openFailed"), String(error)))} onOpenProject={(id) => void loadProjectDetail(id)} onRelocate={() => void relocateProject(detail.project.id)} onDescription={async (description) => { try { await api.updateProject(detail.project.id, { description }); await reload(detail.project.id); } catch (error) { notify("error", t("saveFailed"), String(error)); throw error; } }} onNotes={(notes) => void api.updateProject(detail.project.id, { notes }).then(() => { notify("success", t("settingsSaved")); return reload(detail.project.id); }).catch((error) => notify("error", t("saveFailed"), String(error)))} onTags={(tags) => void api.updateProject(detail.project.id, { tags }).then(() => { notify("success", t("settingsSaved")); return reload(detail.project.id); }).catch((error) => notify("error", t("saveFailed"), String(error)))} /></div> : projectLoading ? <div className="detail-pane-shell"><AppSkeleton /></div> : <div className="detail-pane-shell"><main id="main-content" className="main-pane"><EmptyState title={t("noSelection")} body={t("selectProjectHint")} actions={<Button variant="primary" onClick={() => void chooseDirectory(false)}>{t("addRoot")}</Button>} /></main></div>}
          </Suspense>
        </div>
      </>
    </div>
    <Dialog.Root open={view === "help"} onOpenChange={(open) => { if (!open) closeHelp(); }}>
      <Dialog.Portal>
        <Dialog.Backdrop className="settings-page-backdrop" />
        <Dialog.Popup className="settings-page-dialog" aria-labelledby="help-page-title">
          <Suspense fallback={<AppSkeleton />}>
            <LazyHelpPage locale={locale} t={t} notify={notify} scanning={scanning} onReplayOnboarding={replayOnboarding} onBack={closeHelp} />
          </Suspense>
        </Dialog.Popup>
      </Dialog.Portal>
    </Dialog.Root>
    <Dialog.Root open={settingsOpen} onOpenChange={(open) => { if (!open) closeSettings(); }}>
      <Dialog.Portal>
        <Dialog.Backdrop className="settings-page-backdrop" />
        <Dialog.Popup className="settings-page-dialog" aria-labelledby="settings-page-title">
          <Suspense fallback={<AppSkeleton />}>
            <SettingsPane settings={settings} scanRoots={scanRoots} scanning={scanning} t={t} notify={notify} onSettings={updateSettings} onAddRoot={() => chooseDirectory(false)} onRemoveRoot={async (id) => { await api.removeScanRoot(id, false); notify("success", t("rootRemoved")); await reload(); }} onScanRoot={(id) => startScan(id)} onReload={() => reload()} updateState={updateState} onCheckForUpdates={() => updaterService.checkForUpdates()} onDownloadUpdate={() => updaterService.downloadUpdate()} onInstallUpdate={() => updaterService.installUpdate()} onRestartApp={() => updaterService.restartApp()} onDeferUpdate={() => updaterService.deferUpdate()} onBack={closeSettings} />
          </Suspense>
        </Dialog.Popup>
      </Dialog.Portal>
    </Dialog.Root>
    <CommandPalette open={paletteOpen} onClose={() => setPaletteOpen(false)} t={t} projects={visibleProjects} actions={paletteActions} onProject={(id) => void openProjectFromPalette(id)} />
    <Dialog.Root open={approvalsOpen} onOpenChange={setApprovalsOpen}>
      <Dialog.Portal>
        <Dialog.Backdrop className="dialog-backdrop" />
        <Dialog.Popup className="dialog-popup approval-dialog">
          <Dialog.Title className="dialog-title">{t("attentionCenter")}</Dialog.Title>
          <Dialog.Description className="dialog-description">{t("attentionCenterHint")}</Dialog.Description>
          {attentionError && <p className="error-copy" role="alert">{t("attentionLoadFailed")} · {attentionError}</p>}
          {visibleAttentionItems.length === 0 ? <p className="muted-copy">{t("noAttentionItems")}</p> : <div className="approval-list attention-list">{visibleAttentionItems.map((item) => {
            const approval = item.approvalId ? approvals.find((candidate) => candidate.id === item.approvalId) : undefined;
            return <article key={item.id} className={`approval-item attention-item severity-${item.severity}`}><div><strong>{item.title}</strong><span>{item.detail}</span><time>{new Date(item.occurredAt).toLocaleString()}</time></div><div>{approval ? <><Button variant="quiet" disabled={Boolean(resolvingApproval)} onClick={() => void resolveApproval(approval, false)}>{t("denyTask")}</Button><Button variant="primary" loading={resolvingApproval === approval.id} disabled={Boolean(resolvingApproval)} onClick={() => void resolveApproval(approval, true)}>{t("approveTask")}</Button></> : <>{item.runId && <Button variant="quiet" onClick={() => { setApprovalsOpen(false); void openTaskRun(item.runId!).then(() => setActiveTasksOpen(true)).catch((error) => notify("error", t("tasksLoadFailed"), String(error))); }}>{t("viewOutput")}</Button>}{item.projectId && <Button variant="quiet" onClick={() => { setApprovalsOpen(false); void jumpToProject(item.projectId!); }}>{t("jumpToProject")}</Button>}{(item.acknowledgeable || item.id.startsWith("persistence:")) && <Button variant="quiet" onClick={() => void acknowledgeAttention(item)}>{t("deleteMessage")}</Button>}</>}</div></article>;
          })}</div>}
          <div className="dialog-actions"><Dialog.Close render={<Button>{t("close")}</Button>} /></div>
        </Dialog.Popup>
      </Dialog.Portal>
    </Dialog.Root>
    {activeTasksOpen && <TaskWorkbench
      open={activeTasksOpen}
      t={t}
      theme={theme}
      nowTick={nowTick}
      stoppingRunId={stoppingRunId}
      stoppingAll={stoppingAll}
      onClose={() => setActiveTasksOpen(false)}
      onStop={(runId) => void stopRun(runId)}
      onStopAll={() => void stopAllRuns()}
      onJump={(projectId) => { setActiveTasksOpen(false); void jumpToProject(projectId); }}
      onPreviewError={(error) => notify("error", t("openPreviewFailed"), String(error))}
    />}
    <OnboardingDialog
      open={onboardingOpen}
      step={onboardingStep}
      t={t}
      locale={locale}
      mcpInfo={mcpInfo}
      mcpError={mcpError}
      scanRoots={scanRoots}
      selectedRootId={onboardingRootId}
      scanning={scanning}
      progress={progress}
      scanOutcome={onboardingScanOutcome}
      scanError={onboardingScanError}
      waiting={onboardingOpen && onboardingStep === "waiting" && !foundOnboardingProject}
      checkFailed={onboardingCheckFailed}
      foundProject={foundOnboardingProject}
      notify={notify}
      onStep={setOnboardingStep}
      onCopyInstruction={copyOnboardingInstruction}
      onChooseRoot={() => void chooseOnboardingRoot()}
      onSelectRoot={setOnboardingRootId}
      onStartScan={() => void startScan(onboardingRootId)}
      onCancelScan={() => void cancelScan()}
      onCheckNow={() => void checkNow()}
      onFinish={completeOnboarding}
      onSkip={skipOnboarding}
      onOpenChange={(next) => { if (!next) skipOnboarding(); }}
    />
    <ToastViewport toasts={toasts} onDismiss={(id) => setToasts((current) => current.filter((toast) => toast.id !== id))} />
  </div></Tooltip.Provider></MotionConfig>;
}

