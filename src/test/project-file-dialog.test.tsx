import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { dictionaries, type MessageKey } from "../i18n";
import type { ReadmeDocument } from "../types";

const { readProjectFile } = vi.hoisted(() => ({ readProjectFile: vi.fn() }));
vi.mock("../lib/api", () => ({ api: { readProjectFile } }));
// Layout/virtualization is exercised in the shared preview and desktop checks.
vi.mock("../components/CodePreview", () => ({ CodePreview: ({ code, language }: { code: string; language: string }) => <pre data-language={language}>{code}</pre> }));
import { ProjectFileDialog, evidenceLanguage } from "../components/ProjectFileDialog";

const t = (key: MessageKey) => dictionaries.en[key];
const notify = vi.fn();
const file = { path: "package.json", kind: "manifest", source: "package.json#engines.node" };
const props = { projectId: "atlas", file, t, notify, onClose: vi.fn() };

beforeEach(() => { vi.clearAllMocks(); });

describe("project evidence preview", () => {
  it("retries a failed read and enables content tools only after success", async () => {
    readProjectFile.mockRejectedValueOnce(new Error("read failed")).mockResolvedValueOnce({ path: file.path, content: "{}", truncated: true });
    render(<ProjectFileDialog {...props} />);
    expect(await screen.findByRole("alert")).toHaveTextContent("read failed");
    expect(screen.getByRole("button", { name: t("copyFileContent") })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: t("retry") }));
    expect(await screen.findByText("{}")).toHaveAttribute("data-language", "json");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByText(t("fileTruncated"))).toBeInTheDocument();
    expect(screen.getByRole("button", { name: t("copyFileContent") })).toBeEnabled();
    fireEvent.click(screen.getByRole("button", { name: t("expandPreview") }));
    expect(screen.getByRole("dialog")).toHaveClass("is-expanded");
    fireEvent.click(screen.getByRole("button", { name: t("restorePreview") }));
    expect(screen.getByRole("dialog")).not.toHaveClass("is-expanded");
  });

  it("ignores a late read after selecting another file", async () => {
    let resolveFirst!: (value: ReadmeDocument) => void;
    readProjectFile.mockReturnValueOnce(new Promise<ReadmeDocument>((resolve) => { resolveFirst = resolve; })).mockResolvedValueOnce({ path: "Cargo.toml", content: "[package]", truncated: false });
    const { rerender } = render(<ProjectFileDialog {...props} />);
    rerender(<ProjectFileDialog {...props} file={{ ...file, path: "Cargo.toml", source: "Cargo.toml" }} />);
    expect(await screen.findByText("[package]")).toHaveAttribute("data-language", "toml");
    await act(async () => resolveFirst({ path: file.path, content: "old result", truncated: false }));
    expect(screen.queryByText("old result")).not.toBeInTheDocument();
  });

  it("copies the loaded content and reports clipboard failures", async () => {
    const writeText = vi.fn().mockResolvedValueOnce(undefined).mockRejectedValueOnce(new Error("clipboard denied"));
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
    readProjectFile.mockResolvedValue({ path: file.path, content: "{}", truncated: false });
    render(<ProjectFileDialog {...props} />);
    await screen.findByText("{}");
    fireEvent.click(screen.getByRole("button", { name: t("copyFileContent") }));
    await waitFor(() => expect(notify).toHaveBeenCalledWith("success", t("copied")));
    expect(writeText).toHaveBeenCalledWith("{}");
    fireEvent.click(screen.getByRole("button", { name: t("copyPath") }));
    await waitFor(() => expect(notify).toHaveBeenCalledWith("error", t("fileActionFailed"), "Error: clipboard denied"));
  });

  it("recognizes supported evidence formats and keeps unknown files as text", () => {
    expect(evidenceLanguage("nested\\Cargo.lock")).toBe("toml");
    expect(evidenceLanguage("pubspec.yaml")).toBe("yaml");
    expect(evidenceLanguage(".nvmrc")).toBe("text");
  });
});
