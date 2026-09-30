import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

export interface QuickSearchStatus {
  /** Ctrl+Shift+K on Windows/Linux; Cmd+Shift+K on macOS. */
  shortcut: string;
  registered: boolean;
  /** Registration or window operation failure; null after recovery. */
  error: string | null;
}

export type QuickSearchTarget =
  | { kind: "project"; projectId: string }
  | { kind: "sessions"; sessionId?: string; sources?: boolean; messageIndex?: number };

export type QuickSearchRequest =
  | { requestId: string; action: "focus" }
  | { requestId: string; action: "open-main"; target: QuickSearchTarget };

export const quickSearchApi = {
  status: () => invoke<QuickSearchStatus>("quick_search_status"),
  retryRegistration: () => invoke<QuickSearchStatus>("quick_search_retry_registration"),
  show: () => invoke<void>("quick_search_show"),
  hide: () => invoke<void>("quick_search_hide"),
  /** Queues navigation, wakes main, and hides quick search. Returns its request ID. */
  openMain: (target: QuickSearchTarget) => invoke<string>("quick_search_open_main", { target }),
  /** Low-level replay/ack protocol. Prefer connectQuickSearchRequests for UI integration. */
  ready: (acknowledgedRequestId: string | null = null) =>
    invoke<QuickSearchRequest | null>("quick_search_ready", { acknowledgedRequestId }),
};

const currentTarget = () => ({
  target: { kind: "WebviewWindow" as const, label: getCurrentWebviewWindow().label },
});

export function onQuickSearchStatusChanged(
  handler: (status: QuickSearchStatus) => void,
): Promise<UnlistenFn> {
  return listen<QuickSearchStatus>("quick-search://status", (event) => handler(event.payload), currentTarget());
}

// Serialize drains across reconnects too (including React StrictMode remounts).
let deliveryTail: Promise<void> = Promise.resolve();

/**
 * Connect once per window, after main bootstrap or standalone palette mount.
 * Listens before replaying requests so opening during bootstrap cannot lose navigation.
 * Main handles open-main; quick-search handles focus. Throw/reject on failure so the
 * request stays queued, then reconnect or trigger another request to retry.
 *
 * Handlers must be idempotent: reload between handling and acknowledgement can replay
 * a request. Navigation is safe to replay; do not perform Agent resume in this handler.
 * Resume remains an explicit sessionApi action. Use hide() for Escape, never on blur.
 * Dispose the returned listener on unmount (including late resolution after unmount).
 */
export async function connectQuickSearchRequests(
  handler: (request: QuickSearchRequest) => void | Promise<void>,
  onError: (error: Error) => void,
): Promise<UnlistenFn> {
  let disposed = false;
  let running = false;
  let wakeRequested = false;
  // Keep a successful handler's ack across transient IPC failures, avoiding a duplicate
  // handler call when only the acknowledgement response failed.
  let acknowledgedRequestId: string | null = null;

  const drain = async () => {
    if (disposed) return;
    let request = await quickSearchApi.ready(acknowledgedRequestId);
    acknowledgedRequestId = null;
    while (request && !disposed) {
      await handler(request);
      if (disposed) return;
      acknowledgedRequestId = request.requestId;
      request = await quickSearchApi.ready(acknowledgedRequestId);
      acknowledgedRequestId = null;
    }
  };

  const schedule = () => {
    wakeRequested = true;
    if (disposed || running) return;
    running = true;
    deliveryTail = deliveryTail.then(async () => {
      try {
        do {
          wakeRequested = false;
          await drain();
        } while (!disposed && wakeRequested);
      } catch (error) {
        if (!disposed) onError(error instanceof Error ? error : new Error(String(error)));
      } finally {
        running = false;
      }
    }).catch((error: unknown) => {
      // A consumer's error reporter must not poison future window connections.
      console.error("Quick-search error handler failed", error);
    });
  };

  const unlisten = await listen<void>("quick-search://pending", schedule, currentTarget());
  schedule();
  return () => {
    disposed = true;
    unlisten();
  };
}
