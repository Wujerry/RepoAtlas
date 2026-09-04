import type { ThemedToken } from "shiki/core";

type HighlightResponse = { id: number; lines?: ThemedToken[][]; error?: string };
type PendingHighlight = {
  resolve: (lines: ThemedToken[][]) => void;
  reject: (error: Error) => void;
  cleanup: () => void;
};

let worker: Worker | undefined;
let sequence = 0;
const pending = new Map<number, PendingHighlight>();

function getWorker() {
  if (worker) return worker;
  worker = new Worker(new URL("../workers/shiki.worker.ts", import.meta.url), { type: "module" });
  worker.onmessage = (event: MessageEvent<HighlightResponse>) => {
    const request = pending.get(event.data.id);
    if (!request) return;
    pending.delete(event.data.id);
    request.cleanup();
    if (event.data.error) request.reject(new Error(event.data.error));
    else request.resolve(event.data.lines ?? []);
  };
  worker.onerror = (event) => {
    const error = new Error(event.message || "Syntax highlighting worker failed");
    for (const request of pending.values()) {
      request.cleanup();
      request.reject(error);
    }
    pending.clear();
    worker?.terminate();
    worker = undefined;
  };
  return worker;
}

export function highlightCode(code: string, language = "text", dark = false, signal?: AbortSignal) {
  const id = ++sequence;
  if (signal?.aborted) {
    return Promise.reject(new DOMException("Syntax highlighting request was canceled", "AbortError"));
  }
  const response = new Promise<ThemedToken[][]>((resolve, reject) => {
    const abort = () => {
      const request = pending.get(id);
      if (!request) return;
      pending.delete(id);
      request.cleanup();
      getWorker().postMessage({ type: "cancel", id });
      reject(new DOMException("Syntax highlighting request was canceled", "AbortError"));
    };
    signal?.addEventListener("abort", abort, { once: true });
    pending.set(id, {
      resolve,
      reject,
      cleanup: () => signal?.removeEventListener("abort", abort),
    });
  });
  try {
    getWorker().postMessage({ type: "highlight", id, code, language, theme: dark ? "github-dark" : "github-light" });
  } catch (reason) {
    pending.get(id)?.cleanup();
    pending.delete(id);
    return Promise.reject(reason instanceof Error ? reason : new Error(String(reason)));
  }
  return response;
}
