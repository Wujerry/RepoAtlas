import { fireEvent, render, screen } from "@testing-library/react";
import type { ComponentProps } from "react";
import { describe, expect, it, vi } from "vitest";
import { TitleBar } from "../components/TitleBar";

vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({}) }));

const props: ComponentProps<typeof TitleBar> = {
  title: "RepoAtlas", subtitle: "Projects", commandLabel: "Search",
  minimizeLabel: "Minimize", maximizeLabel: "Maximize", closeLabel: "Close",
  helpLabel: "Help", settingsLabel: "Settings", approvalsLabel: "Attention",
  approvalCount: 0, footprintsLabel: "Footprints", tasksLabel: "Tasks",
  tasksShortcut: "Ctrl + `", tasksCount: 0, portsLabel: "Ports",
  activeView: "library", onCommand: vi.fn(), onLibrary: vi.fn(), onHelp: vi.fn(),
  onSettings: vi.fn(), onApprovals: vi.fn(), onTasks: vi.fn(),
};

describe("title-bar navigation feedback", () => {
  it.each([
    ["Sessions", { historyOpen: true }],
    ["Tasks", { tasksOpen: true }],
    ["Ports", { portsOpen: true }],
    ["Footprints", { footprintsOpen: true }],
    ["Attention", { attentionOpen: true }],
    ["Help", { activeView: "help" as const }],
    ["Settings", { activeView: "settings" as const }],
    ["Usage", { usageOpen: true }],
  ])("tracks %s selection and clears it when leaving", (name, selection) => {
    const view = render(<TitleBar {...props} />);
    const button = screen.getByRole("button", { name });
    const icon = button.querySelector("svg");
    expect(icon).toHaveAttribute("data-active", "false");
    view.rerender(<TitleBar {...props} {...selection} />);
    expect(button).toHaveAttribute("aria-pressed", "true");
    expect(icon).toHaveAttribute("data-active", "true");
    expect(icon).toHaveAttribute("aria-hidden", "true");
    expect(icon).toHaveAttribute("focusable", "false");
    view.rerender(<TitleBar {...props} />);
    expect(button).toHaveAttribute("aria-pressed", "false");
    expect(icon).toHaveAttribute("data-active", "false");
  });

  it("retains the selected SVG during background count updates", () => {
    const view = render(<TitleBar {...props} tasksOpen attentionOpen />);
    const task = screen.getByRole("button", { name: "Tasks" }).querySelector("svg");
    const attention = screen.getByRole("button", { name: "Attention" }).querySelector("svg");
    view.rerender(<TitleBar {...props} tasksOpen attentionOpen tasksCount={3} approvalCount={2} />);
    expect(screen.getByRole("button", { name: "Tasks (3)" }).querySelector("svg")).toBe(task);
    expect(screen.getByRole("button", { name: "Attention (2)" }).querySelector("svg")).toBe(attention);
    expect(task).toHaveAttribute("data-active", "true");
    expect(attention).toHaveAttribute("data-active", "true");
  });

  it("keeps activation on the existing accessible button", () => {
    const onSettings = vi.fn();
    render(<TitleBar {...props} onSettings={onSettings} />);
    const button = screen.getByRole("button", { name: "Settings" });
    button.focus();
    expect(button).toHaveFocus();
    fireEvent.click(button);
    expect(onSettings).toHaveBeenCalledOnce();
  });
});
