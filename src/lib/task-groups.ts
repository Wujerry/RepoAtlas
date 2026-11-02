import type { TaskDefinition } from "../types";

export type TaskGrouping = "auto" | "kind" | "none";

/** Presentation only: never rewrite task definitions or their working directories. */
export function taskGroupKeys(tasks: TaskDefinition[], mode: TaskGrouping): Map<string, string> {
  const directories = new Set(tasks.map(task => task.cwd?.replace(/\\/g, "/").replace(/\/$/, "") || "."));
  const prefixes = new Map<string, number>();
  for (const task of tasks) {
    const prefix = task.name.includes(":") ? task.name.split(":")[0] : "";
    if (prefix) prefixes.set(prefix, (prefixes.get(prefix) ?? 0) + 1);
  }
  return new Map(tasks.map(task => {
    if (mode === "none") return [task.id, ""];
    if (mode === "auto" && directories.size > 1) return [task.id, task.cwd?.replace(/\\/g, "/").replace(/\/$/, "") || "."];
    const prefix = task.name.includes(":") ? task.name.split(":")[0] : "";
    return [task.id, mode === "auto" && (prefixes.get(prefix) ?? 0) > 1 ? `${prefix}:*` : task.kind];
  }));
}
