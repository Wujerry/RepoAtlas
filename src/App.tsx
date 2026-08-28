import { Tooltip } from "@base-ui/react/tooltip";
import { Dialog } from "@base-ui/react/dialog";
import { open } from "@tauri-apps/plugin-dialog";
import { AnimatePresence, motion, MotionConfig } from "framer-motion";
import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { CommandPalette } from "./components/CommandPalette";
import { ProjectList, type ProjectFilters } from "./components/ProjectList";
import { TitleBar } from "./components/TitleBar";
import { Button } from "./components/ui/button";
import { AppSkeleton, EmptyState, ToastViewport } from "./components/ui/feedback";
import { dictionaries, resolveLocale, type Locale, type MessageKey } from "./i18n";
import { api, onScanCompleted, onScanFailed, onScanProgress } from "./lib/api";
import { isSameOrAncestorPath, scanRootsUnderPath } from "./lib/project-tree";
import type { AppSettings, AppView, PendingApproval, ProjectDetail, ProjectQuery, ProjectScope, ProjectSummary, ScanProgress, ScanRoot, ToastMessage, ToastTone } from "./types";

const Dashboard = lazy(() => import("./components/Dashboard").then(({ Dashboard }) => ({ default: Dashboard })));
const LazyHelpPage = lazy(() => import("./components/HelpPage").then(({ HelpPage }) => ({ default: HelpPage })));
const SettingsPane = lazy(() => import("./components/SettingsPane").then(({ SettingsPane }) => ({ default: SettingsPane })));

function DeferredPageFallback() {
  return <div className="detail-pane-shell"><AppSkeleton /></div>;
}

const systemLocale: Locale = navigator.language.toLowerCase().startsWith("zh") ? "zh" : "en";
const emptyFilters: ProjectFilters = { vcs: "", language: "", tag: "" };

function queryForScope(scope: ProjectScope, search = ""): ProjectQuery {
  return { section: scope === "projects" ? undefined : scope, search: search || undefined, includeArchived: scope === "archived" };
}

