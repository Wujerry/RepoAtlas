import { afterEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import { InvalidatedResource, ResourceCache } from "../lib/resource-cache";
import { useCachedResource } from "../lib/use-cached-resource";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
afterEach(() => vi.useRealTimers());

describe("overview resource cache", () => {
  it("deduplicates reads, uses fresh values and preserves stale values during refresh", async () => {
    vi.useFakeTimers();
    const pending = deferred<string>();
    const loader = vi.fn().mockResolvedValueOnce("cached").mockReturnValueOnce(pending.promise);
    const cache = new ResourceCache<string>(loader, 100);
    await cache.read("a");
    await cache.read("a");
    expect(loader).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(101);
    const refresh = cache.read("a");
    expect(cache.read("a")).toBe(refresh);
    expect(cache.peek("a")).toMatchObject({ value: "cached", loading: true });
    pending.resolve("fresh");
    await refresh;
    expect(cache.peek("a")).toMatchObject({ value: "fresh", loading: false });
    expect(loader).toHaveBeenCalledTimes(2);
  });

  it("invalidates pending reads without allowing old results to repopulate the cache", async () => {
    const pending = deferred<string>();
    const loader = vi.fn().mockReturnValueOnce(pending.promise).mockResolvedValueOnce("new");
    const cache = new ResourceCache<string>(loader, 100);
    const old = cache.read("a");
    const rejected = expect(old).rejects.toBeInstanceOf(InvalidatedResource);
    await Promise.resolve();
    cache.clear();
    await cache.read("a");
    pending.resolve("old");
    await rejected;
    expect(cache.peek("a")?.value).toBe("new");
  });

  it("keeps the last value on refresh failure and permits explicit retry", async () => {
    const loader = vi.fn().mockResolvedValueOnce("cached").mockRejectedValueOnce(new Error("offline")).mockResolvedValueOnce("retried");
    const cache = new ResourceCache<string>(loader, 60_000);
    await cache.read("a");
    await expect(cache.read("a", true)).rejects.toThrow("offline");
    expect(cache.peek("a")).toMatchObject({ value: "cached", loading: false, error: "Error: offline" });
    await cache.read("a", true);
    expect(cache.peek("a")?.value).toBe("retried");
  });

  it("evicts least recently used values and does not resurrect evicted pending reads", async () => {
    const pending = deferred<string>();
    const cache = new ResourceCache((key: string) => key === "slow" ? pending.promise : Promise.resolve(key), 1000, 2);
    const slow = cache.read("slow");
    await cache.read("a");
    await cache.read("b");
    pending.resolve("slow");
    await slow;
    expect(cache.peek("slow")).toBeUndefined();
    await cache.read("a");
    await cache.read("c");
    expect(cache.peek("b")).toBeUndefined();
    expect(cache.peek("a")?.value).toBe("a");
  });

  it("shows a cached project on the first render and ignores another project's late response", async () => {
    const pending = deferred<string>();
    const cache = new ResourceCache((key: string) => key === "slow" ? pending.promise : Promise.resolve(key), 60_000);
    await cache.read("cached");
    const { result, rerender } = renderHook(({ id }) => useCachedResource(cache, id), { initialProps: { id: "slow" } });
    await waitFor(() => expect(cache.peek("slow")?.loading).toBe(true));
    rerender({ id: "cached" });
    expect(result.current.value).toBe("cached");
    expect(result.current.loading).toBe(false);
    await act(async () => { pending.resolve("wrong-project"); await pending.promise; });
    expect(result.current.value).toBe("cached");
  });

  it("refreshes an active resource after invalidation", async () => {
    const loader = vi.fn().mockResolvedValueOnce("before").mockResolvedValueOnce("after");
    const cache = new ResourceCache<string>(loader, 60_000);
    const { result } = renderHook(() => useCachedResource(cache, "a"));
    await waitFor(() => expect(result.current.value).toBe("before"));
    act(() => cache.clear());
    await waitFor(() => expect(result.current.value).toBe("after"));
  });
});
