import { afterEach, describe, expect, it } from "vitest";
import { findNewlyAddedProject, ONBOARDING_STORAGE_KEY, readOnboardingState, resolveOnboardingVisibility, writeOnboardingState } from "../lib/onboarding";

afterEach(() => {
  window.localStorage.clear();
});

describe("onboarding visibility", () => {
  it("opens a fresh empty library and records in-progress", () => {
    expect(resolveOnboardingVisibility({ stored: null, projectCount: 0, scanRootCount: 0 })).toEqual({
      open: true,
      write: "in-progress",
    });
  });

  it("silently completes for upgrade users who already have projects", () => {
    expect(resolveOnboardingVisibility({ stored: null, projectCount: 2, scanRootCount: 0 })).toEqual({
      open: false,
      write: "completed",
    });
  });

  it("silently completes for upgrade users who already have scan roots", () => {
    expect(resolveOnboardingVisibility({ stored: null, projectCount: 0, scanRootCount: 1 })).toEqual({
      open: false,
      write: "completed",
    });
  });

  it("resumes an in-progress empty library", () => {
    expect(resolveOnboardingVisibility({ stored: "in-progress", projectCount: 0, scanRootCount: 1 })).toEqual({
      open: true,
    });
  });

  it("completes in-progress when an Agent already added a project", () => {
    expect(resolveOnboardingVisibility({ stored: "in-progress", projectCount: 1, scanRootCount: 0 })).toEqual({
      open: false,
      write: "completed",
    });
  });

  it("does not reopen after completion", () => {
    expect(resolveOnboardingVisibility({ stored: "completed", projectCount: 0, scanRootCount: 0 })).toEqual({
      open: false,
    });
  });

  it("reads and writes the local UI preference", () => {
    expect(readOnboardingState()).toBeNull();
    expect(writeOnboardingState("in-progress")).toBe(true);
    expect(window.localStorage.getItem(ONBOARDING_STORAGE_KEY)).toBe("in-progress");
    expect(readOnboardingState()).toBe("in-progress");
  });

  it("survives unavailable storage", () => {
    const broken = {
      getItem: () => { throw new Error("blocked"); },
      setItem: () => { throw new Error("blocked"); },
    };
    expect(readOnboardingState(broken)).toBeNull();
    expect(writeOnboardingState("completed", broken)).toBe(false);
  });

  it("detects a newly added project against the copied baseline", () => {
    const found = findNewlyAddedProject(["old"], [{ id: "old" }, { id: "new", displayName: "Atlas" }]);
    expect(found).toEqual({ id: "new", displayName: "Atlas" });
  });
});
