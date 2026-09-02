import { Dialog } from "@base-ui/react/dialog";
import {
  ArrowsClockwise,
  ArrowDown,
  ArrowElbowDownLeft,
  ArrowUp,
  DownloadSimple,
  FolderSimple,
  FolderSimplePlus,
  GearSix,
  ListBullets,
  Lightning,
  MagnifyingGlass,
  MapPin,
  Question,
  ShieldCheck,
  Scan,
  Sparkle,
  Star,
  X,
} from "@phosphor-icons/react";
import { AnimatePresence, motion, MotionConfig } from "framer-motion";
import { useEffect, useId, useMemo, useRef, useState, type ReactNode } from "react";
import type { MessageKey } from "../i18n";
import { api } from "../lib/api";
import type { ProjectSummary, SearchHit } from "../types";

interface Action {
  id: string;
  title: string;
  hint?: string;
  run: () => void;
}

type PaletteItem = {
  key: string;
  title: string;
  hint?: string;
  kind: "action" | "project";
  icon: ReactNode;
  run: () => void;
};

function itemDomId(listId: string, key: string) {
  return `${listId}-item-${key.replace(/[^a-zA-Z0-9_-]/g, "-")}`;
}

function actionIcon(id: string) {
  if (id === "open-help") return <Question size={18} weight="duotone" aria-hidden />;
  if (id === "open-settings") return <GearSix size={18} weight="duotone" aria-hidden />;
  if (id === "open-tasks") return <ListBullets size={18} weight="duotone" aria-hidden />;
  if (id === "open-approvals") return <ShieldCheck size={18} weight="duotone" aria-hidden />;
  if (id === "refresh-project") return <ArrowsClockwise size={18} weight="duotone" aria-hidden />;
  if (id === "reveal-project") return <MapPin size={18} weight="duotone" aria-hidden />;
  if (id === "favorite-project") return <Star size={18} weight="duotone" aria-hidden />;
  if (id === "check-updates") return <DownloadSimple size={18} weight="duotone" aria-hidden />;
  if (id === "clear-filters") return <X size={18} weight="duotone" aria-hidden />;
  if (id === "scan-all") return <Scan size={18} weight="duotone" aria-hidden />;
  if (id === "add-root") return <FolderSimplePlus size={18} weight="duotone" aria-hidden />;
  if (id === "register") return <FolderSimple size={18} weight="duotone" aria-hidden />;
  return <Lightning size={18} weight="duotone" aria-hidden />;
}

function projectItemsFrom(
  source: ProjectSummary[],
  actionsById: Map<string, Action>,
  onProject?: (id: string) => void,
): PaletteItem[] {
  return source.slice(0, 8).flatMap((project) => {
    const jump = actionsById.get(`jump:${project.id}`);
    const run = jump?.run ?? (onProject ? () => onProject(project.id) : undefined);
    if (!run) return [];
    const stack = [...project.languages, ...project.frameworks].slice(0, 2).join(" · ");
    return [{
      key: `project:${project.id}`,
      title: project.displayName,
      hint: stack ? `${project.canonicalPath} · ${stack}` : project.canonicalPath,
      kind: "project",
      icon: project.favorite
        ? <Star size={18} weight="fill" aria-hidden />
        : <FolderSimple size={18} weight="duotone" aria-hidden />,
      run,
    }];
  });
}

