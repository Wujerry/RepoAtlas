import { useEffect, useRef } from "react";

export const ONBOARDING_STORAGE_KEY = "repoatlas.onboarding.v1";
export const ONBOARDING_POLL_MS = 2000;

export type OnboardingState = "in-progress" | "completed";

export function readOnboardingState(storage?: Pick<Storage, "getItem"> | null): OnboardingState | null {
  try {
    const value = (storage ?? globalThis.localStorage).getItem(ONBOARDING_STORAGE_KEY);
    return value === "in-progress" || value === "completed" ? value : null;
  } catch {
    return null;
  }
}

export function writeOnboardingState(state: OnboardingState, storage?: Pick<Storage, "setItem"> | null): boolean {
  try {
    (storage ?? globalThis.localStorage).setItem(ONBOARDING_STORAGE_KEY, state);
    return true;
  } catch {
    return false;
  }
}

export function resolveOnboardingVisibility(input: {
  stored: OnboardingState | null;
  projectCount: number;
  scanRootCount: number;
}): { open: boolean; write?: OnboardingState } {
  if (input.stored === "completed") return { open: false };
  if (input.projectCount > 0) {
    return { open: false, write: "completed" };
  }
  if (input.stored === "in-progress") return { open: true };
  if (input.scanRootCount > 0) return { open: false, write: "completed" };
  return { open: true, write: "in-progress" };
}

export function findNewlyAddedProject<T extends { id: string }>(baselineIds: Iterable<string>, projects: T[]): T | undefined {
  const baseline = new Set(baselineIds);
  return projects.find((project) => !baseline.has(project.id));
}

export function useOnboardingProjectWatch<T extends { id: string }>(options: {
  enabled: boolean;
  baselineIds: Iterable<string> | null;
  listProjects: () => Promise<T[]>;
  onFound: (project: T) => void;
  onError?: (error: unknown) => void;
  intervalMs?: number;
}): { checkNow: () => Promise<T | undefined> } {
  const { enabled, baselineIds, listProjects, onFound, onError, intervalMs = ONBOARDING_POLL_MS } = options;
  const onFoundRef = useRef(onFound);
  const onErrorRef = useRef(onError);
  const listProjectsRef = useRef(listProjects);
  const baselineRef = useRef(baselineIds);
  onFoundRef.current = onFound;
  onErrorRef.current = onError;
  listProjectsRef.current = listProjects;
  baselineRef.current = baselineIds;

  const checkNow = async () => {
    const baseline = baselineRef.current;
    if (!baseline) return undefined;
    try {
      const projects = await listProjectsRef.current();
      const found = findNewlyAddedProject(baseline, projects);
      if (found) onFoundRef.current(found);
      return found;
    } catch (error) {
      onErrorRef.current?.(error);
      return undefined;
    }
  };

  useEffect(() => {
    if (!enabled || !baselineIds) return;
    let cancelled = false;
    const run = async () => {
      if (cancelled) return;
      await checkNow();
    };
    void run();
    const timer = window.setInterval(() => { void run(); }, intervalMs);
    const onFocus = () => { void run(); };
    window.addEventListener("focus", onFocus);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
      window.removeEventListener("focus", onFocus);
    };
  }, [baselineIds, enabled, intervalMs]);

  return { checkNow };
}
