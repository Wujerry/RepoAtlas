import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { SessionAgentLabel, SessionAgentSelect } from "../components/ui/session-agent";
import { sessionAgents } from "../lib/sessions";

it("renders a decorative brand glyph and readable name for every session adapter", () => {
  const { container } = render(<>{Object.keys(sessionAgents).map(adapter => <SessionAgentLabel key={adapter} adapter={adapter} />)}</>);
  for (const name of Object.values(sessionAgents)) {
    const label = screen.getByText(name).closest(".session-agent-label")!;
    expect(label.querySelector('svg[aria-hidden="true"]')).not.toBeNull();
    expect(label.querySelector(".brand-glyph-fallback")).toBeNull();
  }
  expect(container.querySelectorAll(".session-agent-label")).toHaveLength(8);
});

it("keeps Agent icons in filter options and sends the selected adapter", async () => {
  const onChange = vi.fn();
  render(<SessionAgentSelect value="" onChange={onChange} label="Agent filter" allLabel="All agents" />);
  fireEvent.click(screen.getByRole("combobox", { name: "Agent filter" }));
  const option = await screen.findByRole("option", { name: "Cursor CLI" });
  expect(option.querySelector('svg[aria-hidden="true"]')).not.toBeNull();
  fireEvent.pointerDown(option, { pointerType: "mouse" });
  fireEvent.click(option);
  await waitFor(() => expect(onChange).toHaveBeenCalledWith("cursor-cli"));
});

it("shows the selected Agent with its icon and preserves App-specific naming", () => {
  render(<><SessionAgentSelect value="copilot" onChange={() => {}} label="Source Agent" /><SessionAgentLabel adapter="codex" name="Codex App" /></>);
  const trigger = screen.getByRole("combobox", { name: "Source Agent" });
  expect(within(trigger).getByText("GitHub Copilot CLI")).toBeInTheDocument();
  expect(trigger.querySelector(".session-agent-label svg")).not.toBeNull();
  expect(screen.getByText("Codex App").closest(".session-agent-label")?.querySelector("svg")).not.toBeNull();
});
