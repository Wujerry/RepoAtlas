import { afterEach, describe, expect, it, vi } from "vitest";

describe("shiki client", () => {
  const originalWorker = globalThis.Worker;

  afterEach(() => {
    globalThis.Worker = originalWorker;
    vi.resetModules();
  });

  it("keeps independent highlight requests and cancels only the requested one", async () => {
    class MockWorker {
      static instance: MockWorker;
      onmessage?: (event: MessageEvent) => void;
      onerror?: (event: ErrorEvent) => void;
      postMessage = vi.fn();
      terminate = vi.fn();

      constructor() {
        MockWorker.instance = this;
      }
    }
    globalThis.Worker = MockWorker as unknown as typeof Worker;
    const { highlightCode } = await import("../lib/shiki-client");

    const first = highlightCode("const first = 1", "typescript");
    const controller = new AbortController();
    const second = highlightCode("const second = 2", "typescript", false, controller.signal);

    expect(MockWorker.instance.postMessage).toHaveBeenNthCalledWith(1, expect.objectContaining({ type: "highlight", id: 1 }));
    expect(MockWorker.instance.postMessage).toHaveBeenNthCalledWith(2, expect.objectContaining({ type: "highlight", id: 2 }));

    controller.abort();
    expect(MockWorker.instance.postMessage).toHaveBeenNthCalledWith(3, { type: "cancel", id: 2 });
    await expect(second).rejects.toMatchObject({ name: "AbortError" });

    const lines = [[{ content: "const first = 1", offset: 0 }]];
    MockWorker.instance.onmessage?.({ data: { id: 1, lines } } as MessageEvent);
    await expect(first).resolves.toEqual(lines);
  });
});
