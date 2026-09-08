import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { dictionaries, type MessageKey } from "../i18n";
import type { ProjectSummary } from "../types";
import { beforeEach, describe, expect, it, vi } from "vitest";

const openMock = vi.hoisted(() => vi.fn());
const setProjectIconMock = vi.hoisted(() => vi.fn());
const clearProjectIconMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: openMock }));

vi.mock("../lib/api", () => ({
  api: {
    readProjectIcons: vi.fn(async (ids: string[]) => ids.map((id) => ({
      projectId: id,
      kind: "language",
      source: "typescript",
      mimeType: null,
      dataUrl: null,
    }))),
    setProjectIcon: setProjectIconMock,
    clearProjectIcon: clearProjectIconMock,
  },
}));
import { ProjectList } from "../components/ProjectList";

const t = (key: MessageKey) => dictionaries.en[key];

function project(id: string, canonicalPath: string): ProjectSummary {
  return {
    id,
    canonicalPath,
    displayName: id,
    detectedName: id,
    description: null,
    notes: null,
    vcsKind: "git",
    availability: "ready",
    archived: false,
    favorite: id === "atlas",
    origin: "scan",
    scanRootId: null,
    languages: ["TypeScript"],
    frameworks: ["React"],
    packageManagers: ["pnpm"],
    tags: [],
    sourceMtime: null,
    lastCommitAt: null,
    lastOpenedAt: null,
    updatedAt: "2026-08-21T00:00:00Z",
  };
}

function renderList(overrides: Partial<React.ComponentProps<typeof ProjectList>> = {}) {
  const projects = [project("atlas", "C:\\code\\atlas"), project("portal", "C:\\code\\portal")];
  const props = {
    projects,
    allProjects: projects,
    selectedId: "atlas",
    onSelect: vi.fn(),
    query: "",
    onQuery: vi.fn(),
    filters: { language: "", tag: "" },
    onFilters: vi.fn(),
    scanning: false,
    progress: null,
    t,
    empty: "Empty",
    onAddRoot: vi.fn(),
    onRegister: vi.fn(),
    onScan: vi.fn(),
    onCancelScan: vi.fn(),
    ...overrides,
  };
  return { ...render(<ProjectList {...props} />), props, projects };
}

function rect(height: number): DOMRect {
  return { bottom: height, height, left: 0, right: 360, top: 0, width: 360, x: 0, y: 0, toJSON: () => ({}) } as DOMRect;
}

beforeEach(() => {
  openMock.mockReset();
  setProjectIconMock.mockReset();
  clearProjectIconMock.mockReset();
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockImplementation(function getOffsetHeight(this: HTMLElement) {
    const element = this as HTMLElement;
    return element.classList.contains("project-scroll") ? 640 : element.classList.contains("virtual-row") ? 84 : 0;
  });
  vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockImplementation(function getOffsetWidth(this: HTMLElement) {
    return (this as HTMLElement).classList.contains("project-scroll") ? 360 : 0;
  });
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function getBoundingClientRect(this: HTMLElement) {
    const element = this as HTMLElement;
    if (element.classList.contains("project-scroll")) return rect(640);
    if (element.classList.contains("virtual-row")) return rect(84);
    return rect(0);
  });
});

