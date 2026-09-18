import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { ComponentProps } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CollectionControls } from "../components/CollectionControls";
import { dictionaries, type MessageKey } from "../i18n";
import type { ProjectCollection, ProjectSummary } from "../types";

const apiMocks = vi.hoisted(() => ({
  listProjects: vi.fn(),
  collectionMemberIds: vi.fn(),
  saveCollection: vi.fn(),
  deleteCollection: vi.fn(),
}));

vi.mock("../lib/api", () => ({ api: apiMocks }));

const t = (key: MessageKey) => dictionaries.en[key];

const project: ProjectSummary = {
  id: "project-1",
  canonicalPath: "C:\\code\\atlas",
  displayName: "Atlas",
  detectedName: "Atlas",
  notes: null,
  description: null,
  vcsKind: "git",
  availability: "ready",
  archived: false,
  favorite: false,
  origin: "scan",
  scanRootId: "root-1",
  languages: ["TypeScript"],
  frameworks: ["React"],
  packageManagers: ["pnpm"],
  tags: [],
  sourceMtime: null,
  lastCommitAt: null,
  lastOpenedAt: null,
  updatedAt: "2026-09-02T00:00:00Z",
};

const collection: ProjectCollection = {
  id: "collection-1",
  name: "Daily work",
  description: null,
  projectCount: 1,
  createdAt: "2026-09-02T00:00:00Z",
  updatedAt: "2026-09-02T00:00:00Z",
};

function renderControls(overrides: Partial<ComponentProps<typeof CollectionControls>> = {}) {
  return render(<CollectionControls
    collections={[]}
    t={t}
    notify={vi.fn()}
    onSelect={vi.fn()}
    onChanged={vi.fn(async () => undefined)}
    {...overrides}
  />);
}

async function chooseCollectionAction(name: string, triggerName = t("collections")) {
  fireEvent.click(screen.getByRole("button", { name: triggerName }));
  fireEvent.click(await screen.findByRole("menuitem", { name }));
}

