import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { dictionaries, type MessageKey } from "../i18n";
import type { ProviderPreset, ProviderProfile } from "../types";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { listProviderProfiles, providerPresets, upsertProviderProfile, deleteProviderProfile } = vi.hoisted(() => ({
  listProviderProfiles: vi.fn(),
  providerPresets: vi.fn(),
  upsertProviderProfile: vi.fn(),
  deleteProviderProfile: vi.fn(),
}));

vi.mock("../lib/api", () => ({
  api: {
    listProviderProfiles,
    providerPresets,
    upsertProviderProfile,
    deleteProviderProfile,
  },
}));

import { AiSettings } from "../components/AiSettings";

const t = (key: MessageKey) => dictionaries.en[key];

const profile: ProviderProfile = {
  id: "provider-1",
  name: "DeepSeek",
  protocol: "openai-compatible",
  baseUrl: "https://api.deepseek.com/v1",
  model: "deepseek-chat",
  credentialRef: "DEEPSEEK_API_KEY",
  createdAt: "2026-08-21T00:00:00Z",
  updatedAt: "2026-08-21T00:00:00Z",
};

function renderSettings(profiles: ProviderProfile[] = [], presets: ProviderPreset[] = []) {
  listProviderProfiles.mockResolvedValue(profiles);
  providerPresets.mockResolvedValue(presets);
  upsertProviderProfile.mockResolvedValue(profile);
  deleteProviderProfile.mockResolvedValue(undefined);
  return render(<AiSettings t={t} />);
}

describe("AiSettings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("rejects an empty provider form before calling the API", async () => {
    renderSettings();
    await screen.findByRole("button", { name: "Save" });

    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(/Name, protocol, model/);
    expect(upsertProviderProfile).not.toHaveBeenCalled();
  });

  it("uses a password field for the credential reference", async () => {
    renderSettings();
    const field = await screen.findByLabelText(/^Credential environment variable/);
    expect(field).toHaveAttribute("type", "password");
  });

  it("requires a valid environment variable reference", async () => {
    renderSettings();
    await screen.findByRole("button", { name: "Save" });

    fireEvent.change(screen.getByLabelText(/^Name/), { target: { value: "Local model" } });
    fireEvent.change(screen.getByLabelText(/^Model/), { target: { value: "llama3" } });
    fireEvent.change(screen.getByLabelText(/^Credential environment variable/), { target: { value: "not valid" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(/valid environment variable name/);
    expect(upsertProviderProfile).not.toHaveBeenCalled();
  });

  it.each([
    {
      name: "Ollama",
      protocol: "ollama",
      baseUrl: "http://localhost:11434",
      defaultModel: "llama3.2",
    },
    {
      name: "LM Studio",
      protocol: "openai-compatible",
      baseUrl: "http://localhost:1234/v1",
      defaultModel: "local-model",
    },
  ])("allows $name without a credential reference", async (preset) => {
    renderSettings([], [{ ...preset, credentialRef: "" }]);

    fireEvent.click(await screen.findByRole("button", { name: preset.name }));

    expect(screen.getByLabelText(/^Credential environment variable/)).toHaveValue("");
    expect(screen.getByText(/does not require an API key/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(upsertProviderProfile).toHaveBeenCalledWith(expect.objectContaining({
      name: preset.name,
      protocol: preset.protocol,
      baseUrl: preset.baseUrl,
      model: preset.defaultModel,
      credentialRef: "",
    })));
  });

  it("starts a fresh provider when applying a preset from an edited profile", async () => {
    const preset: ProviderPreset = {
      name: "OpenCode Go",
      protocol: "openai-compatible",
      baseUrl: "https://opencode.ai/zen/go/v1",
      defaultModel: "deepseek-v4-pro",
      credentialRef: "OPENCODE_API_KEY",
    };
    renderSettings([profile], [preset]);

    const profileRow = await screen.findByText(profile.name);
    fireEvent.click(within(profileRow.closest("li") as HTMLElement).getByRole("button", { name: t("edit") }));
    fireEvent.click(await screen.findByRole("button", { name: preset.name }));
    fireEvent.click(screen.getByRole("button", { name: t("save") }));

    await waitFor(() => expect(upsertProviderProfile).toHaveBeenCalledWith(expect.objectContaining({
      id: null,
      name: preset.name,
      model: preset.defaultModel,
    })));
  });

  it("keeps provider loading failures separate and offers retry", async () => {
    listProviderProfiles.mockRejectedValueOnce(new Error("database unavailable")).mockResolvedValueOnce([]);
    providerPresets.mockResolvedValue([]);
    render(<AiSettings t={t} />);

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("AI providers unavailable");
    expect(alert).toHaveTextContent("database unavailable");
    expect(screen.queryByText("No providers configured")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Retry/ }));
    await waitFor(() => expect(listProviderProfiles).toHaveBeenCalledTimes(2));
    expect(await screen.findByText("No providers configured")).toBeInTheDocument();
  });

  it("requires confirmation before deleting a provider", async () => {
    renderSettings([profile]);
    const row = await screen.findByText("DeepSeek");
    fireEvent.click(within(row.closest("li") as HTMLElement).getByRole("button", { name: "Remove" }));

    const dialog = await screen.findByRole("alertdialog");
    expect(dialog).toHaveTextContent("Remove provider");
    expect(deleteProviderProfile).not.toHaveBeenCalled();

    fireEvent.click(within(dialog).getByRole("button", { name: "Remove" }));
    await waitFor(() => expect(deleteProviderProfile).toHaveBeenCalledWith(profile.id));
  });
});
