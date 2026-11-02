import { describe, expect, it } from "vitest";
import { taskGroupKeys } from "../lib/task-groups";
import type { TaskDefinition } from "../types";
const task = (id: string, name: string, cwd: string | null = null): TaskDefinition => ({ id, name, cwd, kind: "build", executable: "pnpm", argv: ["run", name], inferred: true, shellMode: false });
describe("task presentation groups", () => {
  it("uses directories before script prefixes and normalizes separators without changing definitions", () => {
    const tasks = [task("1", "build:a", "packages\\web"), task("2", "build:b", "packages/web/"), task("3", "build:c")];
    expect([...taskGroupKeys(tasks, "auto").values()]).toEqual(["packages/web", "packages/web", "."]);
    expect(tasks[0].cwd).toBe("packages\\web");
  });
  it("groups repeated prefixes and falls back to kind, independent of filtering", () => {
    const tasks = [task("1", "build:a"), task("2", "build:b"), task("3", "check")];
    expect([...taskGroupKeys(tasks, "auto").values()]).toEqual(["build:*", "build:*", "build"]);
    expect([...taskGroupKeys(tasks, "kind").values()]).toEqual(["build", "build", "build"]);
    expect([...taskGroupKeys(tasks, "none").values()]).toEqual(["", "", ""]);
  });
});
