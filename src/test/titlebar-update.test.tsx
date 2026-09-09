import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ComponentProps } from "react";
import { describe, expect, it, vi } from "vitest";
import { TitleBarUpdate } from "../components/TitleBarUpdate";
import { dictionaries, type MessageKey } from "../i18n";
import type { UpdateState } from "../types";

const t = (key: MessageKey) => dictionaries.en[key];

function state(overrides: Partial<UpdateState> = {}): UpdateState {
  return {
    status: "available",
    currentVersion: "0.1.0",
    version: "0.2.0",
    notes: "A safer project library.",
    date: "2026-09-01T08:00:00Z",
    downloadedBytes: 0,
    restartRequired: false,
    installed: false,
    ...overrides,
  };
}

function renderEntry(nextState: UpdateState = state(), overrides: Partial<ComponentProps<typeof TitleBarUpdate>> = {}) {
  return render(
    <TitleBarUpdate
      state={nextState}
      t={t}
      onCheck={vi.fn(async () => nextState)}
      onDownload={vi.fn(async () => nextState)}
      onInstall={vi.fn(async () => true)}
      onRestart={vi.fn(async () => undefined)}
      onDefer={vi.fn(async () => undefined)}
      {...overrides}
    />,
  );
}

describe("TitleBarUpdate", () => {
  it("stays hidden when no update is in play", () => {
    const { container } = renderEntry(state({ status: "idle", version: undefined, notes: undefined, date: undefined }));
    expect(container).toBeEmptyDOMElement();
  });

  it("keeps a failed check out of the title bar", () => {
    const { container } = renderEntry(state({ status: "error", errorStage: "check", error: "offline" }));
    expect(container).toBeEmptyDOMElement();
  });

  it("shows the changelog and manual download for an available update", async () => {
    const onDownload = vi.fn(async () => state());
    renderEntry(state(), { onDownload });

    fireEvent.click(screen.getByRole("button", { name: "Update available · v0.2.0" }));
    expect(screen.getByText("A safer project library.")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Download update" }));
    await waitFor(() => expect(onDownload).toHaveBeenCalledOnce());
  });

  it("rechecks for updates from the panel", async () => {
    const onCheck = vi.fn(async () => state());
    renderEntry(state(), { onCheck });

    fireEvent.click(screen.getByRole("button", { name: "Update available · v0.2.0" }));
    fireEvent.click(screen.getByRole("button", { name: "Check for updates" }));
    await waitFor(() => expect(onCheck).toHaveBeenCalledOnce());
  });

  it("requires confirmation before installing from the panel", async () => {
    const onInstall = vi.fn(async () => true);
    renderEntry(state({ status: "ready", version: "0.2.0" }), { onInstall });

    fireEvent.click(screen.getByRole("button", { name: "Update downloaded" }));
    fireEvent.click(screen.getByRole("button", { name: "Install update" }));
    expect(screen.getByText("Confirm update installation")).toBeInTheDocument();
    expect(onInstall).not.toHaveBeenCalled();

    const installButtons = screen.getAllByRole("button", { name: "Install update" });
    fireEvent.click(installButtons[installButtons.length - 1]);
    await waitFor(() => expect(onInstall).toHaveBeenCalledOnce());
  });

  it("retries a failed download from the panel", async () => {
    const onDownload = vi.fn(async () => state({ status: "error", errorStage: "download" }));
    renderEntry(state({ status: "error", errorStage: "download", error: "network reset", version: "0.2.0" }), { onDownload });

    fireEvent.click(screen.getByRole("button", { name: "Could not download update" }));
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await waitFor(() => expect(onDownload).toHaveBeenCalledOnce());
  });
});
