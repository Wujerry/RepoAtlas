import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { dictionaries, type MessageKey } from "../i18n";
import type { ProjectSummary } from "../types";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { searchProjects } = vi.hoisted(() => ({ searchProjects: vi.fn() }));

vi.mock("../lib/api", () => ({
  api: { searchProjects },
}));

import { CommandPalette } from "../components/CommandPalette";

const t = (key: MessageKey) => dictionaries.en[key];

function project(id = "project-1"): ProjectSummary {
  return {
    id,
    canonicalPath: `C:/code/${id}`,
    displayName: "Clinical Atlas",
    detectedName: "Clinical Atlas",
    description: null,
    notes: null,
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
    updatedAt: "2026-08-21T00:00:00Z",
  };
}

function renderPalette(overrides: Partial<React.ComponentProps<typeof CommandPalette>> = {}) {
  const onClose = vi.fn();
  const runScan = vi.fn();
  const runJump = vi.fn();
  const currentProject = project();
  render(
    <CommandPalette
      open
      onClose={onClose}
      t={t}
      projects={[currentProject]}
      actions={[
        { id: "scan-all", title: "Scan all", run: runScan },
        { id: `jump:${currentProject.id}`, title: "Jump to project", run: runJump },
      ]}
      {...overrides}
    />,
  );
  return { onClose, runScan, runJump, currentProject };
}

describe("CommandPalette", () => {
  beforeEach(() => {
    searchProjects.mockReset();
  });

  it("focuses and clears the search field when opened", async () => {
    renderPalette();

    const input = await screen.findByRole("combobox");
    const listbox = screen.getByRole("listbox");
    await waitFor(() => expect(input).toHaveFocus());
    expect(input).toHaveValue("");
    expect(input).toHaveAttribute("aria-haspopup", "listbox");
    expect(input).toHaveAttribute("aria-controls", listbox.id);
    expect(input).toHaveAttribute("aria-activedescendant");
    expect(screen.getByRole("group", { name: t("commands") })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: t("projects") })).toBeInTheDocument();
  });

  it("moves with ArrowDown and executes the active item with Enter", async () => {
    const { onClose, runJump } = renderPalette();
    const input = await screen.findByRole("combobox");
    const projectItem = await screen.findByRole("option", { name: /Clinical Atlas/ });

    expect(projectItem).toHaveAttribute("aria-selected", "false");
    fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(projectItem).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(input, { key: "Enter" });

    expect(runJump).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("keeps projects discoverable when the caller supplies an explicit project handler", async () => {
    const onProject = vi.fn();
    const currentProject = project();
    renderPalette({
      actions: [{ id: "scan-all", title: "Scan all", run: vi.fn() }],
      onProject,
    });

    const projectItem = await screen.findByRole("option", { name: /Clinical Atlas/ });
    fireEvent.click(projectItem);

    expect(onProject).toHaveBeenCalledWith(currentProject.id);
  });

  it("uses full-text search for a non-empty query", async () => {
    const currentProject = project();
    searchProjects.mockResolvedValueOnce([{ project: currentProject, score: 12 }]);
    renderPalette({ projects: [] });

    const input = await screen.findByRole("combobox");
    fireEvent.change(input, { target: { value: "clinical" } });

    await waitFor(() => expect(searchProjects).toHaveBeenCalledWith("clinical"));
    expect(await screen.findByRole("option", { name: /Clinical Atlas/ })).toBeInTheDocument();
  });
});