describe("ProjectList tree", () => {
  it("renders an expanded path tree and moves active selection with arrows", async () => {
    renderList();
    const tree = screen.getByRole("tree");
    await waitFor(() => expect(screen.getByRole("treeitem", { name: "atlas" })).toBeInTheDocument());
    expect(screen.getByRole("treeitem", { name: /code/ })).toHaveAttribute("aria-expanded", "true");

    fireEvent.keyDown(tree, { key: "ArrowDown" });
    fireEvent.keyDown(tree, { key: "ArrowDown" });
    expect(tree).toHaveAttribute("aria-activedescendant", "project:atlas");
  });

  it("expands groups that only appear after the first async project load", () => {
    const view = renderList({ projects: [], allProjects: [] });
    expect(screen.queryByRole("treeitem")).not.toBeInTheDocument();

    view.rerender(<ProjectList {...view.props} projects={view.projects} allProjects={view.projects} />);

    expect(screen.getByRole("treeitem", { name: /code/ })).toHaveAttribute("aria-expanded", "true");
  });

  it("keeps project icons decorative and out of the accessible name", async () => {
    renderList();
    const option = await screen.findByRole("treeitem", { name: "atlas" });
    expect(option).toHaveAccessibleName("atlas");
    expect(option.querySelector(".project-identity-mark")).toHaveAttribute("aria-hidden", "true");
  });

  it("keeps row triggers out of the tab sequence and returns focus to the tree", async () => {
    renderList();
    const tree = screen.getByRole("tree");
    const option = await screen.findByRole("treeitem", { name: "atlas" });
    const rowButton = within(option).getByRole("button", { name: "atlas" });

    expect(rowButton).toHaveAttribute("tabindex", "-1");
    fireEvent.click(rowButton);
    expect(tree).toHaveFocus();
  });

  it("opens the active project context menu from Shift+F10 without hover actions", async () => {
    renderList({ onRename: vi.fn() });
    const tree = screen.getByRole("tree");
    const option = await screen.findByRole("treeitem", { name: "atlas" });
    fireEvent.click(within(option).getByRole("button", { name: "atlas" }));

    expect(document.querySelector(".project-actions")).not.toBeInTheDocument();
    fireEvent.keyDown(tree, { key: "F10", shiftKey: true });

    expect(await screen.findByRole("menuitem", { name: t("editName") })).toBeInTheDocument();
  });

  it("gives every project context action a unique icon", async () => {
    renderList({ onReveal: vi.fn(), onRelocate: vi.fn() });
    const option = await screen.findByRole("treeitem", { name: "atlas" });
    fireEvent.contextMenu(within(option).getByRole("button", { name: "atlas" }));

    const items = await screen.findAllByRole("menuitem");
    const icons = items.map((item) => item.querySelector(".menu-item-icon svg")?.innerHTML ?? "");
    expect(icons.every(Boolean)).toBe(true);
    expect(new Set(icons).size).toBe(icons.length);
  });

  it("changes a project icon and restores automatic detection from the context menu", async () => {
    openMock.mockResolvedValue("C:\\icons\\atlas.png");
    setProjectIconMock.mockResolvedValue({ projectId: "atlas", kind: "override", source: "atlas.png", mimeType: "image/png", dataUrl: "data:image/png;base64,AA==" });
    clearProjectIconMock.mockResolvedValue({ projectId: "atlas", kind: "language", source: "typescript", mimeType: null, dataUrl: null });
    renderList();
    const option = await screen.findByRole("treeitem", { name: "atlas" });
    fireEvent.contextMenu(within(option).getByRole("button", { name: "atlas" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: t("changeProjectIcon") }));
    await waitFor(() => expect(setProjectIconMock).toHaveBeenCalledWith("atlas", "C:\\icons\\atlas.png"));
    expect(option.querySelector(".project-identity-mark img")).toHaveAttribute("src", "data:image/png;base64,AA==");

    fireEvent.contextMenu(within(option).getByRole("button", { name: "atlas" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: t("resetProjectIcon") }));
    await waitFor(() => expect(clearProjectIconMock).toHaveBeenCalledWith("atlas"));
    expect(option.querySelector(".project-identity-mark img")).not.toBeInTheDocument();
  });

  it("keeps project identity aligned and edits the name inside its row", async () => {
    const onSelect = vi.fn();
    renderList({ onSelect });
    const option = await screen.findByRole("treeitem", { name: "atlas" });
    expect(option.querySelector(".project-card-layout")).toBeInTheDocument();
    expect(option.querySelector(".project-card-copy .project-name")).toHaveTextContent("atlas");

    const virtualRow = option.closest(".virtual-row") as HTMLElement;
    fireEvent.contextMenu(within(option).getByRole("button", { name: "atlas" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: t("editName") }));
    const input = screen.getByRole("textbox", { name: t("editName") });
    expect(virtualRow).toContainElement(input);
    expect(document.querySelector(".inline-editor")).not.toBeInTheDocument();
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("keeps the card editor open when renaming fails", async () => {
    const onRename = vi.fn(async () => { throw new Error("save failed"); });
    renderList({ onRename });
    const option = await screen.findByRole("treeitem", { name: "atlas" });
    fireEvent.contextMenu(within(option).getByRole("button", { name: "atlas" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: t("editName") }));
    const input = screen.getByRole("textbox", { name: t("editName") });
    fireEvent.change(input, { target: { value: "Atlas renamed" } });
    fireEvent.submit(input.closest("form") as HTMLFormElement);

    await waitFor(() => expect(onRename).toHaveBeenCalledWith("atlas", "Atlas renamed"));
    expect(screen.getByRole("textbox", { name: t("editName") })).toHaveValue("Atlas renamed");
  });

  it("collapses a group with Enter and selects a project with Enter", async () => {
    const onSelect = vi.fn();
    renderList({ onSelect });
    const tree = screen.getByRole("tree");
    await waitFor(() => expect(screen.getByRole("treeitem", { name: "portal" })).toBeInTheDocument());

    fireEvent.keyDown(tree, { key: "Home" });
    fireEvent.keyDown(tree, { key: "Enter" });
    expect(screen.queryByRole("treeitem", { name: "atlas" })).not.toBeInTheDocument();

    fireEvent.keyDown(tree, { key: "Enter" });
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("offers scope filtering when controlled by the parent", () => {
    const onScope = vi.fn();
    renderList({ scope: "favorites", onScope });
    expect(screen.getByRole("tab", { name: t("favorites") })).toHaveAttribute("aria-selected", "true");
    fireEvent.click(screen.getByRole("tab", { name: t("archived") }));
    expect(onScope).toHaveBeenCalledWith("archived");
  });

  it("adds and removes projects through collection-aware context actions", async () => {
    const onAddToCollection = vi.fn();
    const onRemoveFromCollection = vi.fn();
    const collections = [{ id: "daily", name: "Daily work", description: null, projectCount: 0, createdAt: "2026-09-02T00:00:00Z", updatedAt: "2026-09-02T00:00:00Z" }];
    const view = renderList({ collections, onAddToCollection });
    let option = await screen.findByRole("treeitem", { name: "atlas" });
    fireEvent.contextMenu(within(option).getByRole("button", { name: "atlas" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: `${t("addToCollection")} · Daily work` }));
    expect(onAddToCollection).toHaveBeenCalledWith("atlas", "daily");

    view.rerender(<ProjectList
      projects={[project("atlas", "C:\\code\\atlas"), project("portal", "C:\\code\\portal")]}
      allProjects={[project("atlas", "C:\\code\\atlas"), project("portal", "C:\\code\\portal")]}
      selectedId="atlas"
      onSelect={vi.fn()}
      query=""
      onQuery={vi.fn()}
      filters={{ language: "", tag: "" }}
      onFilters={vi.fn()}
      scanning={false}
      progress={null}
      t={t}
      empty="Empty"
      onAddRoot={vi.fn()}
      onRegister={vi.fn()}
      onScan={vi.fn()}
      onCancelScan={vi.fn()}
      collections={collections}
      selectedCollectionId="daily"
      onRemoveFromCollection={onRemoveFromCollection}
    />);
    option = await screen.findByRole("treeitem", { name: "atlas" });
    fireEvent.contextMenu(within(option).getByRole("button", { name: "atlas" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: t("removeFromCollection") }));
    expect(onRemoveFromCollection).toHaveBeenCalledWith("atlas", "daily");
  });
  it("uses listbox options in flat mode without treeitem semantics", async () => {
    renderList();
    await waitFor(() => expect(screen.getByRole("treeitem", { name: "atlas" })).toBeInTheDocument());

    fireEvent.click(screen.getByRole("button", { name: t("flatView") }));

    const listbox = await screen.findByRole("listbox", { name: t("projects") });
    expect(listbox).toBeInTheDocument();
    expect(screen.queryByRole("treeitem")).not.toBeInTheDocument();
    expect(within(listbox).getAllByRole("option")).toHaveLength(2);
    expect(within(listbox).getByRole("option", { name: "atlas" })).not.toHaveAttribute("aria-level");
    expect(within(listbox).getByRole("option", { name: "atlas" })).toHaveAttribute("aria-selected", "true");
  });
  it("keeps a scope-specific empty state separate from filter misses", () => {
    renderList({ projects: [], scope: "favorites", empty: t("noFavorites") });

    expect(screen.queryByText(t("noMatches"))).not.toBeInTheDocument();
    expect(screen.getByText(t("emptyTitle"))).toBeInTheDocument();
    expect(screen.getByText(t("noFavorites"))).toBeInTheDocument();
  });
  it("renders only the virtualizer range for a large project set", async () => {
    const projects = Array.from({ length: 40 }, (_, index) => project(`project-${index}`, `C:\\code\\project-${index}`));
    const { container } = renderList({ projects, allProjects: projects, selectedId: projects[0].id });

    await waitFor(() => expect(container.querySelectorAll(".virtual-row").length).toBeGreaterThan(0));
    expect(container.querySelectorAll(".virtual-row").length).toBeLessThan(projects.length);
  });
  it("groups common project actions under one add menu and keeps the scan status slot", async () => {
    const onRegister = vi.fn();
    const onAddRoot = vi.fn();
    const onScan = vi.fn();
    const { container, rerender } = renderList({ onRegister, onAddRoot, onScan });
    await waitFor(() => expect(screen.getByRole("treeitem", { name: "atlas" })).toBeInTheDocument());
    expect(container.querySelector(".scan-status")).toBeTruthy();
    expect(container.querySelector(".list-header-actions")?.textContent).toBe("");
    fireEvent.click(screen.getByRole("button", { name: t("projectActions") }));
    fireEvent.click(await screen.findByRole("menuitem", { name: t("register") }));
    expect(onRegister).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: t("projectActions") }));
    fireEvent.click(await screen.findByRole("menuitem", { name: t("addRoot") }));
    expect(onAddRoot).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: t("projectActions") }));
    fireEvent.click(await screen.findByRole("menuitem", { name: t("scanAll") }));
    expect(onScan).toHaveBeenCalledOnce();
    expect(container.querySelector(".list-footer")?.textContent).not.toContain(t("addRootShort"));
    expect(container.querySelector(".list-footer")?.textContent).not.toContain(t("discoverProjects"));

    rerender(
      <ProjectList
        projects={[project("atlas", "C:\\code\\atlas"), project("portal", "C:\\code\\portal")]}
        allProjects={[project("atlas", "C:\\code\\atlas"), project("portal", "C:\\code\\portal")]}
        selectedId="atlas"
        onSelect={vi.fn()}
        query=""
        onQuery={vi.fn()}
        filters={{ language: "", tag: "" }}
        onFilters={vi.fn()}
        scanning
        progress={{ scanId: "scan-1", rootPath: "C:\\code", phase: "walk", visited: 4, discovered: 2, currentPath: "C:\\code\\atlas", message: null }}
        t={t}
        empty="Empty"
        onAddRoot={vi.fn()}
        onRegister={vi.fn()}
        onScan={vi.fn()}
        onCancelScan={vi.fn()}
      />,
    );
    expect(screen.getByRole("status")).toHaveTextContent(/Scanning|扫描/);
  });
  it("opens a context menu to remove a project record", async () => {
    const onRemoveProject = vi.fn();
    renderList({ onRemoveProject });
    await waitFor(() => expect(screen.getByRole("treeitem", { name: "atlas" })).toBeInTheDocument());
    fireEvent.contextMenu(screen.getByRole("button", { name: "atlas" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: t("removeRecord") }));
    expect(screen.getByRole("alertdialog")).toHaveTextContent(t("removeRecordHint"));
    fireEvent.click(screen.getByRole("button", { name: t("removeRecord") }));
    await waitFor(() => expect(onRemoveProject).toHaveBeenCalledWith("atlas"));
  });
  it("opens a context menu to remove folder records", async () => {
    const onRemoveFolder = vi.fn();
    renderList({
      onRemoveFolder,
      scanRoots: [{ id: "root-code", path: "C:\\code", createdAt: "2026-08-21T00:00:00Z", lastScannedAt: null }],
    });
    await waitFor(() => expect(screen.getByRole("treeitem", { name: "atlas" })).toBeInTheDocument());
    fireEvent.contextMenu(screen.getByRole("button", { name: /code/ }));
    fireEvent.click(await screen.findByRole("menuitem", { name: t("removeFolder") }));
    expect(screen.getByRole("alertdialog")).toHaveTextContent(t("confirmRemoveFolderHint"));
    fireEvent.click(screen.getByRole("button", { name: t("removeFolder") }));
    await waitFor(() => expect(onRemoveFolder).toHaveBeenCalledWith("C:\\code", ["atlas", "portal"]));
  });
  it("exposes the sort menu and reports sort changes", async () => {
    const onSort = vi.fn();
    renderList({ onSort });
    fireEvent.click(await screen.findByRole("button", { name: t("sortBy") }));
    fireEvent.click(await screen.findByRole("menuitem", { name: t("sortName") }));
    expect(onSort).toHaveBeenCalledWith("name");
  });

  it("reveals and follows a selection made outside the list, even inside a collapsed group", () => {
    const projects = [project("alpha", "C:\\code\\web\\alpha"), project("beta", "C:\\code\\tools\\beta")];
    const view = renderList({ projects, allProjects: projects, selectedId: "alpha" });
    const tree = screen.getByRole("tree");
    expect(screen.getByRole("treeitem", { name: "alpha" })).toBeInTheDocument();

    fireEvent.click(within(screen.getByRole("treeitem", { name: "tools" })).getByRole("button"));
    expect(screen.queryByRole("treeitem", { name: "beta" })).not.toBeInTheDocument();

    view.rerender(<ProjectList {...view.props} selectedId="beta" />);

    expect(screen.getByRole("treeitem", { name: "beta" })).toBeInTheDocument();
    expect(tree).toHaveAttribute("aria-activedescendant", "project:beta");
  });

  it("expands and collapses every group from the tree tools", async () => {
    renderList();
    await waitFor(() => expect(screen.getByRole("treeitem", { name: "atlas" })).toBeInTheDocument());

    fireEvent.click(screen.getByRole("button", { name: t("collapseAll") }));
    expect(screen.queryByRole("treeitem", { name: "atlas" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: t("revealSelected") })).toBeEnabled();

    fireEvent.click(screen.getByRole("button", { name: t("expandAll") }));
    expect(screen.getByRole("treeitem", { name: "atlas" })).toBeInTheDocument();
    expect(screen.getByRole("treeitem", { name: "portal" })).toBeInTheDocument();
  });

  it("collapses every path except the selected project's ancestors", async () => {
    const projects = [project("alpha", "C:\\code\\web\\alpha"), project("beta", "C:\\code\\tools\\beta")];
    renderList({ projects, allProjects: projects, selectedId: "alpha" });
    await waitFor(() => expect(screen.getByRole("treeitem", { name: "alpha" })).toBeInTheDocument());

    fireEvent.click(screen.getByRole("button", { name: t("focusSelected") }));

    expect(screen.getByRole("treeitem", { name: "alpha" })).toBeInTheDocument();
    expect(screen.queryByRole("treeitem", { name: "beta" })).not.toBeInTheDocument();
    expect(screen.getByRole("tree")).toHaveAttribute("aria-activedescendant", "project:alpha");
  });

  it("jumps back to the selected project after keyboard focus moves elsewhere", async () => {
    renderList();
    const tree = screen.getByRole("tree");
    await waitFor(() => expect(screen.getByRole("treeitem", { name: "portal" })).toBeInTheDocument());

    fireEvent.keyDown(tree, { key: "ArrowDown" });
    fireEvent.keyDown(tree, { key: "ArrowDown" });
    fireEvent.keyDown(tree, { key: "ArrowDown" });
    expect(tree).toHaveAttribute("aria-activedescendant", "project:portal");

    fireEvent.click(screen.getByRole("button", { name: t("revealSelected") }));
    expect(tree).toHaveAttribute("aria-activedescendant", "project:atlas");
  });

  it("keeps tree-only tools unavailable in flat layout", async () => {
    renderList();
    await waitFor(() => expect(screen.getByRole("treeitem", { name: "atlas" })).toBeInTheDocument());

    fireEvent.click(screen.getByRole("button", { name: t("flatView") }));

    expect(screen.getByRole("button", { name: t("expandAll") })).toBeDisabled();
    expect(screen.getByRole("button", { name: t("collapseAll") })).toBeDisabled();
    expect(screen.getByRole("button", { name: t("focusSelected") })).toBeDisabled();
    expect(screen.getByRole("button", { name: t("revealSelected") })).toBeEnabled();
  });

});
