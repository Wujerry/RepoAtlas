import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { QuickSearchRequest } from "../lib/quick-search";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn(), label: "main" }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));
vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({ label: mocks.label }),
}));

let transport: typeof import("../lib/quick-search");
let queue: QuickSearchRequest[];
let listeners: Set<() => void>;
let disposers: Array<() => void>;

const navigation = (requestId: string): QuickSearchRequest => ({
  requestId,
  action: "open-main",
  target: { kind: "sessions", sessionId: "session-one", messageIndex: 0 },
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(yes => { resolve = yes; });
  return { promise, resolve };
}

function notifyPending() {
  listeners.forEach(listener => listener());
}

beforeEach(async () => {
  vi.resetAllMocks();
  vi.resetModules();
  mocks.label = "main";
  queue = [];
  listeners = new Set();
  disposers = [];
  mocks.listen.mockImplementation(async (_event: string, listener: () => void) => {
    listeners.add(listener);
    return () => listeners.delete(listener);
  });
  // Model the Rust mailbox's at-least-once transport, including idempotent stale acks.
  mocks.invoke.mockImplementation(async (command: string, args: { acknowledgedRequestId: string | null }) => {
    expect(command).toBe("quick_search_ready");
    expect(listeners.size).toBeGreaterThan(0);
    if (args.acknowledgedRequestId === queue[0]?.requestId) queue.shift();
    return queue[0] ?? null;
  });
  transport = await import("../lib/quick-search");
});

afterEach(() => disposers.forEach(dispose => dispose()));

describe("quick-search window transport", () => {
  it("waits for listener installation, then replays navigation queued before bootstrap", async () => {
    queue.push(navigation("before-bootstrap"));
    const listening = deferred<void>();
    const install = mocks.listen.getMockImplementation()!;
    mocks.listen.mockImplementationOnce(async (...args) => {
      await listening.promise;
      return install(...args);
    });
    const handler = vi.fn();
    const onError = vi.fn();
    const connection = transport.connectQuickSearchRequests(handler, onError);
    expect(mocks.invoke).not.toHaveBeenCalled();
    listening.resolve();
    const dispose = await connection;
    disposers.push(dispose);
    await vi.waitFor(() => expect(queue).toHaveLength(0));
    expect(mocks.listen).toHaveBeenCalledWith("quick-search://pending", expect.any(Function), {
      target: { kind: "WebviewWindow", label: "main" },
    });
    expect(handler).toHaveBeenCalledExactlyOnceWith(navigation("before-bootstrap"));
    expect(onError).not.toHaveBeenCalled();
    dispose();
    expect(listeners.size).toBe(0);
  });

  it("retains failed navigation without acknowledging it, then retries on another notification", async () => {
    queue.push(navigation("retry"));
    const failure = new Error("session read failed");
    const handler = vi.fn().mockRejectedValueOnce(failure).mockResolvedValue(undefined);
    const onError = vi.fn();
    disposers.push(await transport.connectQuickSearchRequests(handler, onError));
    await vi.waitFor(() => expect(onError).toHaveBeenCalledWith(failure));
    expect(queue).toHaveLength(1);
    expect(mocks.invoke).toHaveBeenCalledExactlyOnceWith("quick_search_ready", { acknowledgedRequestId: null });
    notifyPending();
    await vi.waitFor(() => expect(queue).toHaveLength(0));
    expect(handler).toHaveBeenCalledTimes(2);
  });

  it.each(["before", "after"])("retries an ack failure %s backend acceptance without duplicating handling or dropping the next request", async timing => {
    queue.push(navigation("one"), navigation("two"));
    const receive = mocks.invoke.getMockImplementation()!;
    let fail = true;
    mocks.invoke.mockImplementation(async (command, args) => {
      if (args.acknowledgedRequestId && fail) {
        fail = false;
        if (timing === "after") await receive(command, args);
        throw new Error("ack transport failed");
      }
      return receive(command, args);
    });
    const handler = vi.fn();
    const onError = vi.fn();
    disposers.push(await transport.connectQuickSearchRequests(handler, onError));
    await vi.waitFor(() => expect(onError).toHaveBeenCalledTimes(1));
    expect(handler).toHaveBeenCalledExactlyOnceWith(navigation("one"));
    notifyPending();
    await vi.waitFor(() => expect(queue).toHaveLength(0));
    expect(handler.mock.calls.map(([request]) => request.requestId)).toEqual(["one", "two"]);
    expect(mocks.invoke.mock.calls.filter(([, args]) => args.acknowledgedRequestId === "one")).toHaveLength(2);
  });

  it("serializes repeated notifications and receives requests arriving during an unfinished handler", async () => {
    queue.push(navigation("one"));
    const handling = deferred<void>();
    const handler = vi.fn(async request => {
      if (request.requestId === "one") await handling.promise;
    });
    const onError = vi.fn();
    disposers.push(await transport.connectQuickSearchRequests(handler, onError));
    await vi.waitFor(() => expect(handler).toHaveBeenCalledTimes(1));
    queue.push(navigation("two"));
    notifyPending(); notifyPending(); notifyPending();
    expect(handler).toHaveBeenCalledTimes(1);
    handling.resolve();
    await vi.waitFor(() => expect(queue).toHaveLength(0));
    expect(handler.mock.calls.map(([request]) => request.requestId)).toEqual(["one", "two"]);
    expect(onError).not.toHaveBeenCalled();
  });

  it("replays to the remounted consumer after the disposed in-flight handler finishes", async () => {
    queue.push(navigation("remount"));
    const handling = deferred<void>();
    const oldHandler = vi.fn(() => handling.promise);
    const onError = vi.fn();
    const dispose = await transport.connectQuickSearchRequests(oldHandler, onError);
    disposers.push(dispose);
    await vi.waitFor(() => expect(oldHandler).toHaveBeenCalledTimes(1));
    dispose();
    const nextHandler = vi.fn();
    disposers.push(await transport.connectQuickSearchRequests(nextHandler, onError));
    expect(nextHandler).not.toHaveBeenCalled();
    handling.resolve();
    await vi.waitFor(() => expect(queue).toHaveLength(0));
    expect(nextHandler).toHaveBeenCalledExactlyOnceWith(navigation("remount"));
    expect(mocks.invoke.mock.calls.filter(([, args]) => args.acknowledgedRequestId === "remount")).toHaveLength(1);
    expect(onError).not.toHaveBeenCalled();
  });

  it("does not handle or acknowledge a ready response arriving after disposal", async () => {
    const reading = deferred<QuickSearchRequest | null>();
    mocks.invoke.mockImplementationOnce(() => reading.promise);
    const handler = vi.fn();
    const onError = vi.fn();
    const dispose = await transport.connectQuickSearchRequests(handler, onError);
    disposers.push(dispose);
    await vi.waitFor(() => expect(mocks.invoke).toHaveBeenCalledTimes(1));
    dispose();
    reading.resolve(navigation("stale"));
    await reading.promise;
    expect(handler).not.toHaveBeenCalled();
    expect(mocks.invoke).toHaveBeenCalledTimes(1);
    expect(onError).not.toHaveBeenCalled();
  });

  it("handles focus through the quick-search window mailbox without requesting main navigation", async () => {
    mocks.label = "quick-search";
    const focus: QuickSearchRequest = { requestId: "focus-one", action: "focus" };
    queue.push(focus);
    const handler = vi.fn();
    const onError = vi.fn();
    disposers.push(await transport.connectQuickSearchRequests(handler, onError));
    await vi.waitFor(() => expect(queue).toHaveLength(0));
    expect(mocks.listen).toHaveBeenCalledWith("quick-search://pending", expect.any(Function), {
      target: { kind: "WebviewWindow", label: "quick-search" },
    });
    expect(handler).toHaveBeenCalledExactlyOnceWith(focus);
    expect(onError).not.toHaveBeenCalled();
  });
});
