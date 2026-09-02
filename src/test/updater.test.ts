import { beforeEach, describe, expect, it, vi } from "vitest";
import { currentVersion, UpdaterService, type UpdateHandle } from "../lib/updater";

function makeHandle(overrides: Partial<UpdateHandle> = {}): UpdateHandle {
  return {
    currentVersion,
    version: "0.2.0",
    body: "A safer project library.",
    date: "2026-08-28T00:00:00Z",
    download: vi.fn(async (onEvent) => {
      onEvent?.({ event: "Started", data: { contentLength: 100 } });
      onEvent?.({ event: "Progress", data: { chunkLength: 40 } });
      onEvent?.({ event: "Progress", data: { chunkLength: 60 } });
      onEvent?.({ event: "Finished" });
    }),
    install: vi.fn(async () => undefined),
    close: vi.fn(async () => undefined),
    ...overrides,
  };
}

describe("UpdaterService", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it("reports an available update and sends a bounded check request", async () => {
    const update = makeHandle();
    const check = vi.fn(async () => update);
    const service = new UpdaterService({ check, relaunch: vi.fn(async () => undefined) });
    const states: string[] = [];
    service.subscribe((state) => states.push(state.status));

    const result = await service.checkForUpdates();

    expect(check).toHaveBeenCalledWith({ timeout: 10_000 });
    expect(result).toMatchObject({ status: "available", version: "0.2.0", notes: "A safer project library." });
    expect(states).toEqual(["checking", "available"]);
  });

  it("keeps the local app usable when no update is returned", async () => {
    const service = new UpdaterService({ check: vi.fn(async () => null), relaunch: vi.fn(async () => undefined) });

    const result = await service.checkForUpdates();

    expect(result.status).toBe("idle");
    expect(result.checkedAt).toBeTruthy();
    expect(result.error).toBeUndefined();
  });

  it("tracks download progress and keeps the update ready for confirmed install", async () => {
    const update = makeHandle();
    const relaunch = vi.fn(async () => undefined);
    const service = new UpdaterService({ check: vi.fn(async () => update), relaunch });
    await service.checkForUpdates();

    const downloaded = await service.downloadUpdate();

    expect(downloaded).toMatchObject({ status: "ready", downloadedBytes: 100, contentLength: 100 });
    expect(update.download).toHaveBeenCalledOnce();
    expect(await service.installUpdate()).toBe(true);
    expect(update.install).toHaveBeenCalledOnce();
    expect(service.getSnapshot()).toMatchObject({ status: "ready", installed: true, restartRequired: true });
    expect(relaunch).not.toHaveBeenCalled();
  });

  it("allows retrying a failed install without downloading again", async () => {
    const install = vi.fn()
      .mockRejectedValueOnce(new Error("installer unavailable"))
      .mockResolvedValueOnce(undefined);
    const update = makeHandle({ install });
    const service = new UpdaterService({ check: vi.fn(async () => update), relaunch: vi.fn(async () => undefined) });
    await service.checkForUpdates();
    await service.downloadUpdate();

    expect(await service.installUpdate()).toBe(false);
    expect(service.getSnapshot()).toMatchObject({ status: "error", errorStage: "install", error: "installer unavailable" });
    expect(await service.installUpdate()).toBe(true);
    expect(install).toHaveBeenCalledTimes(2);
  });

  it("releases a deferred update and can discover it again", async () => {
    const update = makeHandle();
    const check = vi.fn(async () => update);
    const service = new UpdaterService({ check, relaunch: vi.fn(async () => undefined) });
    await service.checkForUpdates();

    await service.deferUpdate();

    expect(update.close).toHaveBeenCalledOnce();
    expect(service.getSnapshot()).toMatchObject({ status: "idle", downloadedBytes: 0, deferred: true });
    await service.checkForUpdates();
    expect(check).toHaveBeenCalledTimes(2);
  });

  it("surfaces check and download failures as recoverable states", async () => {
    const service = new UpdaterService({
      check: vi.fn()
        .mockRejectedValueOnce(new Error("offline"))
        .mockResolvedValueOnce(makeHandle({ download: vi.fn(async () => { throw new Error("network reset"); }) })),
      relaunch: vi.fn(async () => undefined),
    });

    expect((await service.checkForUpdates()).errorStage).toBe("check");
    await service.checkForUpdates();
    expect((await service.downloadUpdate()).errorStage).toBe("download");
  });
  it("retries a failed download without checking again", async () => {
    const download = vi.fn()
      .mockRejectedValueOnce(new Error("network reset"))
      .mockResolvedValueOnce(undefined);
    const update = makeHandle({ download });
    const check = vi.fn(async () => update);
    const service = new UpdaterService({ check, relaunch: vi.fn(async () => undefined) });
    await service.checkForUpdates();

    expect((await service.downloadUpdate()).errorStage).toBe("download");
    expect(await service.downloadUpdate()).toMatchObject({ status: "ready" });
    expect(download).toHaveBeenCalledTimes(2);
    expect(check).toHaveBeenCalledOnce();
  });

  it("rethrows restart failures after recording restart error state", async () => {
    const relaunch = vi.fn().mockRejectedValue(new Error("relaunch blocked"));
    const service = new UpdaterService({ check: vi.fn(async () => makeHandle()), relaunch });
    await service.checkForUpdates();
    await service.downloadUpdate();
    await service.installUpdate();

    await expect(service.restartApp()).rejects.toThrow("relaunch blocked");
    expect(service.getSnapshot()).toMatchObject({ status: "error", errorStage: "restart" });
    await expect(service.restartApp()).rejects.toThrow("relaunch blocked");
    expect(relaunch).toHaveBeenCalledTimes(2);
  });


});
