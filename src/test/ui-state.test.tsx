import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { NAVIGATION_STORAGE_KEY, PROJECT_TABS, readNavigationState, useStoredChoice, writeUiState } from "../lib/ui-state";

afterEach(() => vi.restoreAllMocks());

describe("local UI state", () => {
  it.each(["{broken", "null", "42", "[]", JSON.stringify({ page: "delete", librarySurface: "project", scope: "invalid", sort: {}, filters: null })])("uses safe defaults for malformed state: %s", raw => {
    localStorage.setItem(NAVIGATION_STORAGE_KEY, raw);
    expect(readNavigationState()).toMatchObject({ page: "library", librarySurface: "dashboard", scope: "projects", sort: "default", query: "", filters: { language: "", tag: "" } });
  });

  it("bounds saved strings and ignores unrecognized/transient fields", () => {
    writeUiState(NAVIGATION_STORAGE_KEY, { query: "a".repeat(5000), approvalsOpen: true, filters: { language: 17, tag: "dev" } });
    expect(readNavigationState().query).toHaveLength(4096);
    expect(readNavigationState().filters).toEqual({ language: "", tag: "dev" });
    expect(readNavigationState()).not.toHaveProperty("approvalsOpen");
  });

  it("continues navigation when storage is unavailable", () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => { throw new Error("denied"); });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => { throw new Error("quota"); });
    expect(readNavigationState().page).toBe("library");
    expect(writeUiState(NAVIGATION_STORAGE_KEY, {})).toBe(false);
    const { result } = renderHook(() => useStoredChoice("tab", PROJECT_TABS, "overview"));
    act(() => result.current[1]("files"));
    expect(result.current[0]).toBe("files");
  });

  it("persists a chosen tab immediately and restores it on remount", () => {
    const first = renderHook(() => useStoredChoice("tab", PROJECT_TABS, "overview"));
    act(() => first.result.current[1]("tasks"));
    first.unmount();
    const second = renderHook(() => useStoredChoice("tab", PROJECT_TABS, "overview"));
    expect(second.result.current[0]).toBe("tasks");
  });
});