export default function App() {
  const media = useMemo(() => window.matchMedia("(prefers-color-scheme: light)"), []);
  const [systemTheme, setSystemTheme] = useState(media.matches ? "light" : "dark");
  const [settings, setSettings] = useState<AppSettings>({ theme: "system", locale: "system", uiFont: "", consoleFont: "" });
  const [view, setView] = useState<AppView>("library");
  const [scope, setScope] = useState<ProjectScope>("projects");
  const [scanRoots, setScanRoots] = useState<ScanRoot[]>([]);
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [scopeProjects, setScopeProjects] = useState<ProjectSummary[]>([]);
  const [selectedId, setSelectedId] = useState<string>();
  const [detail, setDetail] = useState<ProjectDetail | null>(null);
  const [query, setQuery] = useState("");
  const [filters, setFilters] = useState<ProjectFilters>(emptyFilters);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [approvalsOpen, setApprovalsOpen] = useState(false);
  const [approvals, setApprovals] = useState<PendingApproval[]>([]);
  const [resolvingApproval, setResolvingApproval] = useState<string>();
  const [scanning, setScanning] = useState(false);
  const [progress, setProgress] = useState<ScanProgress | null>(null);
  const [booting, setBooting] = useState(true);
  const [bootError, setBootError] = useState<string>();
  const [projectLoading, setProjectLoading] = useState(false);
  const [toasts, setToasts] = useState<ToastMessage[]>([]);
  const booted = useRef(false);
  const querySequence = useRef(0);
  const detailSequence = useRef(0);
  const scanningRef = useRef(false);
  const scopeCache = useRef<{ scope: ProjectScope; projects: ProjectSummary[] } | undefined>(undefined);
  const locale = resolveLocale(settings.locale, systemLocale);
  const t = useCallback((key: MessageKey) => dictionaries[locale][key], [locale]);
  const theme = settings.theme === "system" ? systemTheme : settings.theme;

  const notify = useCallback((tone: ToastTone, title: string, detail?: string) => {
    const id = globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random()}`;
    setToasts((current) => [...current.slice(-3), { id, tone, title, detail }]);
    window.setTimeout(() => setToasts((current) => current.filter((item) => item.id !== id)), tone === "error" ? 8000 : 4800);
  }, []);

  const loadProjectDetail = useCallback(async (id?: string) => {
    const sequence = ++detailSequence.current;
    if (!id) { setSelectedId(undefined); setDetail(null); setProjectLoading(false); return; }
    setProjectLoading(true); setSelectedId(id);
    try { const next = await api.getProject(id); if (sequence === detailSequence.current) setDetail(next); }
    catch (error) { if (sequence === detailSequence.current) { setDetail(null); notify("error", t("projectLoadFailed"), String(error)); } }
    finally { if (sequence === detailSequence.current) setProjectLoading(false); }
  }, [notify, t]);

  const reload = useCallback(async (nextSelected?: string) => {
    const sequence = ++querySequence.current;
    try {
      const baseQuery = queryForScope(scope);
      const [nextRoots, nextScopeProjects] = await Promise.all([
        api.listScanRoots(), api.listProjects(baseQuery),
      ]);
      const nextProjects = query.trim()
        ? await api.listProjects(queryForScope(scope, query))
        : nextScopeProjects;
      if (sequence !== querySequence.current) return;
      scopeCache.current = { scope, projects: nextScopeProjects };
      setScanRoots(nextRoots); setScopeProjects(nextScopeProjects); setProjects(nextProjects);
      const id = nextSelected ?? (nextProjects.some((project) => project.id === selectedId) ? selectedId : nextProjects[0]?.id);
      await loadProjectDetail(id);
    } catch (error) { notify("error", t("reloadFailed"), String(error)); }
  }, [loadProjectDetail, notify, query, scope, selectedId, t]);

  useEffect(() => {
    const onTheme = (event: MediaQueryListEvent) => setSystemTheme(event.matches ? "light" : "dark");
    media.addEventListener("change", onTheme); return () => media.removeEventListener("change", onTheme);
  }, [media]);

  useEffect(() => {
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
      setSettings(boot.settings); setScanRoots(boot.scanRoots); setProjects(boot.projects); setScopeProjects(boot.projects);
      scopeCache.current = { scope: "projects", projects: boot.projects };
      if (boot.projects[0]) await loadProjectDetail(boot.projects[0].id);
    } catch (error) {
      const detail = String(error); setBootError(detail); notify("error", t("startupFailed"), detail);
    } finally { setBooting(false); }
  }, [loadProjectDetail, notify, t]);

  useEffect(() => {
    if (booted.current) return;
    booted.current = true;
    void bootstrapApp();
  }, [bootstrapApp]);

  const loadApprovals = useCallback(async () => {
    try { setApprovals(await api.listPendingApprovals()); }
    catch { /* MCP approval center is non-blocking during startup. */ }
  }, []);

  useEffect(() => {
    if (booting) return;
    void loadApprovals();
    const timer = window.setInterval(() => void loadApprovals(), 5000);
    const onFocus = () => void loadApprovals();
    window.addEventListener("focus", onFocus);
    return () => { window.clearInterval(timer); window.removeEventListener("focus", onFocus); };
  }, [booting, loadApprovals]);

  async function resolveApproval(approval: PendingApproval, approved: boolean) {
    if (resolvingApproval) return;
    setResolvingApproval(approval.id);
    try {
      await api.resolvePendingApproval(approval.id, approved);
      notify(approved ? "success" : "info", approved ? t("approvalStarted") : t("approvalDenied"), approval.title);
      await loadApprovals();
      if (approvals.length <= 1) setApprovalsOpen(false);
    } catch (error) {
      notify("error", t("approvalFailed"), String(error));
    } finally { setResolvingApproval(undefined); }
  }

  useEffect(() => {
    if (booting) return;
    const sequence = ++querySequence.current;
    const timer = window.setTimeout(() => {
      const baseQuery = queryForScope(scope);
      const cached = scopeCache.current;
      const basePromise = cached?.scope === scope
        ? Promise.resolve(cached.projects)
        : api.listProjects(baseQuery);
      const nextPromise = query.trim()
        ? api.listProjects(queryForScope(scope, query))
        : basePromise;
      Promise.all([basePromise, nextPromise]).then(async ([base, next]) => {
        if (sequence !== querySequence.current) return;
        scopeCache.current = { scope, projects: base };
        setScopeProjects(base); setProjects(next);
        const id = next.some((project) => project.id === selectedId) ? selectedId : next[0]?.id;
        await loadProjectDetail(id);
      }).catch((error) => notify("error", t("searchFailed"), String(error)));
    }, 180);
    return () => window.clearTimeout(timer);
  }, [booting, loadProjectDetail, notify, query, scope, t]);

  useEffect(() => {
    const unsubs = Promise.all([
      onScanProgress(setProgress),
      onScanCompleted(async (results) => {
        scanningRef.current = false; setScanning(false); setProgress(null);
        const summary = results.reduce((sum, result) => ({ visited: sum.visited + result.visited, discovered: sum.discovered + result.discovered, errors: sum.errors + result.errors.length, cancelled: sum.cancelled || result.cancelled }), { visited: 0, discovered: 0, errors: 0, cancelled: false });
        const errorDetails = results.flatMap((result) => result.errors).slice(0, 3);
        notify(summary.errors ? "warning" : "success", summary.cancelled ? t("scanCancelled") : t("scanComplete"), `${summary.discovered} ${t("found")} · ${summary.visited} ${t("visited")}${summary.errors ? ` · ${summary.errors} ${t("errors")}` : ""}${errorDetails.length ? `\n${errorDetails.join("\n")}` : ""}`);
        await reload();
      }),
      onScanFailed((message) => { scanningRef.current = false; setScanning(false); setProgress(null); notify("error", t("scanFailed"), message); }),
    ]);
    return () => { void unsubs.then((functions) => functions.forEach((unsubscribe) => unsubscribe())); };
  }, [notify, reload, t]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") { event.preventDefault(); setPaletteOpen(true); }
      if (event.key === "Escape") setPaletteOpen(false);
    };
    window.addEventListener("keydown", onKey); return () => window.removeEventListener("keydown", onKey);
  }, []);

  const visibleProjects = useMemo(() => projects.filter((project) => (!filters.vcs || project.vcsKind === filters.vcs) && (!filters.language || project.languages.includes(filters.language)) && (!filters.tag || project.tags.includes(filters.tag))), [filters, projects]);
  useEffect(() => {
    if (projectLoading) return;
    if (visibleProjects.length === 0) { if (selectedId) void loadProjectDetail(); return; }
    if (!visibleProjects.some((project) => project.id === selectedId)) void loadProjectDetail(visibleProjects[0]?.id);
  }, [loadProjectDetail, projectLoading, selectedId, visibleProjects]);

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
      else { await api.addScanRoot(selected); notify("success", t("rootAdded"), selected); await reload(); }
    } catch (error) { notify("error", register ? t("registerFailed") : t("rootAddFailed"), String(error)); }
  }
  async function selectProject(id: string) { setView("library"); await loadProjectDetail(id); try { await api.markOpened(id); } catch { /* non-blocking */ } }
  async function openProjectFromPalette(id: string) {
    try {
      const next = await api.getProject(id);
      detailSequence.current += 1;
      setView("library"); setScope(next.project.archived ? "archived" : "projects"); setQuery(""); setFilters(emptyFilters);
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

  const paletteActions = [
    { id: "open-help", title: t("openHelp"), run: () => setView("help") }, { id: "open-settings", title: t("openSettings"), run: () => setView("settings") },
    { id: "scan-all", title: t("scanAll"), run: () => void startScan() }, { id: "add-root", title: t("addRoot"), run: () => void chooseDirectory(false) }, { id: "register", title: t("register"), run: () => void chooseDirectory(true) },
    ...visibleProjects.map((project) => ({ id: `jump:${project.id}`, title: `${t("jump")} · ${project.displayName}`, run: () => void selectProject(project.id) })),
  ];
  const emptyMessage = scope === "favorites" ? t("noFavorites") : scope === "recent" ? t("noRecent") : scope === "archived" ? t("noArchived") : t("emptyBody");

  return <MotionConfig reducedMotion="user" transition={{ duration: 0.18, ease: [0.2, 0.8, 0.2, 1] }}><Tooltip.Provider delay={300}><div className="app-shell">
    <TitleBar title={t("appName")} subtitle={t("appTag")} commandLabel={t("commandShort")} localLabel={t("localStatus")} scanningLabel={t("scanningStatus")} minimizeLabel={t("minimizeWindow")} maximizeLabel={t("maximizeWindow")} closeLabel={t("close")} helpLabel={t("help")} settingsLabel={t("settings")} approvalsLabel={t("pendingApprovals")} approvalCount={approvals.length} activeView={view} scanning={scanning} onCommand={() => setPaletteOpen(true)} onLibrary={() => setView("library")} onHelp={() => setView("help")} onSettings={() => setView("settings")} onApprovals={() => { setApprovalsOpen(true); void loadApprovals(); }} />
    <div className={`workspace workspace-${view}`}>
      <Suspense fallback={<DeferredPageFallback />}>
      {view === "settings" ? <Suspense fallback={<DeferredPageFallback />}><SettingsPane settings={settings} scanRoots={scanRoots} scanning={scanning} t={t} notify={notify} onSettings={updateSettings} onAddRoot={() => chooseDirectory(false)} onRemoveRoot={async (id) => { await api.removeScanRoot(id, false); notify("success", t("rootRemoved")); await reload(); }} onScanRoot={(id) => startScan(id)} onReload={() => reload()} /></Suspense> : view === "help" ? <Suspense fallback={<DeferredPageFallback />}><LazyHelpPage locale={locale} t={t} notify={notify} /></Suspense> : <>
        <ProjectList projects={visibleProjects} allProjects={scopeProjects} selectedId={selectedId} onSelect={(id) => void selectProject(id)} query={query} onQuery={setQuery} filters={filters} onFilters={setFilters} scope={scope} onScope={setScope} scanning={scanning} progress={progress} t={t} empty={emptyMessage} onAddRoot={() => void chooseDirectory(false)} onRegister={() => void chooseDirectory(true)} onScan={() => void startScan()} onCancelScan={() => void cancelScan()} onRename={async (id, displayName) => { try { await api.updateProject(id, { displayName }); await reload(id); } catch (error) { notify("error", t("saveFailed"), String(error)); throw error; } }} onDescription={async (id, description) => { try { await api.updateProject(id, { description }); await reload(id); } catch (error) { notify("error", t("saveFailed"), String(error)); throw error; } }} scanRoots={scanRoots} onRemoveProject={async (id) => { try { await removeProjectRecord(id); } catch (error) { notify("error", t("removeRecordFailed"), String(error)); } }} onRemoveFolder={async (path, projectIds) => { try { await removeFolderRecords(path, projectIds); } catch (error) { notify("error", t("removeFolderFailed"), String(error)); } }} onIconError={(error) => notify("error", t("projectIconFailed"), String(error))} onReveal={(path) => void api.openInExplorer(path).catch((error) => notify("error", t("openFailed"), String(error)))} onRelocate={(id) => void relocateProject(id)} />
        <div className={"detail-pane" + (projectLoading && detail ? " is-switching" : "")}>
          <AnimatePresence mode="wait" initial={false}>
            {booting && !detail ? <motion.div key="boot" className="detail-pane-shell" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: 0.16 }}><AppSkeleton /></motion.div> : bootError && !detail ? <motion.div key="boot-error" className="detail-pane-shell" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}><main id="main-content" className="main-pane"><EmptyState title={t("startupFailed")} body={bootError} actions={<Button variant="primary" onClick={() => void bootstrapApp()}>{t("retry")}</Button>} /></main></motion.div> : detail ? <motion.div key={detail.project.id} className="detail-pane-shell" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: 0.18 }}><Dashboard detail={detail} t={t} notify={notify} onFavorite={async () => { await api.updateProject(detail.project.id, { favorite: !detail.project.favorite }); await reload(detail.project.id); }} onArchive={async () => { await api.updateProject(detail.project.id, { archived: !detail.project.archived }); notify("success", detail.project.archived ? t("projectRestored") : t("projectArchived")); await reload(); }} onRefresh={async () => { await api.refreshProject(detail.project.id); await reload(detail.project.id); }} onRemove={async () => { try { await removeProjectRecord(detail.project.id); } catch (error) { notify("error", t("removeRecordFailed"), String(error)); } }} onOpenExplorer={() => void api.openInExplorer(detail.project.canonicalPath).catch((error) => notify("error", t("openFailed"), String(error)))} onOpenTerminal={() => void api.openInTerminal(detail.project.canonicalPath).catch((error) => notify("error", t("openFailed"), String(error)))} onOpenIde={(ide) => void api.openInIde(detail.project.canonicalPath, ide).then(() => reload(detail.project.id)).catch((error) => notify("error", t("openFailed"), String(error)))} onOpenSettings={() => setView("settings")} onOpenProject={(id) => void loadProjectDetail(id)} onRelocate={() => void relocateProject(detail.project.id)} onDescription={async (description) => { try { await api.updateProject(detail.project.id, { description }); await reload(detail.project.id); } catch (error) { notify("error", t("saveFailed"), String(error)); throw error; } }} onNotes={(notes) => void api.updateProject(detail.project.id, { notes }).then(() => { notify("success", t("settingsSaved")); return reload(detail.project.id); }).catch((error) => notify("error", t("saveFailed"), String(error)))} onTags={(tags) => void api.updateProject(detail.project.id, { tags }).then(() => { notify("success", t("settingsSaved")); return reload(detail.project.id); }).catch((error) => notify("error", t("saveFailed"), String(error)))} /></motion.div> : <motion.div key="empty" className="detail-pane-shell" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: 0.16 }}><main id="main-content" className="main-pane"><EmptyState title={t("noSelection")} body={t("selectProjectHint")} actions={<Button variant="primary" onClick={() => void chooseDirectory(false)}>{t("addRoot")}</Button>} /></main></motion.div>}
          </AnimatePresence>
          
        </div>
      </>}
      </Suspense>
    </div>
    <CommandPalette open={paletteOpen} onClose={() => setPaletteOpen(false)} t={t} projects={visibleProjects} actions={paletteActions} onProject={(id) => void openProjectFromPalette(id)} />
    <Dialog.Root open={approvalsOpen} onOpenChange={setApprovalsOpen}>
      <Dialog.Portal>
        <Dialog.Backdrop className="dialog-backdrop" />
        <Dialog.Popup className="dialog-popup approval-dialog">
          <Dialog.Title className="dialog-title">{t("pendingApprovals")}</Dialog.Title>
          <Dialog.Description className="dialog-description">{t("pendingApprovalsHint")}</Dialog.Description>
          {approvals.length === 0 ? <p className="muted-copy">{t("noPendingApprovals")}</p> : <div className="approval-list">{approvals.map((approval) => <article key={approval.id} className="approval-item"><div><strong>{approval.title}</strong><span>{approval.detail}</span><code>{[approval.executable, ...approval.argv].filter(Boolean).join(" ")}</code><code>{approval.cwd}</code></div><div><Button variant="quiet" disabled={Boolean(resolvingApproval)} onClick={() => void resolveApproval(approval, false)}>{t("denyTask")}</Button><Button variant="primary" loading={resolvingApproval === approval.id} disabled={Boolean(resolvingApproval)} onClick={() => void resolveApproval(approval, true)}>{t("approveTask")}</Button></div></article>)}</div>}
          <div className="dialog-actions"><Dialog.Close render={<Button>{t("close")}</Button>} /></div>
        </Dialog.Popup>
      </Dialog.Portal>
    </Dialog.Root>
    <ToastViewport toasts={toasts} onDismiss={(id) => setToasts((current) => current.filter((toast) => toast.id !== id))} />
  </div></Tooltip.Provider></MotionConfig>;
}

