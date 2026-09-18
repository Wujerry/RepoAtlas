import { Dialog } from "@base-ui/react/dialog";
import { Menu } from "@base-ui/react/menu";
import { Check, MagnifyingGlass, PencilSimple, Plus, SpinnerGap, SquaresFour, Trash, X } from "@phosphor-icons/react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { MessageKey } from "../i18n";
import { api } from "../lib/api";
import type { ProjectCollection, ProjectSummary, ToastTone } from "../types";
import { Button } from "./ui/button";
import { SelectionList } from "./ui/selection-list";
import { ConfirmDialog } from "./ui/confirm";

const projectOrder = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });

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
  const nameRef = useRef<HTMLInputElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const feedback = useRef({ notify, t });
  feedback.current = { notify, t };
  const [selectedOnly, setSelectedOnly] = useState(false);
  const [searchState, setSearchState] = useState<"idle" | "loading" | "error">("idle");
  const [searchAttempt, setSearchAttempt] = useState(0);
  const [saveError, setSaveError] = useState<string>();
  const saving = useRef(false);
  const filteredProjects = useMemo(() => projects.filter((project) => (
    (!selectedOnly || memberIds.has(project.id)) &&
    `${project.displayName}\n${project.canonicalPath}`.toLocaleLowerCase().includes(projectQuery.trim().toLocaleLowerCase())
  )).sort((a, b) => projectOrder.compare(a.displayName, b.displayName) || projectOrder.compare(a.canonicalPath, b.canonicalPath) || a.id.localeCompare(b.id)), [projectQuery, projects, memberIds, selectedOnly]);
  const pickerItems = useMemo(() => filteredProjects.map((project) => ({ id: project.id, name: project.displayName, detail: project.canonicalPath })), [filteredProjects]);
  function toggleMember(id: string) {
    setMemberIds((current) => { const next = new Set(current); if (next.has(id)) next.delete(id); else next.add(id); return next; });
  }

  useEffect(() => {
    if (!open) return;
    let active = true;
    setProjects([]);
    setMemberIds(new Set());
    setProjectQuery("");
    setSelectedOnly(false);
    setSearchState("idle");
    setSaveError(undefined);
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
        feedback.current.notify("error", feedback.current.t("collectionLoadFailed"), String(error));
      });
    return () => {
      active = false;
    };
  }, [editingId, loadAttempt, open]);

  useEffect(() => {
    const search = projectQuery.trim();
    if (!open || loadState !== "ready" || !search) { setSearchState("idle"); return; }
    setSearchState("loading");
    let active = true;
    const timer = window.setTimeout(() => {
      void api.listProjects({ includeArchived: true, search, limit: 200 }).then((matches) => {
        if (!active) return;
        setProjects((current) => [...new Map(
          [...current, ...matches].map((project) => [project.id, project]),
        ).values()]);
        setSearchState("idle");
      }).catch(() => { if (active) setSearchState("error"); });
    }, 200);
    return () => {
      active = false;
      window.clearTimeout(timer);
    };
  }, [loadState, open, projectQuery, searchAttempt]);

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
    if (!name.trim() || saving.current || loadState !== "ready") return;
    saving.current = true;
    setSaveError(undefined);
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
      setSaveError(String(error));
      notify("error", t("collectionSaveFailed"), String(error));
    } finally {
      saving.current = false;
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
          <Menu.Trigger render={<button ref={triggerRef} type="button" className={`collection-trigger${selectedCollection ? " is-active" : ""}`} aria-label={selectedCollection ? `${t("collections")}: ${selectedCollection.name}` : t("collections")} title={selectedCollection?.name ?? t("collections")}><SquaresFour weight={selectedCollection ? "fill" : "regular"} aria-hidden="true" /></button>} />
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
      <Dialog.Root open={open} disablePointerDismissal onOpenChange={(next) => !busy && setOpen(next)}>
        <Dialog.Portal>
          <Dialog.Backdrop className="dialog-backdrop" />
          <Dialog.Popup className="dialog-popup collection-dialog" initialFocus={nameRef} finalFocus={triggerRef}>
            <form className="collection-form" onSubmit={(event) => { event.preventDefault(); void save(); }} onKeyDown={(event) => { if (event.key === "Enter" && event.nativeEvent.isComposing) event.preventDefault(); }}>
              <header className="collection-editor-header">
                <div><Dialog.Title className="dialog-title">{editingId ? t("editCollection") : t("newCollection")}</Dialog.Title><Dialog.Description className="dialog-description">{t("collectionEditorHint")}</Dialog.Description></div>
                <Dialog.Close render={<Button type="button" size="icon" variant="quiet" disabled={busy} aria-label={t("close")}><X /></Button>} />
              </header>
              <div className="collection-editor-body">
                <section className="collection-details" aria-label={t("collectionDetails")}>
                  <h3>{t("collectionDetails")}</h3>
                  <label className="field-group"><span>{t("collectionName")}</span><input ref={nameRef} required value={name} maxLength={80} placeholder={t("collectionNamePlaceholder")} disabled={busy} onChange={(event) => setName(event.target.value)} /></label>
                  <label className="field-group"><span>{t("collectionDescription")} <small>{t("optionalField")}</small></span><textarea value={description} maxLength={500} placeholder={t("collectionDescriptionPlaceholder")} disabled={busy} onChange={(event) => setDescription(event.target.value)} /></label>
                  <div className="collection-selection-summary"><SquaresFour aria-hidden="true" /><strong>{memberIds.size}</strong><span>{t("collectionSelectedProjects")}</span></div>
                  <p className="collection-details-hint">{t("collectionEmptyAllowed")}</p>
                </section>
                <section className="collection-picker" aria-label={t("collectionProjects")}>
                  <div className="collection-picker-heading"><h3>{t("collectionProjects")}</h3><span>{t("collectionKeyboardHint")}</span></div>
                  <div className="collection-search">
                    <MagnifyingGlass aria-hidden="true" />
                    <input ref={searchRef} type="search" value={projectQuery} aria-label={t("collectionSearch")} placeholder={t("collectionSearch")} disabled={loadState !== "ready" || busy} onChange={(event) => setProjectQuery(event.target.value)} />
                    {searchState === "loading" && <SpinnerGap className="button-spinner" aria-label={t("searchingProjects")} />}
                    {projectQuery && <Button type="button" size="icon" variant="quiet" aria-label={t("clearSearch")} disabled={busy} onClick={() => { setProjectQuery(""); searchRef.current?.focus(); }}><X /></Button>}
                  </div>
                  <div className="collection-picker-actions">
                    <div className="collection-view-switch" role="group" aria-label={t("collectionProjectView")}><button type="button" aria-pressed={!selectedOnly} disabled={busy} onClick={() => setSelectedOnly(false)}>{t("collectionAllProjects")}</button><button type="button" aria-pressed={selectedOnly} disabled={busy} onClick={() => setSelectedOnly(true)}>{t("collectionSelectedOnly")} <span>{memberIds.size}</span></button></div>
                    <Button type="button" variant="quiet" disabled={busy || loadState !== "ready" || !filteredProjects.length} onClick={() => setMemberIds((current) => { const next = new Set(current); for (const project of filteredProjects) { if (selectedOnly) next.delete(project.id); else next.add(project.id); } return next; })}>{t(selectedOnly ? "collectionRemoveResults" : "collectionSelectResults")}</Button>
                  </div>
                  {loadState === "loading" && <div className="collection-picker-empty" role="status"><SpinnerGap className="button-spinner" /><p>{t("collectionLoading")}</p></div>}
                  {loadState === "error" && <div className="collection-picker-empty" role="alert"><p>{t("collectionLoadFailed")}</p><Button type="button" onClick={() => setLoadAttempt((attempt) => attempt + 1)}>{t("retry")}</Button></div>}
                  {loadState === "ready" && <>
                    <SelectionList key={`${selectedOnly}:${projectQuery}`} items={pickerItems} selected={memberIds} onToggle={toggleMember} disabled={busy} label={t("collectionProjects")} hint={t("collectionKeyboardHint")} emptyMessage={searchState === "loading" ? t("searchingProjects") : t(selectedOnly ? "collectionNoSelectedResults" : "collectionNoResults")} />
                    <div className="collection-picker-status" role={searchState === "error" ? "alert" : "status"}>{searchState === "error" ? <><span>{t("collectionSearchFailed")}</span><Button type="button" variant="quiet" onClick={() => setSearchAttempt((value) => value + 1)}>{t("retry")}</Button></> : <span>{filteredProjects.length} {t("collectionMatchingProjects")} · {t("collectionSearchHint")}</span>}</div>
                  </>}
                </section>
              </div>
              {saveError && <p className="form-error collection-save-error" role="alert">{t("collectionSaveFailed")}: {saveError}</p>}
              <footer className="collection-editor-footer">
                {editingId && <Button type="button" variant="danger" disabled={busy} onClick={() => setConfirmDelete(true)}><Trash />{t("deleteCollection")}</Button>}
                <span className="collection-footer-spacer" />
                <Dialog.Close render={<Button type="button" disabled={busy}>{t("cancel")}</Button>} />
                <Button type="submit" variant="primary" loading={busy} disabled={!name.trim() || loadState !== "ready"}>{t(editingId ? "save" : "createCollection")}</Button>
              </footer>
            </form>
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
