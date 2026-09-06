import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useLibrarySync } from "../lib/library-sync";

describe("shared library synchronization", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.spyOn(document, "visibilityState", "get").mockReturnValue("visible");
  });
  afterEach(() => { vi.useRealTimers(); vi.restoreAllMocks(); });

  it("reloads every external change, including metadata-only changes after discovery", async () => {
    const checkVersion = vi.fn().mockResolvedValue(1);
    const onChange = vi.fn().mockResolvedValue(true);
    renderHook(() => useLibrarySync({ initialVersion: 1, checkVersion, onChange, onError: vi.fn() }));
    await act(async () => { await vi.advanceTimersByTimeAsync(2000); });
    expect(onChange).not.toHaveBeenCalled();
    for (const version of [2, 3, 4]) {
      checkVersion.mockResolvedValue(version);
      await act(async () => { window.dispatchEvent(new Event("focus")); });
      expect(onChange).toHaveBeenCalledTimes(version - 1);
    }
  });

  it("retries interrupted reloads and uses the current navigation callback", async () => {
    const checkVersion = vi.fn().mockResolvedValue(2);
    const interrupted = vi.fn().mockResolvedValue(false);
    const current = vi.fn().mockResolvedValue(true);
    const view = renderHook(({ onChange }) => useLibrarySync({ initialVersion: 1, checkVersion, onChange, onError: vi.fn() }), { initialProps: { onChange: interrupted } });
    await act(async () => {});
    view.rerender({ onChange: current });
    await act(async () => { await vi.advanceTimersByTimeAsync(2000); });
    expect(current).toHaveBeenCalledOnce();
    await act(async () => { await vi.advanceTimersByTimeAsync(2000); });
    expect(current).toHaveBeenCalledOnce();
  });

  it("does not overlap requests or apply a late response after unmount", async () => {
    let finish!: (version: number) => void;
    const checkVersion = vi.fn(() => new Promise<number>((resolve) => { finish = resolve; }));
    const onChange = vi.fn().mockResolvedValue(true);
    const view = renderHook(() => useLibrarySync({ initialVersion: 1, checkVersion, onChange, onError: vi.fn() }));
    await act(async () => { await vi.advanceTimersByTimeAsync(6000); window.dispatchEvent(new Event("focus")); });
    expect(checkVersion).toHaveBeenCalledOnce();
    view.unmount();
    await act(async () => { finish(2); });
    expect(onChange).not.toHaveBeenCalled();
  });

  it("reports an unavailable database once and recovers on a later poll", async () => {
    const checkVersion = vi.fn().mockRejectedValue(new Error("busy"));
    const onError = vi.fn();
    const onChange = vi.fn().mockResolvedValue(true);
    renderHook(() => useLibrarySync({ initialVersion: 1, checkVersion, onChange, onError }));
    await act(async () => { await vi.advanceTimersByTimeAsync(4000); });
    expect(onError).toHaveBeenCalledOnce();
    checkVersion.mockResolvedValue(2);
    await act(async () => { await vi.advanceTimersByTimeAsync(2000); });
    expect(onChange).toHaveBeenCalledOnce();
  });
});