export function CommandPalette({
  open,
  onClose,
  t,
  projects,
  actions,
  onProject,
}: {
  open: boolean;
  onClose: () => void;
  t: (key: MessageKey) => string;
  projects: ProjectSummary[];
  actions: Action[];
  onProject?: (id: string) => void;
}) {
  const [query, setQuery] = useState("");
  const [searchResults, setSearchResults] = useState<SearchHit[]>([]);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [activeIndex, setActiveIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const searchRequestRef = useRef(0);
  const justOpenedRef = useRef(false);
  const listId = useId().replace(/:/g, "");
  const shortcut = /mac|iphone|ipad|ipod/i.test(navigator.userAgent) ? "⌘ K" : "Ctrl K";

  const actionsById = useMemo(() => new Map(actions.map((action) => [action.id, action])), [actions]);
  const normalizedQuery = query.trim().toLowerCase();

  const operationItems = useMemo<PaletteItem[]>(() => actions
    .filter((action) => !action.id.startsWith("jump:"))
    .filter((action) => !normalizedQuery || action.title.toLowerCase().includes(normalizedQuery))
    .map((action) => ({
      key: `action:${action.id}`,
      title: action.title,
      hint: action.hint,
      kind: "action",
      icon: actionIcon(action.id),
      run: action.run,
    })), [actions, normalizedQuery]);

  const projectItems = useMemo(() => {
    const source = normalizedQuery
      ? searchResults.map((hit) => hit.project)
      : projects;
    return projectItemsFrom(source, actionsById, onProject);
  }, [actionsById, normalizedQuery, onProject, projects, searchResults]);

  const groups = useMemo(() => [
    { key: "actions", label: t("commands"), items: operationItems },
    { key: "projects", label: t("projects"), items: projectItems },
  ].filter((group) => group.items.length > 0), [operationItems, projectItems, t]);

  const items = useMemo(() => groups.flatMap((group) => group.items), [groups]);
  const activeItem = items[activeIndex];
  const activeItemId = activeItem ? itemDomId(listId, activeItem.key) : undefined;

  useEffect(() => {
    if (!open) {
      searchRequestRef.current += 1;
      justOpenedRef.current = false;
      setQuery("");
      setSearchResults([]);
      setSearching(false);
      setSearchError(null);
      setActiveIndex(0);
      return;
    }

    justOpenedRef.current = true;
    searchRequestRef.current += 1;
    setQuery("");
    setSearchResults([]);
    setSearching(false);
    setSearchError(null);
    setActiveIndex(0);

    const frame = window.requestAnimationFrame(() => inputRef.current?.focus());
    return () => window.cancelAnimationFrame(frame);
  }, [open]);

  useEffect(() => {
    if (!open) return;
    if (justOpenedRef.current) {
      justOpenedRef.current = false;
      return;
    }

    const q = query.trim();
    const requestId = ++searchRequestRef.current;
    setActiveIndex(0);
    setSearchError(null);

    if (!q) {
      setSearchResults([]);
      setSearching(false);
      return;
    }

    setSearching(true);
    setSearchResults([]);
    void api.searchProjects(q).then((results) => {
      if (searchRequestRef.current !== requestId) return;
      setSearchResults(results);
    }).catch((err) => {
      if (searchRequestRef.current !== requestId) return;
      setSearchResults([]);
      setSearchError(String(err));
    }).finally(() => {
      if (searchRequestRef.current === requestId) setSearching(false);
    });

    return () => {
      if (searchRequestRef.current === requestId) searchRequestRef.current += 1;
    };
  }, [open, query]);

  useEffect(() => {
    setActiveIndex((current) => Math.min(current, Math.max(items.length - 1, 0)));
  }, [items.length]);

  useEffect(() => {
    if (!activeItemId) return;
    document.getElementById(activeItemId)?.scrollIntoView({ block: "nearest" });
  }, [activeItemId]);

  function activate(item: PaletteItem) {
    item.run();
    onClose();
  }

  function onInputKeyDown(event: React.KeyboardEvent<HTMLInputElement>) {
    if (event.key === "Escape") {
      event.preventDefault();
      onClose();
      return;
    }
    if (event.key === "ArrowDown") {
      event.preventDefault();
      if (items.length) setActiveIndex((current) => (current + 1) % items.length);
      return;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      if (items.length) setActiveIndex((current) => (current - 1 + items.length) % items.length);
      return;
    }
    if (event.key === "Enter" && activeItem) {
      event.preventDefault();
      activate(activeItem);
    }
  }

  return (
    <MotionConfig reducedMotion="user">
      <Dialog.Root open={open} onOpenChange={(next) => !next && onClose()}>
        <Dialog.Portal>
          <Dialog.Backdrop className="command-backdrop" />
          <Dialog.Popup className="command-workbench" aria-describedby={`${listId}-description`}>
            <motion.div
              className="command-panel"
              initial={{ opacity: 0, y: -8, scale: 0.985 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              transition={{ duration: 0.16, ease: "easeOut" }}
            >
              <Dialog.Title className="command-visually-hidden">{t("commands")}</Dialog.Title>
              <Dialog.Description id={`${listId}-description`} className="command-visually-hidden">
                {t("commandHint")}
              </Dialog.Description>

              <div className="command-search-row">
                <MagnifyingGlass size={20} weight="regular" className="command-search-icon" aria-hidden />
                <input
                  ref={inputRef}
                  autoFocus
                  role="combobox"
                  aria-label={t("commandHint")}
                  aria-autocomplete="list"
                  aria-haspopup="listbox"
                  aria-controls={`${listId}-list`}
                  aria-expanded={open}
                  aria-activedescendant={activeItemId ?? undefined}
                  aria-describedby={`${listId}-description`}
                  aria-busy={searching}
                  className="command-search-input"
                  placeholder={t("commandHint")}
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  onKeyDown={onInputKeyDown}
                />
                <kbd className="command-search-key" aria-hidden>{shortcut}</kbd>
              </div>

              <div id={`${listId}-list`} role="listbox" className="command-results" aria-label={t("commandHint")} aria-busy={searching}>
                <AnimatePresence mode="wait" initial={false}>
                  {searching ? (
                    <motion.div
                      key="searching"
                      className="command-status"
                      role="status"
                      aria-live="polite"
                      initial={{ opacity: 0 }}
                      animate={{ opacity: 1 }}
                      exit={{ opacity: 0 }}
                    >
                      <MagnifyingGlass size={18} className="command-status-icon" aria-hidden />
                      <span>{t("search")}</span>
                    </motion.div>
                  ) : items.length === 0 ? (
                    <motion.div
                      key="empty"
                      className="command-status"
                      role="status"
                      aria-live="polite"
                      initial={{ opacity: 0 }}
                      animate={{ opacity: 1 }}
                      exit={{ opacity: 0 }}
                    >
                      <Sparkle size={18} weight="duotone" className="command-status-icon" aria-hidden />
                      <span>{searchError ?? t("paletteEmpty")}</span>
                    </motion.div>
                  ) : (
                    <motion.div
                      key={`results:${normalizedQuery}:${items.length}`}
                      className="command-groups"
                      role="presentation"
                      initial={{ opacity: 0, y: 4 }}
                      animate={{ opacity: 1, y: 0 }}
                      exit={{ opacity: 0, y: -2 }}
                      transition={{ duration: 0.12, ease: "easeOut" }}
                    >
                      {groups.map((group) => {
                        const groupLabelId = `${listId}-group-${group.key}`;
                        return <div className="command-group" key={group.key} role="group" aria-labelledby={groupLabelId}>
                          <div id={groupLabelId} className="command-group-label">{group.label}</div>
                          {group.items.map((item) => {
                            const itemIndex = items.indexOf(item);
                            const selected = itemIndex === activeIndex;
                            const id = itemDomId(listId, item.key);
                            return (
                              <button
                                id={id}
                                key={item.key}
                                type="button"
                                role="option"
                                aria-selected={selected}
                                className={`command-item ${selected ? "command-item-active" : ""}`}
                                onMouseEnter={() => setActiveIndex(itemIndex)}
                                onClick={() => activate(item)}
                              >
                                <span className="command-item-icon" aria-hidden>{item.icon}</span>
                                <span className="command-item-copy">
                                  <span className="command-item-title">{item.title}</span>
                                  {item.hint && <span className="command-item-hint">{item.hint}</span>}
                                </span>
                                {item.kind === "project" && <FolderSimple size={16} className="command-item-affordance" aria-hidden />}
                              </button>
                            );
                          })}
                        </div>;
                      })}
                    </motion.div>
                  )}
                </AnimatePresence>
                {searchError && items.length > 0 && <div className="command-error" role="alert">{searchError}</div>}
              </div>

              <footer className="command-footer" aria-hidden>
                <span className="command-footer-hint"><ArrowUp size={14} /><ArrowDown size={14} /><span>{t("navigate")}</span></span>
                <span className="command-footer-hint"><ArrowElbowDownLeft size={14} /><span>{t("openItem")}</span></span>
                <span className="command-footer-hint"><span className="command-key">Esc</span><span>{t("close")}</span></span>
              </footer>
            </motion.div>
          </Dialog.Popup>
        </Dialog.Portal>
      </Dialog.Root>
    </MotionConfig>
  );
}
