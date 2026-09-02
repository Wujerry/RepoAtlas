import { useEffect, useState } from "react";
import { api } from "../lib/api";

const SPLASH_MS = 1200;

export function SplashScreen({
  ready,
  onFinished,
}: {
  ready: boolean;
  onFinished: () => void;
}) {
  const [shown, setShown] = useState(false);
  const [elapsed, setElapsed] = useState(false);
  const reduced = typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  useEffect(() => {
    void api.showMainWindow().then(() => setShown(true)).catch(() => setShown(true));
  }, []);

  useEffect(() => {
    if (reduced) {
      setElapsed(true);
      return;
    }
    const timer = window.setTimeout(() => setElapsed(true), SPLASH_MS);
    return () => window.clearTimeout(timer);
  }, [reduced]);

  useEffect(() => {
    if (shown && ready && elapsed) onFinished();
  }, [elapsed, onFinished, ready, shown]);

  return (
    <div className={"splash-screen" + (reduced ? " is-static" : "")} role="status" aria-live="polite">
      <div className="splash-brand">
        <span className="splash-icon">
          <img src="/repoatlas-mark.png" alt="" />
        </span>
        <div className="splash-wordmark" aria-hidden="true">
          <span>RepoAtlas</span>
        </div>
      </div>
      <span className="sr-only">RepoAtlas</span>
    </div>
  );
}
