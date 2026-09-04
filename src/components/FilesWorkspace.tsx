import {
  asyncDataLoaderFeature,
  hotkeysCoreFeature,
  selectionFeature,
} from "@headless-tree/core";
import { useTree } from "@headless-tree/react";
import { useVirtualizer } from "@tanstack/react-virtual";
import {
  ArrowClockwise,
  CaretRight,
  File,
  FileCode,
  FileImage,
  FileText,
  Folder,
  FolderOpen,
  MagnifyingGlass,
  SpinnerGap,
  Warning,
  X,
} from "@phosphor-icons/react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { MessageKey } from "../i18n";
import { api, onFileIndexProgress } from "../lib/api";
import type {
  ProjectDirectoryEntry,
  ProjectFilePreview,
  ProjectPathIndexStatus,
  ProjectPathSearchResult,
  ProjectSummary,
  ToastTone,
} from "../types";
import { CodePreview } from "./CodePreview";
import { MarkdownDocument } from "./MarkdownDocument";
import { Button } from "./ui/button";

const ROOT_ID = "repoatlas://root";

type TreeEntry = ProjectDirectoryEntry & { loading?: boolean };

export function fileTreeIndent(level: number) {
  return 8 + Math.max(0, level) * 18;
}

function formatSize(size: number) {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(size < 10 * 1024 ? 1 : 0)} KiB`;
  return `${(size / (1024 * 1024)).toFixed(1)} MiB`;
}

function parentPaths(path: string) {
  const parts = path.split("/");
  const result: string[] = [];
  for (let index = 1; index < parts.length; index += 1) result.push(parts.slice(0, index).join("/"));
  return result;
}

function entryIcon(entry: Pick<TreeEntry, "kind" | "name">, expanded = false) {
  if (entry.kind === "directory") return expanded ? FolderOpen : Folder;
  if (/\.(png|jpe?g|gif|webp|bmp|ico)$/i.test(entry.name)) return FileImage;
  if (/\.(md|markdown|txt|rst)$/i.test(entry.name)) return FileText;
  if (/\.(rs|tsx?|jsx?|py|go|java|kt|swift|c|cpp|h|css|scss|json|ya?ml|toml)$/i.test(entry.name)) return FileCode;
  return File;
}

function ProjectTree({ projectId, projectName, showGenerated, refreshGeneration, selectedPath, revealPath, expandedPaths, onExpandedPaths, onSelect, onRevealComplete, t }: {
  projectId: string;
  projectName: string;
  showGenerated: boolean;
  refreshGeneration: number;
  selectedPath?: string;
  revealPath?: string;
  expandedPaths: string[];
  onExpandedPaths: (paths: string[]) => void;
  onSelect: (entry: ProjectDirectoryEntry) => void;
  onRevealComplete: () => void;
  t: (key: MessageKey) => string;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [rootLoading, setRootLoading] = useState(true);
  const data = useRef(new Map<string, TreeEntry>());
  const root = useMemo<TreeEntry>(() => ({ name: projectName, path: "", kind: "directory", generated: false }), [projectName]);
  data.current.set(ROOT_ID, root);

  const tree = useTree<TreeEntry>({
    rootItemId: ROOT_ID,
    initialState: { expandedItems: expandedPaths, selectedItems: selectedPath ? [selectedPath] : [] },
    state: { expandedItems: expandedPaths, selectedItems: selectedPath ? [selectedPath] : [] },
    setExpandedItems: (value) => {
      const next = typeof value === "function" ? value(expandedPaths) : value;
      onExpandedPaths(next);
    },
    setSelectedItems: () => undefined,
    getItemName: (item) => item.getItemData()?.name ?? t("loadingFiles"),
    isItemFolder: (item) => item.getItemData()?.kind === "directory",
    onPrimaryAction: (item) => {
      const entry = item.getItemData();
      if (!entry || item.getId() === ROOT_ID || entry.loading) return;
      onSelect(entry);
    },
    dataLoader: {
      getItem: async (itemId) => data.current.get(itemId) ?? { name: itemId.split("/").slice(-1)[0] ?? itemId, path: itemId, kind: "other", generated: false, loading: true },
      getChildrenWithData: async (itemId) => {
        const path = itemId === ROOT_ID ? "" : itemId;
        try {
          const listing = await api.listProjectDirectory(projectId, path, showGenerated);
          if (itemId === ROOT_ID) setRootLoading(false);
          setErrors((current) => {
            if (!current[itemId]) return current;
            const next = { ...current };
            delete next[itemId];
            return next;
          });
          return listing.entries.map((entry) => {
            data.current.set(entry.path, entry);
            return { id: entry.path, data: entry };
          });
        } catch (error) {
          if (itemId === ROOT_ID) setRootLoading(false);
          setErrors((current) => ({ ...current, [itemId]: String(error) }));
          return [];
        }
      },
    },
    createLoadingItemData: () => ({ name: t("loadingFiles"), path: "", kind: "other", generated: false, loading: true }),
    features: [asyncDataLoaderFeature, selectionFeature, hotkeysCoreFeature],
  });

  const items = tree.getItems();
  const virtualizer = useVirtualizer({ count: items.length, getScrollElement: () => scrollRef.current, estimateSize: () => 29, overscan: 10, initialRect: { width: 320, height: 600 } });

  useEffect(() => {
    if (!revealPath) return;
    let active = true;
    void (async () => {
      await tree.loadChildrenIds(ROOT_ID);
      for (const parent of parentPaths(revealPath)) {
        if (!active) return;
        await tree.loadChildrenIds(parent);
        tree.getItemInstance(parent).expand();
      }
      if (!active) return;
      tree.rebuildTree();
      const index = tree.getItems().findIndex((item) => item.getId() === revealPath);
      if (index >= 0) virtualizer.scrollToIndex(index, { align: "center" });
      onRevealComplete();
    })().catch(() => onRevealComplete());
    return () => { active = false; };
  }, [revealPath, refreshGeneration]);

  return <div ref={scrollRef} className="files-tree-scroll">
    {rootLoading && <div className="files-tree-loading" role="status"><SpinnerGap className="spin" />{t("loadingFiles")}</div>}
    {errors[ROOT_ID] && <div className="files-index-notice is-error"><Warning /><span>{errors[ROOT_ID]}</span><button onClick={() => void tree.getRootItem().invalidateChildrenIds()}>{t("retry")}</button></div>}
    <div {...tree.getContainerProps(t("files"))} className="files-tree" style={{ height: virtualizer.getTotalSize() }}>
      {virtualizer.getVirtualItems().map((row) => {
        const item = items[row.index];
        if (!item) return null;
        const entry = item.getItemData();
        const isRoot = item.getId() === ROOT_ID;
        const Icon = entryIcon(entry, item.isExpanded());
        const props = item.getProps();
        return <div className="files-tree-row-wrap" key={item.getKey()} style={{ transform: `translateY(${row.start}px)` }}>
          <button {...props} className={`files-tree-row${item.isSelected() || item.getId() === selectedPath ? " is-selected" : ""}${item.isFocused() ? " is-focused" : ""}${entry.generated ? " is-generated" : ""}`} style={{ paddingInlineStart: `${fileTreeIndent(item.getItemMeta().level)}px` }} title={entry.path || projectName}>
            <span className="files-tree-caret-slot" aria-hidden="true">{entry.kind === "directory" ? <CaretRight className="files-tree-caret" weight="bold" style={{ transform: item.isExpanded() ? "rotate(90deg)" : undefined }} /> : null}</span>
            <span className={`files-tree-icon${entry.kind === "directory" ? " is-directory" : " is-file"}`} aria-hidden="true">{item.isLoading() ? <SpinnerGap className="spin" /> : <Icon weight={item.isExpanded() && entry.kind === "directory" ? "fill" : "regular"} />}</span>
            <span className="files-tree-name">{entry.name}</span>
            {entry.kind === "symlink" && <span className="files-tree-kind">link</span>}
          </button>
          {errors[item.getId()] && !isRoot && <button className="files-tree-retry" title={errors[item.getId()]} onClick={() => void item.invalidateChildrenIds()}>{t("retry")}</button>}
        </div>;
      })}
    </div>
  </div>;
}

function ImagePreview({ projectId, preview }: { projectId: string; preview: ProjectFilePreview }) {
  const [url, setUrl] = useState<string>();
  const [error, setError] = useState<string>();
  useEffect(() => {
    let active = true;
    let objectUrl: string | undefined;
    setError(undefined);
    void api.readProjectImage(projectId, preview.path).then((bytes) => {
      if (!active) return;
      objectUrl = URL.createObjectURL(new Blob([bytes], { type: preview.mime ?? undefined }));
      setUrl(objectUrl);
    }).catch((reason) => { if (active) setError(String(reason)); });
    return () => {
      active = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [preview.mime, preview.path, projectId]);
  return url ? <div className="files-image-preview"><img src={url} alt={preview.path} /></div>
    : error ? <div className="files-empty-preview is-error"><Warning /><code>{error}</code></div>
      : <div className="files-empty-preview"><SpinnerGap className="spin" /></div>;
}

function FilePreviewPane({ project, preview, loading, error, selectedPath, onOpenPath, notify, t }: {
  project: ProjectSummary;
  preview?: ProjectFilePreview;
  loading: boolean;
  error?: string;
  selectedPath?: string;
  onOpenPath: (path: string) => void;
  notify: (tone: ToastTone, title: string, detail?: string) => void;
  t: (key: MessageKey) => string;
}) {
  return <section className="files-preview-pane" aria-label={t("filePreview")}>
    <header className="files-preview-header">
      <div><strong>{selectedPath || t("selectProjectFile")}</strong>{preview && <span>{preview.language ?? preview.mime ?? preview.kind} · {formatSize(preview.size)}{preview.width && preview.height ? ` · ${preview.width}×${preview.height}` : ""}</span>}</div>
      {selectedPath && <div className="files-preview-actions">
        <Button size="sm" onClick={() => void navigator.clipboard.writeText(selectedPath).then(() => notify("success", t("copied"), selectedPath)).catch((reason) => notify("error", t("copyFailed"), String(reason)))}>{t("copyPath")}</Button>
        <Button size="sm" onClick={() => void api.revealProjectPath(project.id, selectedPath).catch((reason) => notify("error", t("openFailed"), String(reason)))}>{t("revealInExplorer")}</Button>
        {preview?.kind === "unsupported" && <Button size="sm" onClick={() => void api.openProjectFile(project.id, selectedPath).catch((reason) => notify("error", t("openFailed"), String(reason)))}>{t("openInSystem")}</Button>}
      </div>}
    </header>
    <div className="files-preview-body">
      {loading ? <div className="files-empty-preview"><SpinnerGap className="spin" /><span>{t("loadingFile")}</span></div>
        : error ? <div className="files-empty-preview is-error"><Warning /><strong>{t("filePreviewFailed")}</strong><code>{error}</code></div>
          : !preview ? <div className="files-empty-preview"><FileText /><strong>{t("selectProjectFile")}</strong><span>{t("selectProjectFileHint")}</span></div>
            : preview.kind === "markdown" && preview.content !== null ? <div className="files-markdown-preview"><MarkdownDocument content={preview.content} projectId={project.id} documentPath={preview.path} onOpenPath={onOpenPath} /></div>
              : (preview.kind === "code" || preview.kind === "text") && preview.content !== null ? <CodePreview code={preview.content} language={preview.language} />
                : preview.kind === "image" && !preview.message ? <ImagePreview projectId={project.id} preview={preview} />
                  : <div className="files-empty-preview"><File /><strong>{t("previewUnsupported")}</strong><span>{preview.message ?? preview.mime ?? ""}</span></div>}
    </div>
    {preview?.truncated && <div className="files-truncated" role="status"><Warning />{t("fileTruncated")}</div>}
  </section>;
}

export function FilesWorkspace({ project, active, t, notify }: {
  project: ProjectSummary;
  active: boolean;
  t: (key: MessageKey) => string;
  notify: (tone: ToastTone, title: string, detail?: string) => void;
}) {
  const [activated, setActivated] = useState(active);
  const [showGenerated, setShowGenerated] = useState(false);
  const [searchGenerated, setSearchGenerated] = useState(false);
  const searchGeneratedRef = useRef(searchGenerated);
  const [query, setQuery] = useState("");
  const queryRef = useRef(query);
  const [results, setResults] = useState<ProjectPathSearchResult[]>([]);
  const [indexStatus, setIndexStatus] = useState<ProjectPathIndexStatus>();
  const [expandedPaths, setExpandedPaths] = useState<string[]>([]);
  const [selectedPath, setSelectedPath] = useState<string>();
  const [selectedEntry, setSelectedEntry] = useState<ProjectDirectoryEntry>();
  const [revealPath, setRevealPath] = useState<string>();
  const [preview, setPreview] = useState<ProjectFilePreview>();
  const [previewLoading, setPreviewLoading] = useState(false);
  const [previewError, setPreviewError] = useState<string>();
  const [refreshGeneration, setRefreshGeneration] = useState(0);
  const requestSequence = useRef(0);
  const searchSequence = useRef(0);

  useEffect(() => { if (active) setActivated(true); }, [active]);
  useEffect(() => {
    setActivated(active);
    setQuery(""); setResults([]); setIndexStatus(undefined); setExpandedPaths([]); setSelectedPath(undefined); setSelectedEntry(undefined); setPreview(undefined); setPreviewError(undefined); setRefreshGeneration(0);
    return () => { void api.cancelProjectPathIndex(project.id); };
  }, [project.id]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void onFileIndexProgress((status) => {
      if (status.projectId !== project.id || status.includeGenerated !== searchGeneratedRef.current) return;
      setIndexStatus((current) => current && status.generation < current.generation ? current : status);
      if ((status.state === "ready" || status.state === "limited") && queryRef.current.trim()) {
        const sequence = ++searchSequence.current;
        void api.searchProjectPaths(project.id, queryRef.current.trim()).then((response) => {
          if (sequence === searchSequence.current && queryRef.current.trim()) setResults(response.results);
        }).catch((reason) => {
          if (sequence === searchSequence.current) setIndexStatus((current) => current ? { ...current, state: "failed", message: String(reason) } : current);
        });
      }
    }).then((dispose) => { unlisten = dispose; });
    return () => unlisten?.();
  }, [project.id]);

  const selectPath = useCallback((entry: ProjectDirectoryEntry) => {
    const sequence = ++requestSequence.current;
    setSelectedEntry(entry); setSelectedPath(entry.path); setRevealPath(entry.path);
    if (entry.kind !== "file") { setPreview(undefined); setPreviewError(undefined); setPreviewLoading(false); return; }
    setPreviewLoading(true); setPreviewError(undefined);
    void api.readProjectFilePreview(project.id, entry.path).then((value) => {
      if (sequence === requestSequence.current) setPreview(value);
    }).catch((reason) => {
      if (sequence === requestSequence.current) { setPreview(undefined); setPreviewError(String(reason)); }
    }).finally(() => { if (sequence === requestSequence.current) setPreviewLoading(false); });
  }, [project.id]);

  const updateExpandedPaths = useCallback((next: string[]) => {
    setExpandedPaths((current) => current.length === next.length && current.every((path, index) => path === next[index]) ? current : next);
  }, []);

  useEffect(() => {
    queryRef.current = query;
    searchGeneratedRef.current = searchGenerated;
    const sequence = ++searchSequence.current;
    const normalized = query.trim();
    if (!normalized) {
      setResults([]);
      void api.cancelProjectPathSearch(project.id);
      return;
    }
    const timer = window.setTimeout(() => {
      void api.ensureProjectPathIndex(project.id, searchGenerated).then(async (status) => {
        if (sequence !== searchSequence.current || queryRef.current.trim() !== normalized) return;
        setIndexStatus(status);
        if (status.state === "ready" || status.state === "limited") {
          const response = await api.searchProjectPaths(project.id, normalized);
          if (sequence === searchSequence.current && queryRef.current.trim() === normalized) setResults(response.results);
        }
      }).catch((reason) => {
        if (sequence === searchSequence.current) setIndexStatus({ projectId: project.id, generation: 0, state: "failed", scannedCount: 0, indexedCount: 0, includeGenerated: searchGenerated, message: String(reason) });
      });
    }, 180);
    return () => {
      window.clearTimeout(timer);
      void api.cancelProjectPathSearch(project.id);
    };
  }, [project.id, query, refreshGeneration, searchGenerated]);

  function refresh() {
    void api.cancelProjectPathIndex(project.id);
    setIndexStatus(undefined); setResults([]); setRefreshGeneration((value) => value + 1);
    if (selectedEntry?.kind === "file") selectPath(selectedEntry);
  }

  function selectSearchResult(result: ProjectPathSearchResult) {
    selectPath({ name: result.name, path: result.path, kind: result.kind, generated: false });
    setQuery("");
  }

  return <div className="files-workspace" hidden={!active} aria-hidden={!active}>
    <aside className="files-sidebar">
      <div className="files-toolbar">
        <label className="files-search"><MagnifyingGlass /><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder={t("searchProjectFiles")} aria-label={t("searchProjectFiles")} />{query && <button aria-label={t("clearSearch")} onClick={() => setQuery("")}><X /></button>}</label>
        <Button size="icon" aria-label={t("refreshFiles")} onClick={refresh}><ArrowClockwise /></Button>
      </div>
      <div className="files-options">
        <label><input type="checkbox" checked={showGenerated} onChange={(event) => setShowGenerated(event.target.checked)} />{t("showGeneratedDirectories")}</label>
        {query && <label><input type="checkbox" checked={searchGenerated} onChange={(event) => { void api.cancelProjectPathIndex(project.id); setIndexStatus(undefined); setResults([]); setSearchGenerated(event.target.checked); }} />{t("searchGeneratedDirectories")}</label>}
      </div>
      <div className="files-browser-body">
        <div className="files-search-results" hidden={!query}>
          {indexStatus?.state === "building" && <div className="files-index-progress" role="status"><SpinnerGap className="spin" /><span>{t("indexingFiles")} · {indexStatus.indexedCount.toLocaleString()}</span><button onClick={() => { void api.cancelProjectPathIndex(project.id); setIndexStatus({ ...indexStatus, state: "canceled" }); }}>{t("cancel")}</button></div>}
          {indexStatus?.state === "limited" && <div className="files-index-notice"><Warning />{t("fileIndexLimited")}</div>}
          {indexStatus?.state === "failed" && <div className="files-index-notice is-error"><Warning />{indexStatus.message ?? t("fileIndexFailed")}</div>}
          {results.map((result) => {
            const Icon = entryIcon(result);
            return <button key={result.path} className={result.path === selectedPath ? "is-selected" : ""} onClick={() => selectSearchResult(result)}><Icon /><span><strong>{result.name}</strong><small>{result.path}</small></span></button>;
          })}
          {indexStatus && indexStatus.state !== "building" && results.length === 0 && <div className="files-no-results">{t("noFileResults")}</div>}
        </div>
        <div className="files-tree-host" hidden={Boolean(query)}>{activated && <ProjectTree key={`${project.id}:${showGenerated}:${refreshGeneration}`} projectId={project.id} projectName={project.displayName} showGenerated={showGenerated} refreshGeneration={refreshGeneration} selectedPath={selectedPath} revealPath={revealPath} expandedPaths={expandedPaths} onExpandedPaths={updateExpandedPaths} onSelect={selectPath} onRevealComplete={() => setRevealPath(undefined)} t={t} />}</div>
      </div>
    </aside>
    <FilePreviewPane project={project} preview={preview} loading={previewLoading} error={previewError} selectedPath={selectedPath} onOpenPath={(path) => selectPath({ name: path.split("/").slice(-1)[0] ?? path, path, kind: "file", generated: false })} notify={notify} t={t} />
  </div>;
}