describe("CollectionControls", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    HTMLElement.prototype.scrollTo = vi.fn();
    apiMocks.listProjects.mockResolvedValue([project]);
    apiMocks.collectionMemberIds.mockResolvedValue([]);
    apiMocks.saveCollection.mockResolvedValue(collection);
    apiMocks.deleteCollection.mockResolvedValue(undefined);
  });

  it("saves collection metadata and members atomically", async () => {
    const onChanged = vi.fn(async () => undefined);
    renderControls({ onChanged });

    await chooseCollectionAction(t("newCollection"));
    fireEvent.change(await screen.findByLabelText(t("collectionName")), { target: { value: "Daily work" } });
    fireEvent.click(await screen.findByRole("checkbox", { name: /Atlas/ }));
    fireEvent.click(screen.getByRole("button", { name: t("createCollection") }));

    await waitFor(() => expect(apiMocks.saveCollection).toHaveBeenCalledWith(undefined, { name: "Daily work", description: null }, [project.id]));
    expect(onChanged).toHaveBeenCalledWith(collection.id);
  });

  it("does not allow saving partial membership after loading fails", async () => {
    apiMocks.listProjects.mockRejectedValueOnce(new Error("database unavailable"));
    renderControls({ collections: [collection], selectedId: collection.id });

    await chooseCollectionAction(t("editCollection"), `${t("collections")}: ${collection.name}`);
    expect(await screen.findByText(t("collectionLoadFailed"))).toBeInTheDocument();
    expect(screen.getByRole("button", { name: t("save") })).toBeDisabled();
    expect(apiMocks.saveCollection).not.toHaveBeenCalled();
  });

  it("keeps unsaved edits when the collection list refreshes", async () => {
    const view = renderControls({ collections: [collection], selectedId: collection.id });
    await chooseCollectionAction(t("editCollection"), `${t("collections")}: ${collection.name}`);
    const name = await screen.findByLabelText(t("collectionName"));
    fireEvent.change(name, { target: { value: "In progress" } });

    view.rerender(<CollectionControls
      collections={[{ ...collection, name: "Server refresh" }]}
      selectedId={collection.id}
      t={t}
      notify={vi.fn()}
      onSelect={vi.fn()}
      onChanged={vi.fn(async () => undefined)}
    />);

    expect(screen.getByLabelText(t("collectionName"))).toHaveValue("In progress");
  });

  it("returns to all projects after deleting the active collection", async () => {
    const onChanged = vi.fn(async () => undefined);
    renderControls({ collections: [collection], selectedId: collection.id, onChanged });
    await chooseCollectionAction(t("editCollection"), `${t("collections")}: ${collection.name}`);
    fireEvent.click(await screen.findByRole("button", { name: t("deleteCollection") }));
    const confirmation = await screen.findByRole("alertdialog");
    fireEvent.click(within(confirmation).getByRole("button", { name: t("deleteCollection") }));

    await waitFor(() => expect(apiMocks.deleteCollection).toHaveBeenCalledWith(collection.id));
    expect(onChanged).toHaveBeenCalledWith(undefined);
  });

  it("uses a compact menu to switch between all projects and a collection", async () => {
    const onSelect = vi.fn();
    renderControls({ collections: [collection], onSelect });

    expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: t("collections") }));
    fireEvent.click(await screen.findByRole("menuitem", { name: /Daily work/ }));
    expect(onSelect).toHaveBeenCalledWith(collection.id);
  });
  it("focuses the name on open and restores focus to the collection trigger", async () => {
    renderControls();
    await chooseCollectionAction(t("newCollection"));
    await waitFor(() => expect(screen.getByLabelText(t("collectionName"))).toHaveFocus());
    fireEvent.click(screen.getByRole("button", { name: t("cancel") }));
    await waitFor(() => expect(screen.getByRole("button", { name: t("collections") })).toHaveFocus());
  });

  it("preserves selected projects across searches and parent refreshes", async () => {
    const other = { ...project, id: "other", displayName: "Other", canonicalPath: "C:/other" };
    apiMocks.listProjects.mockResolvedValue([project, other]);
    const view = renderControls();
    await chooseCollectionAction(t("newCollection"));
    fireEvent.click(await screen.findByRole("checkbox", { name: /Atlas/ }));
    view.rerender(<CollectionControls collections={[]} t={t} notify={vi.fn()} onSelect={vi.fn()} onChanged={vi.fn(async () => undefined)} />);
    expect(screen.getByRole("checkbox", { name: /Atlas/ })).toBeChecked();
    expect(apiMocks.listProjects).toHaveBeenCalledTimes(1);
    fireEvent.change(screen.getByRole("searchbox"), { target: { value: "Other" } });
    fireEvent.click(screen.getByRole("button", { name: t("collectionSelectResults") }));
    fireEvent.click(screen.getByRole("button", { name: t("clearSearch") }));
    expect(screen.getByRole("searchbox")).toHaveFocus();
    expect(screen.getByRole("checkbox", { name: /Atlas/ })).toBeChecked();
    expect(screen.getByRole("checkbox", { name: /Other/ })).toBeChecked();
  });

  it("supports an empty collection and keeps drafts after a failed save", async () => {
    apiMocks.saveCollection.mockRejectedValueOnce(new Error("disk busy")).mockResolvedValueOnce(collection);
    renderControls();
    await chooseCollectionAction(t("newCollection"));
    await screen.findByRole("checkbox", { name: /Atlas/ });
    fireEvent.change(screen.getByLabelText(t("collectionName")), { target: { value: "Empty collection" } });
    fireEvent.click(screen.getByRole("button", { name: t("createCollection") }));
    expect(await screen.findByRole("alert")).toHaveTextContent("disk busy");
    expect(screen.getByLabelText(t("collectionName"))).toHaveValue("Empty collection");
    fireEvent.click(screen.getByRole("button", { name: t("createCollection") }));
    await waitFor(() => expect(apiMocks.saveCollection).toHaveBeenLastCalledWith(undefined, { name: "Empty collection", description: null }, []));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("can reach a virtualized project using End and return using Home", async () => {
    apiMocks.listProjects.mockResolvedValue(Array.from({ length: 150 }, (_, index) => ({ ...project, id: `p${index}`, displayName: `Project ${index}` })));
    renderControls();
    await chooseCollectionAction(t("newCollection"));
    const first = await screen.findByRole("checkbox", { name: /^Project 0 / });
    act(() => first.focus());
    fireEvent.keyDown(first, { key: "End" });
    const last = await screen.findByRole("checkbox", { name: /^Project 149 / });
    expect(last).toHaveFocus();
    fireEvent.click(last);
    expect(last).toBeChecked();
    fireEvent.keyDown(last, { key: "Home" });
    expect(screen.getByRole("checkbox", { name: /^Project 0 / })).toHaveFocus();
  });

  it("removes only the visible selected results while retaining hidden members", async () => {
    apiMocks.listProjects.mockResolvedValue([project, { ...project, id: "other", displayName: "Other" }]);
    apiMocks.collectionMemberIds.mockResolvedValue([project.id, "other"]);
    renderControls({ collections: [collection], selectedId: collection.id });
    await chooseCollectionAction(t("editCollection"), `${t("collections")}: ${collection.name}`);
    await screen.findByRole("checkbox", { name: /Atlas/ });
    fireEvent.click(screen.getByRole("button", { name: /Selected 2/ }));
    fireEvent.change(screen.getByRole("searchbox"), { target: { value: "Other" } });
    fireEvent.click(screen.getByRole("button", { name: t("collectionRemoveResults") }));
    fireEvent.click(screen.getByRole("button", { name: t("save") }));
    await waitFor(() => expect(apiMocks.saveCollection).toHaveBeenCalledWith(collection.id, { name: collection.name, description: null }, [project.id]));
  });

});
