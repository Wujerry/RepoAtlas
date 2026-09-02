import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ComponentProps } from "react";
import { describe, expect, it, vi } from "vitest";
import { UpdateSettings } from "../components/UpdateSettings";
import { dictionaries, type MessageKey } from "../i18n";
import type { UpdateState } from "../types";

const t = (key: MessageKey) => dictionaries.en[key];
const notify = vi.fn();

function state(overrides: Partial<UpdateState> = {}): UpdateState {
  return {
    status: "available",
    currentVersion: "0.1.0",
    version: "0.2.0",
    notes: "A safer project library.",
    downloadedBytes: 0,
    restartRequired: false,
    installed: false,
    ...overrides,
  };
}

function renderUpdates(nextState: UpdateState = state(), overrides: Partial<ComponentProps<typeof UpdateSettings>> = {}) {
  return render(
    <UpdateSettings
      state={nextState}
      t={t}
      notify={notify}
      onCheck={vi.fn(async () => nextState)}
      onDownload={vi.fn(async () => nextState)}
      onInstall={vi.fn(async () => true)}
      onRestart={vi.fn(async () => undefined)}
      onDefer={vi.fn(async () => undefined)}
      {...overrides}
    />,
  );
}

describe("UpdateSettings", () => {
  it("shows target version and release notes for an available update", () => {
    renderUpdates();

    expect(screen.getByText("Update available")).toBeInTheDocument();
    expect(screen.getByText("v0.1.0")).toBeInTheDocument();
    expect(screen.getByText("v0.2.0")).toBeInTheDocument();
    expect(screen.getByText("A safer project library.")).toBeInTheDocument();
  });

  it("requires confirmation before installing a downloaded update", async () => {
    const onInstall = vi.fn(async () => true);
    renderUpdates(state({ status: "ready", version: "0.2.0" }), { onInstall });

    fireEvent.click(screen.getByRole("button", { name: "Install update" }));
    expect(screen.getByText("Confirm update installation")).toBeInTheDocument();
    expect(onInstall).not.toHaveBeenCalled();

    const installButtons = screen.getAllByRole("button", { name: "Install update" });
    fireEvent.click(installButtons[installButtons.length - 1]);
    await waitFor(() => expect(onInstall).toHaveBeenCalledOnce());
  });

  it("renders determinate download progress", () => {
    renderUpdates(state({ status: "downloading", downloadedBytes: 25, contentLength: 100 }));

    expect(screen.getByRole("progressbar")).toHaveAttribute("value", "25");
    expect(screen.getByText("25 B / 100 B")).toBeInTheDocument();
  });
  it("retries the failed download instead of checking again", async () => {
    const onDownload = vi.fn(async () => state({ status: "error", errorStage: "download" }));
    const onCheck = vi.fn(async () => state());
    renderUpdates(state({ status: "error", errorStage: "download", error: "network reset" }), { onDownload, onCheck });

    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await waitFor(() => expect(onDownload).toHaveBeenCalledOnce());
    expect(onCheck).not.toHaveBeenCalled();
  });

  it("retries a failed restart", async () => {
    const onRestart = vi.fn(async () => undefined);
    const onCheck = vi.fn(async () => state());
    renderUpdates(state({ status: "error", errorStage: "restart", error: "relaunch blocked" }), { onRestart, onCheck });

    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await waitFor(() => expect(onRestart).toHaveBeenCalledOnce());
    expect(onCheck).not.toHaveBeenCalled();
  });

  it("does not claim the app is up to date after postponing an update", () => {
    renderUpdates(state({ status: "idle", checkedAt: "2026-08-28T00:00:00Z", deferred: true, version: undefined }));

    expect(screen.getByText("Update postponed")).toBeInTheDocument();
    expect(screen.queryByText("You're up to date")).not.toBeInTheDocument();
  });

});
