import { useEffect, useSyncExternalStore } from "react";
import { ResourceCache } from "./resource-cache";

export function useCachedResource<T>(cache: ResourceCache<T>, key: string | undefined, delay = 0) {
  const snapshot = useSyncExternalStore(cache.subscribe, () => key ? cache.peek(key) : undefined);
  const revision = useSyncExternalStore(cache.subscribe, () => cache.revision);
  useEffect(() => {
    if (!key) return;
    const timer = window.setTimeout(() => { void cache.read(key).catch(() => undefined); }, delay);
    return () => window.clearTimeout(timer);
  }, [cache, key, revision, delay]);
  return {
    value: snapshot?.value,
    loading: Boolean(key && (!snapshot || snapshot.loading)),
    error: snapshot?.error,
    refresh: () => key ? cache.read(key, true).catch(() => undefined) : Promise.resolve(undefined),
  };
}
