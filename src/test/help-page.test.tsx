import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { HelpPage } from "../components/HelpPage";
import { dictionaries, type MessageKey } from "../i18n";

vi.mock("../lib/api", () => ({
  api: {
    mcpSetupInfo: vi.fn(async () => ({
      dbPath: "C:\\Users\\dev\\repoatlas.sqlite",
      platform: "windows",
      binaryName: "repoatlas-mcp.exe",
      binaryPath: "C:\\Program Files\\RepoAtlas\\repoatlas-mcp.exe",
      workspacePath: null,
    })),
  },
}));

const t = (key: MessageKey) => dictionaries.en[key];

describe("HelpPage first-launch replay", () => {
  it("reopens onboarding from help and stays disabled during a scan", () => {
    const onReplayOnboarding = vi.fn();
    const { rerender } = render(
      <HelpPage locale="en" t={t} notify={vi.fn()} onReplayOnboarding={onReplayOnboarding} />,
    );
    fireEvent.click(screen.getByRole("button", { name: t("onboardingReplay") }));
    expect(onReplayOnboarding).toHaveBeenCalledTimes(1);

    rerender(<HelpPage locale="en" t={t} notify={vi.fn()} scanning onReplayOnboarding={onReplayOnboarding} />);
    expect(screen.getByRole("button", { name: t("onboardingReplay") })).toBeDisabled();
    expect(screen.getByText(t("onboardingReplayBusy"))).toBeInTheDocument();
  });
});
