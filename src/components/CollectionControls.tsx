import { Dialog } from "@base-ui/react/dialog";
import { Menu } from "@base-ui/react/menu";
import { Check, PencilSimple, Plus, SquaresFour, Trash } from "@phosphor-icons/react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useDeferredValue, useEffect, useMemo, useRef, useState } from "react";
import type { MessageKey } from "../i18n";
import { api } from "../lib/api";
import type { ProjectCollection, ProjectSummary, ToastTone } from "../types";
import { Button } from "./ui/button";
import { ConfirmDialog } from "./ui/confirm";

export function CollectionControls({
  collections,
  selectedId,
  t,
  notify,
  onSelect,
  onChanged,
}: {
  collections: ProjectCollection[];
  selectedId?: string;
  t: (key: MessageKey) => string;
  notify: (tone: ToastTone, title: string, detail?: string) => void;
  onSelect: (id?: string) => void;
  onChanged: (selectedId?: string) => Promise<void>;
}) {
  const selectedCollection = collections.find((item) => item.id === selectedId);
  const [open, setOpen] = useState(false);
  const [editingId, setEditingId] = useState<string>();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [memberIds, setMemberIds] = useState<Set<string>>(new Set());
  const [loadState, setLoadState] = useState<"loading" | "ready" | "error">("loading");
  const [projectQuery, setProjectQuery] = useState("");
  const [loadAttempt, setLoadAttempt] = useState(0);
  const [busy, setBusy] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const projectListRef = useRef<HTMLDivElement>(null);
  const deferredProjectQuery = useDeferredValue(projectQuery.trim().toLocaleLowerCase());
  const filteredProjects = useMemo(() => projects.filter((project) => (
    `${project.displayName}\n${project.canonicalPath}`
      .toLocaleLowerCase()
      .includes(deferredProjectQuery)
  )), [deferredProjectQuery, projects]);
  const projectVirtualizer = useVirtualizer({
    count: filteredProjects.length,
    getScrollElement: () => projectListRef.current,
    estimateSize: () => 52,
    overscan: 8,
    initialRect: { width: 520, height: 260 },
  });

  useEffect(() => {
    if (!open) return;
    let active = true;
    setProjects([]);
    setMemberIds(new Set());
    setProjectQuery("");
    setLoadState("loading");
    Promise.all([
      api.listProjects({ includeArchived: true, limit: 500 }),
      editingId ? api.collectionMemberIds(editingId) : Promise.resolve([]),
      editingId
        ? api.listProjects({ includeArchived: true, collectionId: editingId })
        : Promise.resolve([]),
    ])
      .then(([nextProjects, nextMembers, memberProjects]) => {
        if (!active) return;
        setProjects([...new Map(
          [...memberProjects, ...nextProjects].map((project) => [project.id, project]),
        ).values()]);
        setMemberIds(new Set(nextMembers));
        setLoadState("ready");
      })
      .catch((error) => {
        if (!active) return;
        setLoadState("error");
        notify("error", t("collectionLoadFailed"), String(error));
      });
    return () => {
      active = false;
    };
  }, [editingId, loadAttempt, notify, open, t]);

  useEffect(() => {
    const search = projectQuery.trim();
    if (!open || loadState !== "ready" || !search) return;
    let active = true;
    const timer = window.setTimeout(() => {
      void api.listProjects({ includeArchived: true, search, limit: 200 }).then((matches) => {
        if (!active) return;
        setProjects((current) => [...new Map(
          [...current, ...matches].map((project) => [project.id, project]),
        ).values()]);
      }).catch(() => undefined);
    }, 200);
    return () => {
      active = false;
      window.clearTimeout(timer);
    };
  }, [loadState, open, projectQuery]);

  function beginCreate() {
    setEditingId(undefined);
    setName("");
    setDescription("");
    setOpen(true);
  }

  function beginEdit() {
    if (!selectedId) return;
    const collection = collections.find((item) => item.id === selectedId);
    setEditingId(selectedId);
    setName(collection?.name ?? "");
    setDescription(collection?.description ?? "");
    setOpen(true);
  }

  async function save() {
    if (!name.trim() || busy || loadState !== "ready") return;
    setBusy(true);
    try {
      const collection = await api.saveCollection(editingId, {
        name: name.trim(),
        description: description.trim() || null,
      }, [...memberIds]);
      setOpen(false);
      await onChanged(collection.id);
      notify("success", t("collectionSaved"), collection.name);
    } catch (error) {
      notify("error", t("collectionSaveFailed"), String(error));
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    if (!editingId || busy) return;
    setBusy(true);
    try {
      await api.deleteCollection(editingId);
      setConfirmDelete(false);
      setOpen(false);
      await onChanged(undefined);
      notify("success", t("collectionDeleted"));
    } catch (error) {
      notify("error", t("collectionSaveFailed"), String(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <div className="collection-controls" role="group" aria-label={t("collections")}>
        <Menu.Root>
          <Menu.Trigger render={<button type="button" className={`collection-trigger${selectedCollection ? " is-active" : ""}`} aria-label={selectedCollection ? `${t("collections")}: ${selectedCollection.name}` : t("collections")} title={selectedCollection?.name ?? t("collections")}><SquaresFour weight={selectedCollection ? "fill" : "regular"} aria-hidden="true" /></button>} />
          <Menu.Portal>
            <Menu.Positioner sideOffset={6} className="menu-positioner">
              <Menu.Popup className="menu-popup collection-menu">
                <div className="collection-menu-heading">{t("collections")}</div>
                <Menu.Item className="menu-item collection-menu-item" onClick={() => onSelect(undefined)}>
                  <span className="menu-item-icon" aria-hidden="true"><SquaresFour /></span>
                  <span>{t("allCollections")}</span>
                  {!selectedId && <Check className="collection-menu-check" weight="bold" aria-hidden="true" />}
                </Menu.Item>
                {collections.map((collection) => (
                  <Menu.Item key={collection.id} className="menu-item collection-menu-item" onClick={() => onSelect(collection.id)}>
                    <span className="collection-menu-mark" aria-hidden="true" />
                    <span>{collection.name}</span>
                    <small>{collection.projectCount}</small>
                    {selectedId === collection.id && <Check className="collection-menu-check" weight="bold" aria-hidden="true" />}
                  </Menu.Item>
                ))}
                <div className="collection-menu-rule" aria-hidden="true" />
                <Menu.Item className="menu-item collection-menu-item" onClick={beginCreate}>
                  <span className="menu-item-icon" aria-hidden="true"><Plus /></span>
                  <span>{t("newCollection")}</span>
                </Menu.Item>
                {selectedId && <Menu.Item className="menu-item collection-menu-item" onClick={beginEdit}>
                  <span className="menu-item-icon" aria-hidden="true"><PencilSimple /></span>
                  <span>{t("editCollection")}</span>
                </Menu.Item>}
              </Menu.Popup>
            </Menu.Positioner>
          </Menu.Portal>
        </Menu.Root>
      </div>
      <Dialog.Root open={open} onOpenChange={(next) => !busy && setOpen(next)}>
        <Dialog.Portal>
          <Dialog.Backdrop className="dialog-backdrop" />
          <Dialog.Popup className="dialog-popup collection-dialog">
            <Dialog.Title className="dialog-title">
              {editingId ? t("editCollection") : t("newCollection")}
            </Dialog.Title>
            <Dialog.Description className="dialog-description">
              {t("deleteCollectionHint")}
            </Dialog.Description>
            <label className="field-group">
              <span>{t("collectionName")}</span>
              <input
                autoFocus
                value={name}
                maxLength={80}
                onChange={(event) => setName(event.target.value)}
              />
            </label>
            <label className="field-group">
              <span>{t("collectionDescription")}</span>
              <textarea
                value={description}
                maxLength={500}
                onChange={(event) => setDescription(event.target.value)}
              />
            </label>
            <fieldset className="collection-projects">
              <legend>{t("collectionProjects")}</legend>
              <input value={projectQuery} onChange={(event) => setProjectQuery(event.target.value)} placeholder={t("search")} disabled={loadState !== "ready"} />
              {loadState === "loading" && <p className="muted-copy">{t("collectionLoading")}</p>}
              {loadState === "error" && <div className="error-copy"><p>{t("collectionLoadFailed")}</p><Button type="button" size="sm" variant="quiet" onClick={() => setLoadAttempt((attempt) => attempt + 1)}>{t("retry")}</Button></div>}
              {loadState === "ready" && filteredProjects.length <= 100 && filteredProjects.map((project) => (
                <label key={project.id}>
                  <input
                    type="checkbox"
                    checked={memberIds.has(project.id)}
                    onChange={(event) => setMemberIds((current) => {
                      const next = new Set(current);
                      if (event.target.checked) next.add(project.id);
                      else next.delete(project.id);
                      return next;
                    })}
                  />
                  <span><strong>{project.displayName}</strong><code>{project.canonicalPath}</code></span>
                </label>
              ))}
              {loadState === "ready" && filteredProjects.length > 100 && <div ref={projectListRef} className="collection-project-list">
                <div className="collection-project-list-inner" style={{ height: projectVirtualizer.getTotalSize() }}>
                {projectVirtualizer.getVirtualItems().map((row) => {
                  const project = filteredProjects[row.index]!;
                  return <label key={project.id} style={{ transform: `translateY(${row.start}px)` }}>
                  <input
                    type="checkbox"
                    checked={memberIds.has(project.id)}
                    onChange={(event) =>
                      setMemberIds((current) => {
                        const next = new Set(current);
                        if (event.target.checked) next.add(project.id);
                        else next.delete(project.id);
                        return next;
                      })
                    }
                  />
                  <span>
                    <strong>{project.displayName}</strong>
                    <code>{project.canonicalPath}</code>
                  </span>
                </label>;
                })}
                </div>
              </div>}
            </fieldset>
            <div className="dialog-actions collection-dialog-actions">
              {editingId && (
                <Button variant="danger" onClick={() => setConfirmDelete(true)}>
                  <Trash />
                  {t("deleteCollection")}
                </Button>
              )}
              <span />
              <Dialog.Close
                render={<Button disabled={busy}>{t("cancel")}</Button>}
              />
              <Button
                variant="primary"
                loading={busy}
                disabled={!name.trim() || loadState !== "ready"}
                onClick={() => void save()}
              >
                {t("save")}
              </Button>
            </div>
          </Dialog.Popup>
        </Dialog.Portal>
      </Dialog.Root>
      <ConfirmDialog
        open={confirmDelete}
        title={t("deleteCollection")}
        body={t("deleteCollectionHint")}
        confirmLabel={t("deleteCollection")}
        cancelLabel={t("cancel")}
        busy={busy}
        onOpenChange={setConfirmDelete}
        onConfirm={() => void remove()}
      />
    </>
  );
}
