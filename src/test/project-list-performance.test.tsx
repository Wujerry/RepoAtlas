import { act, render, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import type { ComponentProps } from "react";
import type { ProjectSummary } from "../types";
import { dictionaries, type MessageKey } from "../i18n";

const state = vi.hoisted(() => ({ start: 0, count: 0, measure: vi.fn(), read: vi.fn() }));
vi.mock("@tanstack/react-virtual", () => {
  const virtualizer = {
    measure: state.measure, measureElement: () => {}, scrollToIndex: () => {},
    getTotalSize: () => state.count * 84,
    getVirtualItems: () => Array.from({ length: state.count }, (_, index) => ({ index, key: index, start: index * 84, size: 84, end: (index + 1) * 84 }))
      .slice(state.start, state.start + 4),
  };
  return { useVirtualizer: (options: { count: number }) => { state.count = options.count; return virtualizer; } };
});
vi.mock("../lib/api", () => ({ api: { readProjectIcons: state.read } }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
import { ProjectList } from "../components/ProjectList";

function props(): ComponentProps<typeof ProjectList> {
  const projects = ["a", "b", "c", "d", "e"].map(id => ({
    id, canonicalPath: `C:\\code\\${id}`, displayName: id, detectedName: id,
    description: null, notes: null, vcsKind: "none", availability: "ready", archived: false,
    favorite: false, origin: "manual", scanRootId: null, languages: [], frameworks: [],
    packageManagers: [], tags: [], sourceMtime: null, lastCommitAt: null, lastOpenedAt: null,
    updatedAt: "1",
  } as ProjectSummary));
  return { projects, allProjects: projects, selectedId: "a", onSelect: vi.fn(), query: "", onQuery: vi.fn(),
    filters: { language: "", tag: "" }, onFilters: vi.fn(), scanning: false, progress: null,
    t: (key: MessageKey) => dictionaries.en[key], empty: "Empty", onAddRoot: vi.fn(), onRegister: vi.fn(), onScan: vi.fn(), onCancelScan: vi.fn() };
}
beforeEach(() => { state.start = 0; state.measure.mockClear(); state.read.mockReset(); });

it("keeps row measurements when selecting another project in the same expanded folders", async () => {
  state.read.mockResolvedValue([]);
  const initial = props();
  const view = render(<ProjectList {...initial} />);
  await waitFor(() => expect(state.read).toHaveBeenCalled());
  state.measure.mockClear();
  view.rerender(<ProjectList {...initial} selectedId="b" />);
  expect(state.measure).not.toHaveBeenCalled();
});

it("shares pending icons across overlapping viewports and caches offscreen completions", async () => {
  const pending: (() => void)[] = [];
  state.read.mockImplementation((ids: string[]) => new Promise(resolve => pending.push(() => resolve(ids.map(projectId => ({ projectId, kind: "language", source: null, dataUrl: null }))))));
  const initial = props();
  const view = render(<ProjectList {...initial} />);
  await waitFor(() => expect(state.read).toHaveBeenCalledTimes(1));
  state.start = 2;
  view.rerender(<ProjectList {...initial} selectedId="b" />);
  await waitFor(() => expect(state.read).toHaveBeenCalledTimes(2));
  const requested = state.read.mock.calls.flatMap(([ids]) => ids as string[]);
  expect(new Set(requested).size).toBe(requested.length);
  await act(async () => { pending.forEach(resolve => resolve()); });
  state.start = 0;
  view.rerender(<ProjectList {...initial} />);
  expect(state.read).toHaveBeenCalledTimes(2);
});

it("ignores old revision responses while retaining the replacement request", async () => {
  const pending: (() => void)[] = [];
  state.read.mockImplementation((ids: string[]) => {
    const revision = pending.length + 1;
    return new Promise(resolve => pending.push(() => resolve(ids.map(projectId => ({
      projectId, kind: "image", source: null, dataUrl: `data:image/png;base64,revision${revision}`,
    })))));
  });
  const initial = props();
  const view = render(<ProjectList {...initial} />);
  await waitFor(() => expect(state.read).toHaveBeenCalledTimes(1));
  const projects = initial.projects.map(project => ({ ...project, updatedAt: "2" }));
  view.rerender(<ProjectList {...initial} projects={projects} allProjects={projects} />);
  await waitFor(() => expect(state.read).toHaveBeenCalledTimes(2));
  await act(async () => { pending[1](); });
  await act(async () => { pending[0](); });
  view.rerender(<ProjectList {...initial} projects={[...projects]} allProjects={projects} selectedId="b" />);
  expect(state.read).toHaveBeenCalledTimes(2);
  const images = view.container.querySelectorAll(".project-identity-mark img");
  expect(images.length).toBeGreaterThan(0);
  images.forEach(image => expect(image).toHaveAttribute("src", "data:image/png;base64,revision2"));
});
