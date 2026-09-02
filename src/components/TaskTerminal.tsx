import { useEffect, useRef, useState } from "react";
import { api } from "../lib/api";
import { setInputOwner } from "../lib/task-runs";
import type { TaskRun } from "../types";

type XtermSession = {
  term: { open: (host: HTMLElement) => void; write: (data: string) => void; reset: () => void; dispose: () => void; focus: () => void; cols: number; rows: number; onData: (cb: (data: string) => void) => { dispose: () => void }; options: { theme?: unknown; fontFamily?: string } };
  fit: { fit: () => void };
  written: number;
};

function themeFor(mode: "dark" | "light") {
  return mode === "light"
    ? { background: "#f6f5f2", foreground: "#1f2328", cursor: "#1f2328" }
    : { background: "#090909", foreground: "#d4d4d4", cursor: "#ffa31a" };
}

function writeLog(session: { term: { write: (data: string) => void; reset: () => void }; written: number }, log: string) {
  if (log.length < session.written) {
    session.term.reset();
    session.written = 0;
  }
  if (log.length > session.written) {
    session.term.write(log.slice(session.written));
    session.written = log.length;
  }
}

function isTestEnvironment() {
  const env = (import.meta as { env?: { VITEST?: boolean; MODE?: string } }).env;
  return Boolean(env?.VITEST) || env?.MODE === "test" || (typeof navigator !== "undefined" && /jsdom/i.test(navigator.userAgent));
}

function resolveConsoleFont(node?: Element | null) {
  const stack = (node ? getComputedStyle(node) : getComputedStyle(document.documentElement)).getPropertyValue("--console-font").trim();
  return stack || '"Maple Mono NF", "Maple Mono NF CN", "CaskaydiaCove NF", "Cascadia Code", "Sarasa Gothic SC", "Microsoft YaHei Mono", "Geist Mono", ui-monospace, monospace';
}

export function TaskTerminal({
  run,
  log,
  interactive,
  theme = "dark",
}: {
  run: TaskRun;
  log: string;
  interactive: boolean;
  theme?: "dark" | "light";
}) {
  const hostRef = useRef<HTMLDivElement>(null);
  const logRef = useRef(log);
  const sessionRef = useRef<XtermSession | null>(null);
  const [canMountXterm] = useState(() => typeof window !== "undefined" && typeof ResizeObserver === "function" && !isTestEnvironment());

  logRef.current = log;

  useEffect(() => {
    const host = hostRef.current;
    if (!host || !canMountXterm) return;
    let cancelled = false;
    let observer: ResizeObserver | undefined;
    let fontObserver: MutationObserver | undefined;
    let frame = 0;
    let disposable: { dispose: () => void } | undefined;
    void Promise.all([import("@xterm/xterm"), import("@xterm/addon-fit"), import("@xterm/xterm/css/xterm.css")]).then(([{ Terminal }, { FitAddon }]) => {
      if (cancelled || !host.isConnected) return;
      const term = new Terminal({
        convertEol: true,
        fontFamily: resolveConsoleFont(host),
        fontSize: 12,
        cursorBlink: true,
        allowProposedApi: false,
        theme: themeFor(theme),
      });
      const fit = new FitAddon();
      term.loadAddon(fit);
      const session = { term, fit, written: 0 };
      sessionRef.current = session;
      term.open(host);
      writeLog(session, logRef.current);

      const resize = () => {
        if (!host.isConnected || host.clientWidth < 8 || host.clientHeight < 8) return;
        fit.fit();
        if (interactive) {
          void api.resizeTaskRun(run.id, term.cols, term.rows).catch(() => undefined);
        }
      };
      const applyFont = () => {
        term.options.fontFamily = resolveConsoleFont(host);
        resize();
      };
      observer = new ResizeObserver(resize);
      observer.observe(host);
      fontObserver = new MutationObserver(applyFont);
      fontObserver.observe(document.documentElement, { attributes: true, attributeFilter: ["style"] });
      frame = window.requestAnimationFrame(resize);
      disposable = interactive
        ? term.onData((data) => {
            setInputOwner(run.id);
            void api.writeTaskStdin(run.id, data).catch(() => undefined);
          })
        : undefined;
      if (interactive) {
        setInputOwner(run.id);
        term.focus();
      }
    });
    return () => {
      cancelled = true;
      window.cancelAnimationFrame(frame);
      observer?.disconnect();
      fontObserver?.disconnect();
      disposable?.dispose();
      sessionRef.current?.term.dispose();
      sessionRef.current = null;
    };
  }, [canMountXterm, interactive, run.id, theme]);

  useEffect(() => {
    if (!canMountXterm) return;
    const session = sessionRef.current;
    if (!session) return;
    writeLog(session, log);
  }, [canMountXterm, log]);

  if (!canMountXterm) {
    return <pre className={"log-pane log-pane--" + theme}>{log}</pre>;
  }
  return <div ref={hostRef} className={"task-xterm task-xterm--" + theme} />;
}
