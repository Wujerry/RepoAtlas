import { fireEvent, render, screen } from "@testing-library/react";
import { Suspense } from "react";
import { describe, expect, it, vi } from "vitest";
import { dictionaries, type MessageKey } from "../i18n";
import type { UpdateState } from "../types";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));
vi.mock("../lib/api", () => ({ api: {} }));
vi.mock("../components/UpdateSettings", () => ({ UpdateSettings: () => <section>Update settings</section> }));

import { SettingsPane } from "../components/SettingsPane";

const t = (key: MessageKey) => dictionaries.en[key];
const updateState: UpdateState = { status: "idle", currentVersion: "0.1.0", downloadedBytes: 0, restartRequired: false };

describe("SettingsPane", () => {
  it("offers an explicit route back to the project library", async () => {
    const onBack = vi.fn();
    render(<Suspense fallback={null}><SettingsPane
      settings={{ theme: "system", locale: "en", uiFont: "", consoleFont: "" }}
      scanRoots={[]}
      scanning={false}
      t={t}
      notify={vi.fn()}
      onSettings={vi.fn(async () => undefined)}
      onAddRoot={vi.fn(async () => undefined)}
      onRemoveRoot={vi.fn(async () => undefined)}
      onScanRoot={vi.fn(async () => undefined)}
      onReload={vi.fn(async () => undefined)}
      updateState={updateState}
      onCheckForUpdates={vi.fn(async () => updateState)}
      onDownloadUpdate={vi.fn(async () => updateState)}
      onInstallUpdate={vi.fn(async () => true)}
      onRestartApp={vi.fn(async () => undefined)}
      onDeferUpdate={vi.fn(async () => undefined)}
      onBack={onBack}
    /></Suspense>);

    fireEvent.click(await screen.findByRole("button", { name: t("backToProjects") }));
    expect(onBack).toHaveBeenCalledOnce();
    expect(screen.getByRole("button", { name: t("backToProjects") })).toHaveAttribute("type", "button");
  });
});
