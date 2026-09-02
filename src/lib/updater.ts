import { check as tauriCheck, type DownloadEvent } from "@tauri-apps/plugin-updater";
import { relaunch as tauriRelaunch } from "@tauri-apps/plugin-process";
import packageJson from "../../package.json";
import type { UpdateErrorStage, UpdateState } from "../types";

export interface UpdateHandle {
  currentVersion: string;
  version: string;
  date?: string;
  body?: string;
  download: (onEvent?: (event: DownloadEvent) => void) => Promise<void>;
  install: () => Promise<void>;
  close?: () => Promise<void>;
}

export interface UpdaterDriver {
  check: (options?: { timeout?: number }) => Promise<UpdateHandle | null>;
  relaunch: () => Promise<void>;
}

export interface UpdaterListener {
  (state: UpdateState): void;
}

export const UPDATE_CHECK_TIMEOUT_MS = 10_000;
export const currentVersion = packageJson.version;

function initialState(): UpdateState {
  return {
    status: "idle",
    currentVersion,
    downloadedBytes: 0,
    restartRequired: false,
    installed: false,
  };
}

function errorMessage(error: unknown): string {
  if (error instanceof Error && error.message) return error.message;
  if (typeof error === "string" && error) return error;
  try {
    const serialized = JSON.stringify(error);
    return serialized ?? String(error);
  } catch {
    return String(error);
  }
}

function isWindowsRuntime(): boolean {
  return typeof navigator !== "undefined" && navigator.userAgent.toLowerCase().includes("windows");
}

/**
 * Coordinates the Tauri updater resource and exposes a small observable state
 * machine to the UI. Keeping plugin calls here makes startup checks easy to
 * run in the background and keeps settings components platform-agnostic.
 */
export class UpdaterService {
  private state: UpdateState = initialState();
  private handle: UpdateHandle | null = null;
  private readonly listeners = new Set<UpdaterListener>();
  private operation = 0;

  constructor(private readonly driver: UpdaterDriver) {}

  getSnapshot = (): UpdateState => this.state;

  subscribe = (listener: UpdaterListener): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  private setState(next: Partial<UpdateState>): UpdateState {
    this.state = { ...this.state, ...next };
    for (const listener of this.listeners) listener(this.state);
    return this.state;
  }

  private setError(stage: UpdateErrorStage, error: unknown): UpdateState {
    return this.setState({
      status: "error",
      errorStage: stage,
      error: errorMessage(error),
    });
  }

  private async release(handle: UpdateHandle | null): Promise<void> {
    if (!handle?.close) return;
    try {
      await handle.close();
    } catch {
      // Releasing a stale updater resource must never make a later check fail.
    }
  }

  async checkForUpdates(): Promise<UpdateState> {
    if (this.state.status === "checking" || this.state.status === "downloading" || this.state.status === "ready") {
      return this.state;
    }

    const previous = this.handle;
    this.handle = null;
    void this.release(previous);

    const operation = ++this.operation;
    this.setState({
      status: "checking",
      error: undefined,
      errorStage: undefined,
      version: undefined,
      notes: undefined,
      date: undefined,
      downloadedBytes: 0,
      contentLength: undefined,
      restartRequired: false,
      installed: false,
    });

    try {
      const next = await this.driver.check({ timeout: UPDATE_CHECK_TIMEOUT_MS });
      if (operation !== this.operation) {
        await this.release(next);
        return this.state;
      }

      this.handle = next;
      const checkedAt = new Date().toISOString();
      if (!next) {
        return this.setState({ status: "idle", checkedAt, currentVersion, error: undefined, errorStage: undefined, deferred: false });
      }

      return this.setState({
        status: "available",
        currentVersion: next.currentVersion || currentVersion,
        version: next.version,
        notes: next.body,
        date: next.date,
        checkedAt,
        error: undefined,
        errorStage: undefined,
        deferred: false,
      });
    } catch (error) {
      if (operation !== this.operation) return this.state;
      this.handle = null;
      return this.setError("check", error);
    }
  }

  async downloadUpdate(): Promise<UpdateState> {
    const handle = this.handle;
    const canRetryDownload = this.state.status === "error" && this.state.errorStage === "download";
    if (!handle || (this.state.status !== "available" && !canRetryDownload)) {
      return this.setError("download", new Error("No update is available."));
    }

    const operation = ++this.operation;
    this.setState({
      status: "downloading",
      downloadedBytes: 0,
      contentLength: undefined,
      error: undefined,
      errorStage: undefined,
    });

    try {
      await handle.download((event) => {
        if (operation !== this.operation) return;
        if (event.event === "Started") {
          this.setState({ contentLength: event.data.contentLength });
        } else if (event.event === "Progress") {
          this.setState({ downloadedBytes: this.state.downloadedBytes + event.data.chunkLength });
        }
      });
      if (operation !== this.operation) return this.state;
      return this.setState({ status: "ready", error: undefined, errorStage: undefined, restartRequired: false, installed: false });
    } catch (error) {
      if (operation !== this.operation) return this.state;
      return this.setError("download", error);
    }
  }

  /**
   * Installs a downloaded update. Tauri exits automatically on Windows before
   * the installer runs; on macOS/Linux the caller gets a separate restart
   * confirmation opportunity.
   */
  async installUpdate(): Promise<boolean> {
    const handle = this.handle;
    const canRetry = this.state.status === "error" && this.state.errorStage === "install";
    if (!handle || (this.state.status !== "ready" && !canRetry)) {
      this.setError("install", new Error("Download the update before installing it."));
      return false;
    }

    const operation = ++this.operation;
    try {
      await handle.install();
      if (operation !== this.operation) return false;
      this.handle = null;
      void this.release(handle);
      const restartRequired = !isWindowsRuntime();
      this.setState({ status: "ready", restartRequired, error: undefined, errorStage: undefined, installed: true });
      return restartRequired;
    } catch (error) {
      if (operation !== this.operation) return false;
      this.setError("install", error);
      return false;
    }
  }

  async restartApp(): Promise<void> {
    try {
      await this.driver.relaunch();
      this.setState({ restartRequired: false, error: undefined, errorStage: undefined });
    } catch (error) {
      this.setError("restart", error);
      throw error;
    }
  }

  /**
   * Dismisses the current update without installing it. This is intentionally
   * session-scoped; the next manual or startup check can discover it again.
   */
  async deferUpdate(): Promise<void> {
    if (!this.handle && !["available", "ready", "error"].includes(this.state.status)) return;
    ++this.operation;
    const handle = this.handle;
    this.handle = null;
    await this.release(handle);
    this.setState({
      status: "idle",
      version: undefined,
      notes: undefined,
      date: undefined,
      downloadedBytes: 0,
      contentLength: undefined,
      error: undefined,
      errorStage: undefined,
      restartRequired: false,
      installed: false,
      deferred: true,
    });
  }
}

export const updaterService = new UpdaterService({ check: tauriCheck, relaunch: tauriRelaunch });
