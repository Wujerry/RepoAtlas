import { render, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { dictionaries, type MessageKey } from "../i18n";
import type { ProjectSummary } from "../types";

const { listProjectDirectory, cancelProjectPathIndex } = vi.hoisted(() => ({
  listProjectDirectory: vi.fn(),
  cancelProjectPathIndex: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("../lib/api", () => ({
  api: {
    listProjectDirectory,
    cancelProjectPathIndex,
    cancelProjectPathSearch: vi.fn().mockResolvedValue(undefined),
    ensureProjectPathIndex: vi.fn(),
    searchProjectPaths: vi.fn(),
    readProjectFilePreview: vi.fn(),
    revealProjectPath: vi.fn(),
  },
  onFileIndexProgress: vi.fn(async () => () => undefined),
}));

import { fileTreeIndent, FilesWorkspace } from "../components/FilesWorkspace";

const project: ProjectSummary = {
  id: "atlas",
  canonicalPath: "C:/code/atlas",
  displayName: "Atlas",
  detectedName: "Atlas",
  notes: null,
  description: null,
  vcsKind: "git",
  availability: "ready",
  archived: false,
  favorite: false,
  origin: "scan",
  scanRootId: "root",
  languages: ["TypeScript"],
  frameworks: ["React"],
  packageManagers: ["pnpm"],
  tags: [],
  sourceMtime: null,
  lastCommitAt: null,
  lastOpenedAt: null,
  updatedAt: "2026-09-03T00:00:00Z",
};

const t = (key: MessageKey) => dictionaries.en[key];

describe("FilesWorkspace", () => {
  it("does not read the root until first activation and keeps the directory cache while hidden", async () => {
    listProjectDirectory.mockResolvedValue({
      path: "",
      skippedCount: 1,
      entries: [{ name: "src", path: "src", kind: "directory", generated: false }, { name: "README.md", path: "README.md", kind: "file", generated: false }],
    });
    const { rerender } = render(<FilesWorkspace project={project} active={false} t={t} notify={vi.fn()} />);
    expect(listProjectDirectory).not.toHaveBeenCalled();

    rerender(<FilesWorkspace project={project} active t={t} notify={vi.fn()} />);
    await waitFor(() => expect(listProjectDirectory).toHaveBeenCalledTimes(1));
    rerender(<FilesWorkspace project={project} active={false} t={t} notify={vi.fn()} />);
    rerender(<FilesWorkspace project={project} active t={t} notify={vi.fn()} />);
    expect(listProjectDirectory).toHaveBeenCalledTimes(1);
  });

  it("adds visible indentation at every nested directory level", () => {
    expect(fileTreeIndent(0)).toBe(8);
    expect(fileTreeIndent(1)).toBe(26);
    expect(fileTreeIndent(2)).toBe(44);
  });
});
