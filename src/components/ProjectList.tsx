import { useVirtualizer } from "@tanstack/react-virtual";
import { open } from "@tauri-apps/plugin-dialog";
import { Archive, ArrowCounterClockwise, ArrowsInSimple, CaretDoubleDown, CaretDoubleUp, CaretRight, Check, ClockCounterClockwise, Copy, Crosshair, FolderPlus, FolderSimple, Image, ListBullets, MagnifyingGlass, MapPin, NotePencil, PencilSimple, Plus, PlusCircle, SortAscending, SquaresFour, Star, Trash, TreeStructure, XCircle } from "@phosphor-icons/react";
import { useEffect, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import type { MessageKey } from "../i18n";
import { api } from "../lib/api";
import { formatTime, stackOf } from "../lib/format";
import { LanguageGlyph, languageFallback } from "../lib/project-identity";
import {
  ancestorGroupIds,
  buildProjectTree,
  collectGroupIds,
  flattenProjectTree,
  groupLabel,
  isSameOrAncestorPath,
  sortProjects,
  scanRootsUnderPath,
  type ProjectSort,
  type ProjectTreeRow,
} from "../lib/project-tree";
import type { ProjectCollection, ProjectScope, ProjectSummary, ScanProgress, ScanRoot } from "../types";
import { Button } from "./ui/button";
import { ConfirmDialog } from "./ui/confirm";
import { EmptyState } from "./ui/feedback";
import { ActionMenu, ItemContextMenu } from "./ui/menu";

export interface ProjectFilters {
  language: string;
  tag: string;
}

const scopeKeys: Record<ProjectScope, MessageKey> = {
  projects: "projects",
  favorites: "favorites",
  recent: "recent",
  archived: "archived",
};

const scopeOptions: ProjectScope[] = ["projects", "favorites", "recent", "archived"];
const scopeIcons = {
  projects: FolderSimple,
  favorites: Star,
  recent: ClockCounterClockwise,
  archived: Archive,
} as const;

const sortOptions: ProjectSort[] = ["default", "recent-opened", "recent-updated", "recent-committed", "name", "path"];

const sortLabels: Record<ProjectSort, MessageKey> = {
  default: "sortDefault",
  "recent-opened": "sortRecentOpened",
  "recent-updated": "sortRecentUpdated",
  "recent-committed": "sortRecentCommitted",
  name: "sortName",
  path: "sortPath",
};

interface ProjectListProps {
  projects: ProjectSummary[];
  allProjects: ProjectSummary[];
  selectedId?: string;
  onSelect: (id: string) => void;
  query: string;
  onQuery: (value: string) => void;
  filters: ProjectFilters;
  onFilters: (filters: ProjectFilters) => void;
  sort?: ProjectSort;
  onSort?: (sort: ProjectSort) => void;
  scope?: ProjectScope;
  onScope?: (scope: ProjectScope) => void;
  scanning: boolean;
  progress: ScanProgress | null;
  t: (key: MessageKey) => string;
  empty: string;
  onAddRoot: () => void;
  onRegister: () => void;
  onScan: () => void;
  onCancelScan: () => void;
  onRename?: (id: string, displayName: string) => void | Promise<void>;
  onDescription?: (id: string, description: string | null) => void | Promise<void>;
  scanRoots?: ScanRoot[];
  onRemoveProject?: (id: string) => void | Promise<void>;
  onRemoveFolder?: (path: string, projectIds: string[]) => void | Promise<void>;
  onIconError?: (error: unknown) => void;
  onReveal?: (path: string) => void;
  onRelocate?: (id: string) => void | Promise<void>;
  collectionControls?: ReactNode;
  collections?: ProjectCollection[];
  selectedCollectionId?: string;
  onAddToCollection?: (projectId: string, collectionId: string) => void | Promise<void>;
  onRemoveFromCollection?: (projectId: string, collectionId: string) => void | Promise<void>;
}

export function ProjectList({
  projects,
  allProjects,
  selectedId,
  onSelect,
  query,
  onQuery,
  filters,
  onFilters,
  sort = "default",
  onSort,
  scope = "projects",
  onScope,
  scanning,
  progress,
  t,
  empty,
  onAddRoot,
  onRegister,
  onScan,
  onCancelScan,
  onRename,
  onDescription,
  scanRoots = [],
  onRemoveProject,
  onRemoveFolder,
  onIconError,
  onReveal,
  onRelocate,
  collectionControls,
  collections = [],
  selectedCollectionId,
  onAddToCollection,
  onRemoveFromCollection,
}: ProjectListProps) {
  const iconCache = useRef(new Map<string, { revision: string; icon: { kind: string; source: string | null; dataUrl: string | null } }>());
  const parentRef = useRef<HTMLDivElement>(null);
  const revealBaselineRef = useRef(false);
  const lastSelectedIdRef = useRef<string | undefined>(undefined);
  const pendingRevealRef = useRef(false);
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(() => new Set());
  const [activeRowId, setActiveRowId] = useState<string>();
  const initializedExpansion = useRef(false);
  const expansionBeforeSearch = useRef<Set<string> | null>(null);
  const searchWasActive = useRef(false);
  const previousScope = useRef(scope);
  const [layout, setLayout] = useState<"tree" | "flat">("tree");
  const [editing, setEditing] = useState<{ id: string; field: "name" | "description" } | null>(null);
  const [draft, setDraft] = useState("");
  const [pendingProject, setPendingProject] = useState<ProjectSummary>();
  const [pendingFolder, setPendingFolder] = useState<{ path: string; projectIds: string[]; rootCount: number }>();
  const [icons, setIcons] = useState<Record<string, { kind: string; source: string | null; dataUrl: string | null }>>({});
  const projectIconRevision = useMemo(
    () => projects.map((project) => `${project.id}:${project.updatedAt}`).join("|"),
    [projects],
  );
  const roots = useMemo(() => buildProjectTree(projects, sort), [projects, sort]);
  const groupIds = useMemo(() => collectGroupIds(roots), [roots]);
  const rows = useMemo(() => flattenProjectTree(roots, expandedGroups), [expandedGroups, roots]);
  const selectedInList = useMemo(() => projects.some((project) => project.id === selectedId), [projects, selectedId]);
  const sortedProjects = useMemo(() => sortProjects(projects, sort), [projects, sort]);
  const visibleRows = useMemo<ProjectTreeRow[]>(() => layout === "tree" ? rows : sortedProjects.map((project, index) => ({
    kind: "project",
    id: `project:${project.id}`,
    project,
    depth: 1,
    parentId: "",
    siblingIndex: index + 1,
    siblingCount: projects.length,
  })), [layout, sortedProjects, rows]);
  const facets = useMemo(() => ({
    languages: [...new Set(allProjects.flatMap((project) => project.languages))].sort(),
    tags: [...new Set(allProjects.flatMap((project) => project.tags))].sort(),
  }), [allProjects]);
  const hasFacetFilters = Boolean(filters.language || filters.tag);
  const hasTextFilter = Boolean(query.trim() || hasFacetFilters);

  useEffect(() => {
    setExpandedGroups((current) => {
      const valid = new Set(groupIds);
      if (!initializedExpansion.current) {
        if (groupIds.length === 0) return current;
        initializedExpansion.current = true;
        previousScope.current = scope;
        searchWasActive.current = Boolean(query.trim());
        return valid;
      }
      if (previousScope.current !== scope) {
        previousScope.current = scope;
        expansionBeforeSearch.current = null;
        searchWasActive.current = Boolean(query.trim());
        return valid;
      }
      const searching = Boolean(query.trim());
      if (searching && !searchWasActive.current) expansionBeforeSearch.current = new Set(current);
      const source = !searching && searchWasActive.current && expansionBeforeSearch.current
        ? expansionBeforeSearch.current
        : current;
      const next = new Set([...source].filter((id) => valid.has(id)));
      if (searching) groupIds.forEach((id) => next.add(id));
      if (selectedId) ancestorGroupIds(roots, selectedId).forEach((id) => next.add(id));
      if (!searching && searchWasActive.current) expansionBeforeSearch.current = null;
      searchWasActive.current = searching;
      return next;
    });
  }, [groupIds, query, roots, scope, selectedId]);

  useEffect(() => {
    const preferred = selectedId ? `project:${selectedId}` : undefined;
    setActiveRowId((current) => visibleRows.some((row) => row.id === current) ? current : preferred && visibleRows.some((row) => row.id === preferred) ? preferred : visibleRows[0]?.id);
  }, [selectedId, visibleRows]);

  const virtualizer = useVirtualizer({
    count: visibleRows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: (index) => {
      const row = visibleRows[index];
      if (!row || row.kind === "group") return 32;
      return row.project.description ? 108 : 84;
    },
    measureElement: (element) => element.getBoundingClientRect().height,
    initialRect: { width: 360, height: 640 },
    overscan: 12,
  });
  useEffect(() => {
    virtualizer.measure();
  }, [editing, layout, visibleRows, virtualizer]);
  const visibleProjectIds = virtualizer.getVirtualItems()
    .map((item) => visibleRows[item.index])
    .filter((row): row is Extract<ProjectTreeRow, { kind: "project" }> => row?.kind === "project")
    .map((row) => row.project.id);
  useEffect(() => {
    setIcons(Object.fromEntries(projects.flatMap((project) => {
      const cached = iconCache.current.get(project.id);
      return cached?.revision === project.updatedAt ? [[project.id, cached.icon]] : [];
    })));
  }, [projectIconRevision, projects]);

  useEffect(() => {
    const missing = visibleProjectIds.filter((id) => icons[id] === undefined);
    if (missing.length === 0) return;
    let cancelled = false;
    void api.readProjectIcons(missing).then((items) => {
      if (cancelled) return;
      setIcons((current) => {
        const next = { ...current };
        for (const item of items) {
          const icon = { kind: item.kind, source: item.source, dataUrl: item.dataUrl };
          next[item.projectId] = icon;
          const revision = projects.find((project) => project.id === item.projectId)?.updatedAt;
          if (revision) iconCache.current.set(item.projectId, { revision, icon });
        }
        for (const id of missing) {
          if (!next[id]) next[id] = { kind: "language", source: null, dataUrl: null };
        }
        return next;
      });
    }).catch(() => {
      if (cancelled) return;
      setIcons((current) => {
        const next = { ...current };
        for (const id of missing) next[id] = { kind: "language", source: null, dataUrl: null };
        return next;
      });
    });
    return () => { cancelled = true; };
  }, [icons, projectIconRevision, visibleProjectIds.join("|")]);
  const activeIndex = visibleRows.findIndex((row) => row.id === activeRowId);

  async function chooseProjectIcon(projectId: string) {
    try {
      const path = await open({
        multiple: false,
        filters: [{ name: t("projectIcon"), extensions: ["png", "jpg", "jpeg", "webp", "ico"] }],
      });
      if (typeof path !== "string") return;
      const icon = await api.setProjectIcon(projectId, path);
      setIcons((current) => ({ ...current, [projectId]: icon }));
    } catch (error) {
      onIconError?.(error);
    }
  }

  async function clearProjectIcon(projectId: string) {
    try {
      const icon = await api.clearProjectIcon(projectId);
      setIcons((current) => ({ ...current, [projectId]: icon }));
    } catch (error) {
      onIconError?.(error);
    }
  }

  useEffect(() => {
    if (activeIndex >= 0) virtualizer.scrollToIndex(activeIndex, { align: "auto" });
  }, [activeIndex, virtualizer]);

  // A fresh selection from outside the list (command palette, dashboard, scope
  // auto-pick) must reveal its row: expand ancestors, move the active row, and
  // scroll the virtual list once the row exists. Repeated renders keep waiting
  // until ancestor expansion makes the row visible.
  useEffect(() => {
    if (!revealBaselineRef.current) {
      revealBaselineRef.current = true;
      lastSelectedIdRef.current = selectedId;
      return;
    }
    const selectionChanged = selectedId !== lastSelectedIdRef.current;
    lastSelectedIdRef.current = selectedId;
    if (selectionChanged) pendingRevealRef.current = Boolean(selectedId);
    if (!pendingRevealRef.current || !selectedId) return;
    if (!projects.some((project) => project.id === selectedId)) {
      pendingRevealRef.current = false;
      return;
    }
    const index = visibleRows.findIndex((row) => row.id === `project:${selectedId}`);
    if (index < 0) return;
    pendingRevealRef.current = false;
    setActiveRowId(`project:${selectedId}`);
    virtualizer.scrollToIndex(index, { align: "auto" });
  }, [projects, selectedId, visibleRows, virtualizer]);

  function toggleGroup(id: string) {
    setExpandedGroups((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  }

  function expandAllGroups() {
    setExpandedGroups(new Set(groupIds));
  }

  function collapseAllGroups() {
    setExpandedGroups(new Set());
  }

  function revealSelectedProject() {
    if (!selectedId || !selectedInList) return;
    setExpandedGroups((current) => {
      const next = new Set(current);
      ancestorGroupIds(roots, selectedId).forEach((id) => next.add(id));
      return next;
    });
    pendingRevealRef.current = true;
  }

  function collapseToSelected() {
    if (!selectedId || !selectedInList) return;
    setExpandedGroups(new Set(ancestorGroupIds(roots, selectedId)));
    pendingRevealRef.current = true;
  }

  function moveActive(nextIndex: number) {
    const row = visibleRows[nextIndex];
    if (!row) return;
    setActiveRowId(row.id);
  }

  function focusTree() {
    parentRef.current?.focus({ preventScroll: true });
  }

  function parentRowIndex(row: ProjectTreeRow): number {
    if (!row.parentId) return -1;
    return visibleRows.findIndex((candidate) => candidate.id === `group:${row.parentId}`);
  }

  function onTreeKeyDown(event: React.KeyboardEvent<HTMLDivElement>) {
    if (visibleRows.length === 0) return;
    const currentIndex = activeIndex >= 0 ? activeIndex : 0;
    const current = visibleRows[currentIndex];
    if (!current) return;

    if (event.key === "ArrowDown") {
      event.preventDefault();
      moveActive(Math.min(visibleRows.length - 1, currentIndex + 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      moveActive(Math.max(0, currentIndex - 1));
    } else if (event.key === "Home") {
      event.preventDefault();
      moveActive(0);
    } else if (event.key === "End") {
      event.preventDefault();
      moveActive(visibleRows.length - 1);
    } else if (event.key === "ArrowRight" && current.kind === "group") {
      event.preventDefault();
      if (!current.expanded) toggleGroup(current.id.replace(/^group:/, ""));
      else moveActive(Math.min(visibleRows.length - 1, currentIndex + 1));
    } else if (event.key === "ArrowLeft") {
      event.preventDefault();
      if (current.kind === "group" && current.expanded) toggleGroup(current.id.replace(/^group:/, ""));
      else {
        const parentIndex = parentRowIndex(current);
        if (parentIndex >= 0) moveActive(parentIndex);
      }
    } else if (event.key === "Enter") {
      event.preventDefault();
      if (current.kind === "group") toggleGroup(current.id.replace(/^group:/, ""));
      else onSelect(current.project.id);
    } else if (event.key === "Escape" && query) {
      event.preventDefault();
      onQuery("");
    } else if (event.key === "F2" && current.kind === "project" && onRename) {
      event.preventDefault();
      setEditing({ id: current.project.id, field: "name" });
      setDraft(current.project.displayName);
    } else if ((event.key === "ContextMenu" || (event.shiftKey && event.key === "F10"))) {
      event.preventDefault();
      const target = parentRef.current?.querySelector<HTMLElement>(`[data-context-row="${CSS.escape(current.id)}"]`);
      if (target) {
        const rect = target.getBoundingClientRect();
        target.dispatchEvent(new MouseEvent("contextmenu", {
          bubbles: true,
          cancelable: true,
          clientX: rect.left + Math.min(36, rect.width / 2),
          clientY: rect.top + Math.min(20, rect.height / 2),
        }));
      }
    }
  }

  function renderRow(row: ProjectTreeRow, virtualRow: ReturnType<typeof virtualizer.getVirtualItems>[number]) {
    const selected = row.kind === "project" && selectedId === row.project.id;
    const className = row.kind === "group" ? "path-tree-group-row" : `path-tree-project-row${selected ? " active" : ""}`;
    const isTree = layout === "tree";

    return <div
      className={`virtual-row ${row.kind === "group" ? "path-tree-virtual-group" : "path-tree-virtual-project"}`}
      key={virtualRow.key}
      ref={virtualizer.measureElement}
      data-index={virtualRow.index}
      style={{ transform: `translateY(${virtualRow.start}px)` }}
    >
      <div
        id={row.id}
        role={isTree ? "treeitem" : "option"}
        aria-level={isTree ? row.depth : undefined}
        aria-posinset={isTree ? row.siblingIndex : undefined}
        aria-setsize={isTree ? row.siblingCount : undefined}
        aria-selected={row.kind === "project" ? selected : undefined}
        aria-expanded={isTree && row.kind === "group" ? row.expanded : undefined}
        aria-label={row.kind === "project" ? row.project.displayName : row.label}
        className={className}
        data-tree-active={row.id === activeRowId || undefined}
        style={{ paddingLeft: `${Math.max(0, row.depth - 1) * 12 + 10}px` }}
      >
        {row.kind === "group" ? <ItemContextMenu
           trigger={<button
             className="path-tree-group-toggle"
             tabIndex={-1}
             data-context-row={row.id}
             onMouseDown={(event) => { if (event.button === 0) event.preventDefault(); focusTree(); }}
             onClick={() => { focusTree(); setActiveRowId(row.id); toggleGroup(row.id.replace(/^group:/, "")); }}
             onContextMenu={() => setActiveRowId(row.id)}
             aria-label={row.label}
             aria-expanded={row.expanded}
          >
            <CaretRight className={row.expanded ? "path-tree-caret expanded" : "path-tree-caret"} aria-hidden="true" />
            <FolderSimple className="path-tree-folder" weight="fill" aria-hidden="true" />
            <span className="path-tree-group-label">{groupLabel(row.path, row.depth)}</span>
            <span className="path-tree-group-count">{row.projectCount}</span>
          </button>}
          items={[{
            label: t("removeFolder"),
            danger: true,
            icon: <Trash />,
            onClick: () => {
              setPendingFolder({
                path: row.path,
                projectIds: allProjects.filter((project) => isSameOrAncestorPath(row.path, project.canonicalPath)).map((project) => project.id),
                rootCount: scanRootsUnderPath(scanRoots, row.path).length,
              });
            },
          }]}
        /> : <div className="virtual-row-project-content">
          {editing?.id === row.project.id ? <form className="project-card-editor" onKeyDown={(event) => event.stopPropagation()} onSubmit={(event) => { event.preventDefault(); void commitEdit(); }}>
            <div className="project-card-editor-field">
              <span className="project-identity-mark" aria-hidden="true">{icons[row.project.id]?.dataUrl ? <img src={icons[row.project.id].dataUrl ?? undefined} alt="" /> : <LanguageGlyph language={languageFallback(row.project)} />}</span>
              <label>
                <span>{editing.field === "name" ? t("editName") : t("editDescription")}</span>
                {editing.field === "name"
                  ? <input autoFocus value={draft} onChange={(event) => setDraft(event.target.value)} onKeyDown={(event) => { if (event.key === "Escape") setEditing(null); }} />
                  : <textarea autoFocus value={draft} onChange={(event) => setDraft(event.target.value)} onKeyDown={(event) => { if (event.key === "Escape") setEditing(null); }} />}
              </label>
            </div>
            <div className="project-card-editor-actions">
              <Button size="sm" variant="quiet" type="button" onClick={() => setEditing(null)}>{t("cancel")}</Button>
              <Button size="sm" variant="primary" type="submit">{t("save")}</Button>
            </div>
          </form> : <>
            <ItemContextMenu
              trigger={<button
               className={"project-row" + (selected ? " active" : "") + (row.project.description ? " has-blurb" : "")}
                tabIndex={-1}
                data-context-row={row.id}
                onMouseDown={(event) => { if (event.button === 0) event.preventDefault(); focusTree(); }}
                onClick={() => { focusTree(); setActiveRowId(row.id); onSelect(row.project.id); }}
                aria-label={row.project.displayName}
                onContextMenu={() => setActiveRowId(row.id)}
              >
                <span className="project-card-layout">
                  <span className="project-card-copy">
                    <span className="project-row-main"><span className="project-identity-mark" aria-hidden="true">{icons[row.project.id]?.dataUrl ? <img src={icons[row.project.id].dataUrl ?? undefined} alt="" /> : <LanguageGlyph language={languageFallback(row.project)} />}</span><span className="project-name">{row.project.displayName}</span>{row.project.favorite && <Star className="favorite-mark" weight="fill" aria-label={t("favorite")} />}</span>
                    <code className="project-path" title={row.project.canonicalPath}>{row.project.canonicalPath}</code>
                    {row.project.description && <span className="project-blurb">{row.project.description}</span>}
                    <span className="project-meta">{stackOf(row.project).slice(0, 2).map((value) => <span className="badge" key={value}>{value}</span>)}<time>{formatTime(row.project.sourceMtime)}</time></span>
                  </span>
                </span>
              </button>}
              items={[
                { label: t("editName"), icon: <PencilSimple />, onClick: () => { setEditing({ id: row.project.id, field: "name" }); setDraft(row.project.displayName); } },
                { label: t("editDescription"), icon: <NotePencil />, onClick: () => { setEditing({ id: row.project.id, field: "description" }); setDraft(row.project.description ?? ""); } },
                { label: t("changeProjectIcon"), icon: <Image />, onClick: () => void chooseProjectIcon(row.project.id) },
                { label: t("resetProjectIcon"), icon: <ArrowCounterClockwise />, onClick: () => void clearProjectIcon(row.project.id) },
                { label: t("copyPath"), icon: <Copy />, onClick: () => void navigator.clipboard?.writeText(row.project.canonicalPath) },
                ...(onReveal ? [{ label: t("revealInExplorer"), icon: <FolderSimple />, onClick: () => onReveal(row.project.canonicalPath) }] : []),
                ...(onRelocate ? [{ label: t("relocateProject"), icon: <MapPin />, onClick: () => void onRelocate(row.project.id) }] : []),
                ...(selectedCollectionId && onRemoveFromCollection ? [{ label: t("removeFromCollection"), icon: <XCircle />, onClick: () => void onRemoveFromCollection(row.project.id, selectedCollectionId) }] : []),
                ...(!selectedCollectionId && onAddToCollection ? collections.map((collection) => ({ label: `${t("addToCollection")} · ${collection.name}`, icon: <SquaresFour />, onClick: () => void onAddToCollection(row.project.id, collection.id) })) : []),
                { label: t("removeRecord"), danger: true, icon: <Trash />, onClick: () => setPendingProject(row.project) },
              ]}
            />
          </>}
        </div>}
      </div>
    </div>;
  }

  return (
    <section className="list-pane" aria-label={t("projects")}>
      <header className="list-header">
        <div className="list-heading"><div><h1>{t("projects")}</h1><span className="project-count">{projects.length}</span></div></div>
        <div className="list-header-actions">
          <ActionMenu
            trigger={
              <button type="button" className="library-actions-trigger" aria-haspopup="menu" aria-label={t("projectActions")} title={t("projectActions")}>
                <Plus weight="bold" aria-hidden="true" />
              </button>
            }
            items={[
              { label: t("register"), icon: <PlusCircle aria-hidden="true" />, onClick: onRegister },
              { label: t("addRoot"), icon: <FolderPlus aria-hidden="true" />, onClick: onAddRoot },
              { label: t("scanAll"), icon: <MagnifyingGlass aria-hidden="true" />, disabled: scanning, onClick: onScan },
            ]}
          />
        </div>
      </header>
      <div className="list-toolbar">
        <label className="search-field">
          <span className="sr-only">{t("searchHint")}</span><MagnifyingGlass aria-hidden="true" />
          <input value={query} placeholder={t("searchHint")} onChange={(event) => onQuery(event.target.value)} />
        </label>
        <div className="list-toolbar-primary">
          {onScope && <div className="scope-tabs" role="tablist" aria-label={t("projects")}>
            {scopeOptions.map((value) => { const ScopeIcon = scopeIcons[value]; return <button key={value} type="button" role="tab" aria-label={t(scopeKeys[value])} title={t(scopeKeys[value])} aria-selected={scope === value} tabIndex={scope === value ? 0 : -1} className={scope === value ? "active" : ""} onKeyDown={moveTabFocus} onClick={() => onScope(value)}><ScopeIcon weight={scope === value ? "fill" : "regular"} aria-hidden="true" /></button>; })}
          </div>}
          {collectionControls}
        </div>
        <div className="list-toolbar-meta">
          <div className="facet-row" aria-label={t("filters")}>
            <select aria-label={t("languageFilter")} value={filters.language} onChange={(event) => onFilters({ ...filters, language: event.target.value })}>
              <option value="">{t("allLanguages")}</option>{facets.languages.map((value) => <option key={value} value={value}>{value}</option>)}
            </select>
            {facets.tags.length > 0 && <select aria-label={t("tagFilter")} value={filters.tag} onChange={(event) => onFilters({ ...filters, tag: event.target.value })}>
              <option value="">{t("allTags")}</option>{facets.tags.map((value) => <option key={value} value={value}>{value}</option>)}
            </select>}
            {(query.trim() || hasFacetFilters) && <button className="clear-filters" onClick={() => { onQuery(""); onFilters({ language: "", tag: "" }); }}>{t("clear")}</button>}
          </div>
          <div className="list-view-tools">
            <div className="list-tree-tools" role="group" aria-label={t("treeTools")}>
              <button type="button" disabled={layout !== "tree" || projects.length === 0} aria-label={t("expandAll")} title={t("expandAll")} onClick={expandAllGroups}><CaretDoubleDown aria-hidden="true" /></button>
              <button type="button" disabled={layout !== "tree" || projects.length === 0} aria-label={t("collapseAll")} title={t("collapseAll")} onClick={collapseAllGroups}><CaretDoubleUp aria-hidden="true" /></button>
              <button type="button" disabled={layout !== "tree" || !selectedInList} aria-label={t("focusSelected")} title={t("focusSelected")} onClick={collapseToSelected}><ArrowsInSimple aria-hidden="true" /></button>
              <button type="button" disabled={!selectedInList} aria-label={t("revealSelected")} title={t("revealSelected")} onClick={revealSelectedProject}><Crosshair aria-hidden="true" /></button>
            </div>
            <ActionMenu
              trigger={
                <button type="button" className="sort-trigger" aria-haspopup="menu" aria-label={t("sortBy")} title={t(sortLabels[sort])}>
                  <SortAscending aria-hidden="true" />
                </button>
              }
              items={sortOptions.map((value) => ({
                label: t(sortLabels[value]),
                icon: sort === value ? <Check aria-hidden="true" /> : undefined,
                onClick: () => onSort?.(value),
              }))}
            />
            <div className="list-view-toggle" role="group" aria-label={t("listLayout")}>
              <button type="button" className={layout === "tree" ? "active" : ""} aria-pressed={layout === "tree"} aria-label={t("treeView")} onClick={() => setLayout("tree")}><TreeStructure aria-hidden="true" /></button>
              <button type="button" className={layout === "flat" ? "active" : ""} aria-pressed={layout === "flat"} aria-label={t("flatView")} onClick={() => setLayout("flat")}><ListBullets aria-hidden="true" /></button>
            </div>
          </div>
        </div>
      </div>
      <div className={`scan-status ${scanning ? "is-active" : "is-idle"}`} role="status" aria-live="polite" hidden={!scanning}>
        <div className="scan-sweep" aria-hidden="true" />
        <div className="scan-status-copy"><strong>{t("scanning")}</strong><span>{progress?.discovered ?? 0} {t("found")} · {progress?.visited ?? 0} {t("visited")}</span><code title={progress?.currentPath ?? progress?.rootPath}>{progress?.currentPath ?? progress?.rootPath ?? t("preparingScan")}</code></div>
        <Button variant="quiet" onClick={onCancelScan}>{t("cancel")}</Button>
      </div>
      <div
        ref={parentRef}
        className="project-scroll"
        role={layout === "tree" ? "tree" : "listbox"}
        aria-label={t("projects")}
        aria-activedescendant={activeRowId && visibleRows.length > 0 ? activeRowId : undefined}
        tabIndex={0}
        onKeyDown={onTreeKeyDown}
      >
        {projects.length === 0 ? <EmptyState compact title={hasTextFilter ? t("noMatches") : t("emptyTitle")} body={hasTextFilter ? t("adjustFilters") : empty} /> :
          <div className="virtual-list" style={{ height: virtualizer.getTotalSize() }}>
            {virtualizer.getVirtualItems().map((virtualRow) => {
              const row = visibleRows[virtualRow.index];
              return row ? renderRow(row, virtualRow) : null;
            })}
          </div>}
      </div>
      <footer className="list-footer" hidden />
      <ConfirmDialog
        open={Boolean(pendingProject)}
        title={t("confirmRemove")}
        body={pendingProject ? `${pendingProject.displayName}
${pendingProject.canonicalPath}
${t("removeRecordHint")}` : ""}
        confirmLabel={t("removeRecord")}
        cancelLabel={t("cancel")}
        onOpenChange={(open) => !open && setPendingProject(undefined)}
        onConfirm={async () => {
          if (!pendingProject) return;
          await onRemoveProject?.(pendingProject.id);
          setPendingProject(undefined);
        }}
      />
      <ConfirmDialog
        open={Boolean(pendingFolder)}
        title={t("confirmRemoveFolder")}
        body={pendingFolder ? `${pendingFolder.path}
${t("confirmRemoveFolderHint")}${pendingFolder.rootCount ? `
${t("confirmRemoveFolderRootsHint")}` : ""}` : ""}
        confirmLabel={t("removeFolder")}
        cancelLabel={t("cancel")}
        onOpenChange={(open) => !open && setPendingFolder(undefined)}
        onConfirm={async () => {
          if (!pendingFolder) return;
          await onRemoveFolder?.(pendingFolder.path, pendingFolder.projectIds);
          setPendingFolder(undefined);
        }}
      />
    </section>
  );

  async function commitEdit() {
    if (!editing) return;
    const value = draft.trim();
    try {
      if (editing.field === "name") {
        if (value) await onRename?.(editing.id, value);
      } else {
        await onDescription?.(editing.id, value || null);
      }
      setEditing(null);
    } catch {
      // The parent owns user-facing feedback; keep the editor and draft intact.
    }
  }
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
