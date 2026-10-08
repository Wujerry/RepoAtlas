import { useState } from "react";
import type { ProjectScope } from "../types";
import type { ProjectSort } from "./project-tree";

export const NAVIGATION_STORAGE_KEY = "repoatlas.navigation.v1";
export const PROJECT_TABS = ["overview", "files", "git", "tasks"] as const;
export const SETTINGS_TABS = ["general", "sources", "data", "updates"] as const;
const PAGES = ["library", "settings", "help", "sessions", "footprints", "usage"] as const;
const SCOPES: readonly ProjectScope[] = ["projects", "favorites", "recent", "archived"];
const SORTS: readonly ProjectSort[] = ["default", "recent-opened", "recent-updated", "recent-committed", "name", "path"];

export interface NavigationState {
  page: typeof PAGES[number];
  librarySurface: "dashboard" | "project";
  selectedId?: string;
  scope: ProjectScope;
  collectionId?: string;
  query: string;
  filters: { language: string; tag: string };
  sort: ProjectSort;
}

function choice<T extends string>(value: unknown, options: readonly T[], fallback: T): T {
  return options.includes(value as T) ? value as T : fallback;
}

function text(value: unknown): string {
  return typeof value === "string" ? value.slice(0, 4096) : "";
}

function read(key: string): unknown {
  try { return JSON.parse(window.localStorage.getItem(key) ?? "null"); }
  catch { return null; }
}

export function writeUiState(key: string, value: unknown): boolean {
  try { window.localStorage.setItem(key, JSON.stringify(value)); return true; }
  catch { return false; } // Unavailable storage must never prevent local startup/navigation.
}

export function readNavigationState(): NavigationState {
  const raw = read(NAVIGATION_STORAGE_KEY);
  const saved = raw && typeof raw === "object" ? raw as Record<string, unknown> : {};
  const filters = saved.filters && typeof saved.filters === "object" ? saved.filters as Record<string, unknown> : {};
  const selectedId = text(saved.selectedId) || undefined;
  return {
    page: choice(saved.page, PAGES, "library"),
    librarySurface: saved.librarySurface === "project" && selectedId ? "project" : "dashboard",
    selectedId,
    scope: choice(saved.scope, SCOPES, "projects"),
    collectionId: text(saved.collectionId) || undefined,
    query: text(saved.query),
    filters: { language: text(filters.language), tag: text(filters.tag) },
    sort: choice(saved.sort, SORTS, "default"),
  };
}

// Write on a user change: no debounce/unload race, and mounting a hidden
// surface cannot overwrite the saved choice with its temporary default.
export function useStoredChoice<T extends string>(key: string, options: readonly T[], fallback: T) {
  const [value, setValue] = useState<T>(() => choice(read(key), options, fallback));
  const select = (next: T) => {
    const valid = choice(next, options, fallback);
    setValue(valid);
    writeUiState(key, valid);
  };
  return [value, select] as const;
}
