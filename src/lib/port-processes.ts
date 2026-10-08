import type { PortConflict } from "../types";

export interface PortConflictContext { projectId: string; taskId: string; ports: number[] }
export function openPortProcesses(context?: PortConflictContext) {
  window.dispatchEvent(new CustomEvent("repoatlas:port-processes", { detail: context }));
}

/** One shared desktop review for starts from Projects and Pending Approvals. */
export function reviewPortConflicts(projectId: string, taskId: string, conflicts: PortConflict[]): Promise<boolean> {
  if (!conflicts.length) return Promise.resolve(false);
  return new Promise(resolve => window.dispatchEvent(new CustomEvent("repoatlas:port-conflicts", {
    detail: { projectId, taskId, conflicts, resolve },
  })));
}
