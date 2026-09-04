import type { ProjectSummary } from "../types";

export interface ProjectPathGroup {
  id: string;
  label: string;
  path: string;
  depth: number;
  children: ProjectPathGroup[];
  projects: ProjectSummary[];
  projectCount: number;
}

export interface ProjectTreeGroupRow {
  kind: "group";
  id: string;
  label: string;
  path: string;
  depth: number;
  projectCount: number;
  expanded: boolean;
  parentId?: string;
  siblingIndex: number;
  siblingCount: number;
}

export interface ProjectTreeProjectRow {
  kind: "project";
  id: string;
  project: ProjectSummary;
  depth: number;
  parentId: string;
  siblingIndex: number;
  siblingCount: number;
}

export type ProjectTreeRow = ProjectTreeGroupRow | ProjectTreeProjectRow;

export type ProjectSort = "default" | "recent-opened" | "recent-updated" | "recent-committed" | "name" | "path";

interface ParsedPath {
  segments: string[];
  windows: boolean;
}

const segmentCollator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });

function parsePath(input: string): ParsedPath {
  const raw = input.trim();
  const unc = /^[/\\]{2}/.test(raw);
  const normalized = raw.replace(/[\\/]+/g, "\\");
  const windowsDrive = /^[A-Za-z]:/.test(normalized);

  if (unc) {
    const parts = raw.replace(/^[/\\]{2}/, "").split(/[\\/]+/).filter(Boolean);
    if (parts.length === 0) return { segments: ["\\\\"], windows: true };
    const shareRoot = `\\\\${parts.shift()!}${parts.length ? `\\${parts.shift()!}` : ""}`;
    return { segments: [shareRoot, ...parts], windows: true };
  }

  if (windowsDrive) {
    const drive = normalized.slice(0, 2);
    const rest = normalized.slice(2).split("\\").filter(Boolean);
    return { segments: [drive, ...rest], windows: true };
  }

  if (raw.startsWith("/")) {
    return { segments: ["/", ...raw.replace(/^\/+/, "").split(/[\\/]+/).filter(Boolean)], windows: false };
  }

  const segments = raw.split(/[\\/]+/).filter(Boolean);
  return { segments: segments.length ? segments : ["."], windows: false };
}

export function splitCanonicalPath(path: string): string[] {
  return parsePath(path).segments;
}

export function groupLabel(path: string, depth: number): string {
  const segments = splitCanonicalPath(path);
  const segment = segments[Math.max(0, depth - 1)] ?? path;
  if (depth === 1 && /^[A-Za-z]:$/.test(segment)) return `${segment}\\`;
  return segment;
}

function joinPath(parent: string, segment: string): string {
  if (parent === "/") return `/${segment}`;
  if (parent.endsWith("\\")) return `${parent}${segment}`;
  return parent ? `${parent}\\${segment}` : segment;
}

function pathKey(path: string, windows: boolean): string {
  const normalized = path.replace(/[\\/]+/g, "\\");
  return `path:${windows ? normalized.toLowerCase() : normalized}`;
}

function makeGroup(id: string, label: string, path: string, depth: number): ProjectPathGroup {
  return { id, label, path, depth, children: [], projects: [], projectCount: 0 };
}

function lastSegment(path: string): string {
  const segments = splitCanonicalPath(path);
  return segments[segments.length - 1] ?? path;
}

function compareLabels(a: string, b: string): number {
  return segmentCollator.compare(a, b);
}

function compareGroups(a: ProjectPathGroup, b: ProjectPathGroup): number {
  return compareLabels(a.label, b.label) || compareLabels(a.path, b.path);
}

function compareProjects(a: ProjectSummary, b: ProjectSummary): number {
  const aKey = lastSegment(a.canonicalPath) || a.displayName;
  const bKey = lastSegment(b.canonicalPath) || b.displayName;
  return compareLabels(aKey, bKey) || compareLabels(a.displayName, b.displayName) || compareLabels(a.canonicalPath, b.canonicalPath);
}

const timeValue = (value: string | null | undefined): number => (value ? Date.parse(value) : 0);

export function sortProjects(projects: readonly ProjectSummary[], sort: ProjectSort): ProjectSummary[] {
  if (sort === "default") return [...projects];
  const sorted = [...projects];
  const byTimeDesc = (pick: (project: ProjectSummary) => string | null, fallback: (project: ProjectSummary) => string | null) => (a: ProjectSummary, b: ProjectSummary): number =>
    timeValue(pick(b)) - timeValue(pick(a)) || timeValue(fallback(b)) - timeValue(fallback(a)) || compareLabels(a.displayName, b.displayName);
  switch (sort) {
    case "recent-opened":
      sorted.sort(byTimeDesc((project) => project.lastOpenedAt, (project) => project.updatedAt));
      break;
    case "recent-updated":
      sorted.sort(byTimeDesc((project) => project.updatedAt, (project) => project.updatedAt));
      break;
    case "recent-committed":
      sorted.sort(byTimeDesc((project) => project.lastCommitAt, (project) => project.updatedAt));
      break;
    case "name":
      sorted.sort((a, b) => compareLabels(a.displayName, b.displayName) || compareLabels(a.canonicalPath, b.canonicalPath));
      break;
    case "path":
      sorted.sort((a, b) => compareLabels(a.canonicalPath, b.canonicalPath) || compareLabels(a.displayName, b.displayName));
      break;
  }
  return sorted;
}

