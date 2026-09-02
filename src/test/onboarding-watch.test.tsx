import { act, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ONBOARDING_POLL_MS, useOnboardingProjectWatch } from "../lib/onboarding";

function Watcher(props: {
  enabled: boolean;
  baselineIds: string[] | null;
  listProjects: () => Promise<Array<{ id: string }>>;
  onFound: (project: { id: string }) => void;
  onError?: (error: unknown) => void;
}) {
  useOnboardingProjectWatch(props);
  return null;
}

describe("onboarding project watch", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("polls while waiting and stops after a new project appears", async () => {
    const listProjects = vi.fn()
      .mockResolvedValueOnce([{ id: "old" }])
      .mockResolvedValueOnce([{ id: "old" }, { id: "new" }]);
    const onFound = vi.fn();
    const { rerender } = render(
      <Watcher enabled baselineIds={["old"]} listProjects={listProjects} onFound={onFound} />,
    );

    await act(async () => { await Promise.resolve(); });
    expect(onFound).not.toHaveBeenCalled();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(ONBOARDING_POLL_MS);
      await Promise.resolve();
    });
    expect(onFound).toHaveBeenCalledWith({ id: "new" });

    rerender(<Watcher enabled={false} baselineIds={["old"]} listProjects={listProjects} onFound={onFound} />);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(ONBOARDING_POLL_MS * 2);
    });
    expect(listProjects).toHaveBeenCalledTimes(2);
  });

  it("checks immediately when the window is focused", async () => {
    const listProjects = vi.fn()
      .mockResolvedValueOnce([{ id: "old" }])
      .mockResolvedValueOnce([{ id: "old" }, { id: "fresh" }]);
    const onFound = vi.fn();
    render(<Watcher enabled baselineIds={["old"]} listProjects={listProjects} onFound={onFound} />);
    await act(async () => { await Promise.resolve(); });
    expect(onFound).not.toHaveBeenCalled();

    await act(async () => {
      window.dispatchEvent(new Event("focus"));
      await Promise.resolve();
    });
    expect(onFound).toHaveBeenCalledWith({ id: "fresh" });
  });

  it("keeps polling after a failed check", async () => {
    const listProjects = vi.fn()
      .mockRejectedValueOnce(new Error("offline"))
      .mockResolvedValueOnce([{ id: "new" }]);
    const onFound = vi.fn();
    const onError = vi.fn();
    render(<Watcher enabled baselineIds={["old"]} listProjects={listProjects} onFound={onFound} onError={onError} />);

    await act(async () => { await Promise.resolve(); });
    expect(onError).toHaveBeenCalledTimes(1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(ONBOARDING_POLL_MS);
      await Promise.resolve();
    });
    expect(onFound).toHaveBeenCalledWith({ id: "new" });
  });
});
