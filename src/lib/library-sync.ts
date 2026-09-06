import { useEffect, useRef } from "react";

export function useLibrarySync(options: {
  initialVersion: number | null;
  checkVersion: () => Promise<number>;
  onChange: () => Promise<boolean>;
  onError: (error: unknown) => void;
  intervalMs?: number;
}) {
  const callbacks = useRef(options);
  callbacks.current = options;
  const { initialVersion, intervalMs = 2000 } = options;
  useEffect(() => {
    if (initialVersion == null) return;
    let version = initialVersion;
    let stopped = false;
    let busy = false;
    let failed = false;
    const check = async () => {
      if (stopped || busy || document.visibilityState === "hidden") return;
      busy = true;
      try {
        const next = await callbacks.current.checkVersion();
        if (stopped) return;
        if (next !== version) {
          // An interrupted/failed reload must be retried, never acknowledged.
          if (await callbacks.current.onChange()) version = next;
        }
        failed = false;
      } catch (error) {
        if (!stopped && !failed) callbacks.current.onError(error);
        failed = true;
      } finally {
        busy = false;
      }
    };
    const onFocus = () => { void check(); };
    const timer = window.setInterval(onFocus, intervalMs);
    window.addEventListener("focus", onFocus);
    document.addEventListener("visibilitychange", onFocus);
    void check();
    return () => {
      stopped = true;
      window.clearInterval(timer);
      window.removeEventListener("focus", onFocus);
      document.removeEventListener("visibilitychange", onFocus);
    };
  }, [initialVersion, intervalMs]);
}