export function buildProjectTree(projects: readonly ProjectSummary[], sort: ProjectSort = "default"): ProjectPathGroup[] {
  const roots = new Map<string, ProjectPathGroup>();
  const childMaps = new Map<string, Map<string, ProjectPathGroup>>();

  for (const project of projects) {
    const parsed = parsePath(project.canonicalPath);
    const groupSegments = parsed.segments.length > 1 ? parsed.segments.slice(0, -1) : parsed.segments;
    let parent: ProjectPathGroup | undefined;
    let currentPath = "";
    let currentMap = roots;

    groupSegments.forEach((segment, index) => {
      currentPath = index === 0 ? (segment === "/" ? "/" : segment) : joinPath(currentPath, segment);
      const id = pathKey(currentPath, parsed.windows);
      let group = currentMap.get(id);
      if (!group) {
        group = makeGroup(id, segment, currentPath, index + 1);
        currentMap.set(id, group);
        childMaps.set(id, new Map());
        if (parent) parent.children.push(group);
      }
      parent = group;
      currentMap = childMaps.get(id)!;
      if (index === 0 && !roots.has(id)) roots.set(id, group);
    });

    if (!parent) continue;
    parent.projects.push(project);
  }

  const sortGroups = (groups: ProjectPathGroup[]) => {
    groups.sort(compareGroups);
    for (const group of groups) {
      group.children.sort(compareGroups);
      group.projects = sort === "default" ? [...group.projects].sort(compareProjects) : sortProjects(group.projects, sort);
      sortGroups(group.children);
      group.projectCount = group.projects.length + group.children.reduce((count, child) => count + child.projectCount, 0);
    }
  };
  const result = [...roots.values()];
  sortGroups(result);
  return result;
}

export function collectGroupIds(groups: readonly ProjectPathGroup[]): string[] {
  return groups.flatMap((group) => [group.id, ...collectGroupIds(group.children)]);
}

export function flattenProjectTree(groups: readonly ProjectPathGroup[], expanded: ReadonlySet<string>): ProjectTreeRow[] {
  const rows: ProjectTreeRow[] = [];

  const appendGroup = (group: ProjectPathGroup, parentId: string | undefined, siblingIndex: number, siblingCount: number) => {
    rows.push({
      kind: "group",
      id: `group:${group.id}`,
      label: group.label,
      path: group.path,
      depth: group.depth,
      projectCount: group.projectCount,
      expanded: expanded.has(group.id),
      parentId,
      siblingIndex,
      siblingCount,
    });
    if (!expanded.has(group.id)) return;

    const childGroups = [...group.children].sort(compareGroups);
    const childProjects = group.projects;
    const childCount = childGroups.length + childProjects.length;
    childGroups.forEach((child, index) => {
      appendGroup(child, group.id, index + 1, childCount);
    });
    childProjects.forEach((project, index) => {
      rows.push({
        kind: "project",
        id: `project:${project.id}`,
        project,
        depth: group.depth + 1,
        parentId: group.id,
        siblingIndex: childGroups.length + index + 1,
        siblingCount: childCount,
      });
    });
  };

  groups.forEach((group, index) => appendGroup(group, undefined, index + 1, groups.length));
  return rows;
}

export function ancestorGroupIds(groups: readonly ProjectPathGroup[], projectId: string): string[] {
  const result: string[] = [];
  const visit = (group: ProjectPathGroup): boolean => {
    if (group.projects.some((project) => project.id === projectId)) {
      result.push(group.id);
      return true;
    }
    for (const child of group.children) {
      if (visit(child)) {
        result.push(group.id);
        return true;
      }
    }
    return false;
  };
  groups.some(visit);
  return result;
}

function comparableSegment(segment: string, windows: boolean): string {
  return windows ? segment.toLowerCase() : segment;
}

export function isSameOrAncestorPath(ancestor: string, path: string): boolean {
  const parent = parsePath(ancestor);
  const child = parsePath(path);
  const windows = parent.windows || child.windows;
  if (parent.segments.length > child.segments.length) return false;
  return parent.segments.every((segment, index) => (
    comparableSegment(segment, windows) === comparableSegment(child.segments[index] ?? "", windows)
  ));
}

export function findGroup(groups: readonly ProjectPathGroup[], groupId: string): ProjectPathGroup | undefined {
  for (const group of groups) {
    if (group.id === groupId) return group;
    const nested = findGroup(group.children, groupId);
    if (nested) return nested;
  }
  return undefined;
}

export function collectProjectsInGroup(groups: readonly ProjectPathGroup[], groupId: string): ProjectSummary[] {
  const group = findGroup(groups, groupId);
  if (!group) return [];
  const collect = (item: ProjectPathGroup): ProjectSummary[] => [
    ...item.projects,
    ...item.children.flatMap(collect),
  ];
  return collect(group);
}

export function scanRootsUnderPath<T extends { id: string; path: string }>(
  scanRoots: readonly T[],
  folderPath: string,
): T[] {
  return scanRoots.filter((root) => isSameOrAncestorPath(folderPath, root.path));
}
