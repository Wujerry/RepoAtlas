import { buildProjectTree, collectGroupIds, collectProjectsInGroup, flattenProjectTree, isSameOrAncestorPath, scanRootsUnderPath, sortProjects, splitCanonicalPath } from "../lib/project-tree";
import type { ProjectSummary } from "../types";
import { describe, expect, it } from "vitest";

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
    favorite: false,
    origin: "scan",
    scanRootId: null,
    languages: [],
    frameworks: [],
    packageManagers: [],
    tags: [],
    sourceMtime: null,
    lastCommitAt: null,
    lastOpenedAt: null,
    updatedAt: "2026-08-21T00:00:00Z",
  };
}

function withTimes(id: string, canonicalPath: string, lastOpenedAt: string | null, updatedAt: string): ProjectSummary {
  return { ...project(id, canonicalPath), lastOpenedAt, updatedAt };
}

describe("project tree", () => {
  it("splits Windows, UNC, POSIX, and mixed-separator paths", () => {
    expect(splitCanonicalPath("C:\\code/mixed\\repo")).toEqual(["C:", "code", "mixed", "repo"]);
    expect(splitCanonicalPath("\\\\server/share/repo")).toEqual(["\\\\server\\share", "repo"]);
    expect(splitCanonicalPath("/home/user/repo")).toEqual(["/", "home", "user", "repo"]);
  });

  it("groups projects by their real path and counts descendants", () => {
    const roots = buildProjectTree([
      project("atlas", "C:\\code\\atlas"),
      project("portal", "C:/code/portal"),
      project("docs", "C:\\docs"),
    ]);
    expect(roots.map((root) => root.label)).toEqual(["C:"]);
    expect(roots[0]?.projectCount).toBe(3);
    expect(roots[0]?.children.map((child) => child.label)).toEqual(["code"]);
    expect(roots[0]?.children[0]?.projects.map((item) => item.id)).toEqual(["atlas", "portal"]);
    expect(roots[0]?.projects.map((item) => item.id)).toEqual(["docs"]);
  });

  it("flattens only expanded branches and preserves tree metadata", () => {
    const roots = buildProjectTree([project("atlas", "C:\\code\\atlas")]);
    const allExpanded = new Set(collectGroupIds(roots));
    const expandedRows = flattenProjectTree(roots, allExpanded);
    expect(expandedRows.map((row) => row.kind)).toEqual(["group", "group", "project"]);
    expect(expandedRows[expandedRows.length - 1]).toMatchObject({ kind: "project", id: "project:atlas", depth: 3, siblingIndex: 1 });

    const collapsedRows = flattenProjectTree(roots, new Set([roots[0]!.id]));
    expect(collapsedRows.map((row) => row.kind)).toEqual(["group", "group"]);
    expect(collapsedRows[1]).toMatchObject({ kind: "group", label: "code", expanded: false });
  });
  it("treats mixed separators and case as the same Windows path", () => {
    const roots = buildProjectTree([
      project("one", "C:\\Code\\Atlas"),
      project("two", "c:/code/portal"),
    ]);
    expect(roots).toHaveLength(1);
    expect(roots[0]?.children).toHaveLength(1);
    expect(roots[0]?.children[0]?.projects.map((item) => item.id).sort()).toEqual(["one", "two"]);
  });
  it("lists sibling folders before projects, then sorts each group by path name", () => {
    const roots = buildProjectTree([
      project("zebra", "F:\\code\\zebra"),
      project("alpha", "F:\\code\\alpha"),
      project("clients", "F:\\code\\kh\\new_emergency\\clients"),
      project("api", "F:\\code\\api"),
    ]);
    const rows = flattenProjectTree(roots, new Set(collectGroupIds(roots)));
    const labels = rows.map((row) => row.kind === "project" ? row.project.id : row.label);
    expect(labels.indexOf("kh")).toBeGreaterThan(-1);
    expect(labels.indexOf("kh")).toBeLessThan(labels.indexOf("alpha"));
    expect(labels.indexOf("alpha")).toBeLessThan(labels.indexOf("api"));
    expect(labels.indexOf("api")).toBeLessThan(labels.indexOf("zebra"));
  });
  it("matches folder descendants and scan roots without treating siblings as children", () => {
    expect(isSameOrAncestorPath("C:\\code", "C:\\code\\atlas")).toBe(true);
    expect(isSameOrAncestorPath("C:\\code", "C:\\code")).toBe(true);
    expect(isSameOrAncestorPath("C:\\code", "C:\\codex\\atlas")).toBe(false);
    const roots = buildProjectTree([
      project("atlas", "C:\\code\\atlas"),
      project("portal", "C:\\code\\portal"),
      project("docs", "C:\\docs"),
    ]);
    const code = roots[0]?.children[0];
    expect(code).toBeTruthy();
    expect(collectProjectsInGroup(roots, code!.id).map((item) => item.id).sort()).toEqual(["atlas", "portal"]);
    expect(scanRootsUnderPath([{ id: "root-code", path: "C:\\code" }, { id: "root-docs", path: "C:\\docs" }], "C:\\code").map((root) => root.id)).toEqual(["root-code"]);
  });
  it("sortProjects supports recency, name, and path orders with stable fallbacks", () => {
    const opened = withTimes("opened", "C:\\code\\portal", "2026-08-21T10:00:00Z", "2026-08-21T00:00:00Z");
    const older = withTimes("older", "C:\\code\\atlas", "2026-08-20T10:00:00Z", "2026-08-19T00:00:00Z");
    const never = withTimes("never", "C:\\code\\api", null, "2026-08-18T00:00:00Z");
    const list = [opened, never, older];
    expect(sortProjects(list, "default").map((item) => item.id)).toEqual(["opened", "never", "older"]);
    expect(sortProjects(list, "name").map((item) => item.id)).toEqual(["never", "older", "opened"]);
    expect(sortProjects(list, "path").map((item) => item.id)).toEqual(["never", "older", "opened"]);
    expect(sortProjects(list, "recent-opened").map((item) => item.id)).toEqual(["opened", "older", "never"]);
    expect(sortProjects(list, "recent-updated").map((item) => item.id)).toEqual(["opened", "older", "never"]);
  });
  it("applies the requested sort inside groups while keeping path-based group order", () => {
    const roots = buildProjectTree([
      withTimes("zulu", "F:\\code\\zulu", "2026-08-21T10:00:00Z", "2026-08-21T00:00:00Z"),
      withTimes("alpha", "F:\\code\\alpha", "2026-08-22T10:00:00Z", "2026-08-22T00:00:00Z"),
    ], "recent-opened");
    expect(roots[0]?.children[0]?.projects.map((item) => item.id)).toEqual(["alpha", "zulu"]);
  });
  it("builds a five-thousand-folder tree without quadratic child-map rebuilding", () => {
    const projects = Array.from({ length: 5_000 }, (_, index) => (
      project(`project-${index}`, `F:\\code\\group-${index}\\project-${index}`)
    ));
    const started = performance.now();
    const roots = buildProjectTree(projects);
    const elapsed = performance.now() - started;
    expect(roots[0]?.projectCount).toBe(5_000);
    expect(elapsed).toBeLessThan(750);
  });
});
