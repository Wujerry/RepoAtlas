import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { LIST_PANE_DEFAULT, LIST_PANE_MAX, LIST_PANE_MIN, PaneResizer, clampListWidth, readStoredListWidth } from "../components/ui/pane-resizer";

function renderResizer() {
  const onWidthChange = vi.fn();
  const onWidthCommit = vi.fn();
  render(
    <div>
      <PaneResizer width={360} label="Resize sidebar" onWidthChange={onWidthChange} onWidthCommit={onWidthCommit} />
    </div>,
  );
  const resizer = screen.getByRole("separator", { name: "Resize sidebar" });
  return { onWidthChange, onWidthCommit, resizer };
}

describe("PaneResizer", () => {
  it("clamps widths to the supported range", () => {
    expect(clampListWidth(120)).toBe(LIST_PANE_MIN);
    expect(clampListWidth(9999)).toBe(LIST_PANE_MAX);
    expect(clampListWidth(Number.NaN)).toBe(LIST_PANE_DEFAULT);
    expect(clampListWidth(432)).toBe(432);
  });

  it("reads and normalizes the stored width", () => {
    expect(readStoredListWidth({ getItem: () => "480" })).toBe(480);
    expect(readStoredListWidth({ getItem: () => "10" })).toBe(LIST_PANE_MIN);
    expect(readStoredListWidth({ getItem: () => null })).toBe(LIST_PANE_DEFAULT);
    expect(readStoredListWidth({ getItem: () => { throw new Error("unavailable"); } })).toBe(LIST_PANE_DEFAULT);
  });

  it("resizes while dragging and commits on pointer up", () => {
    const { onWidthChange, onWidthCommit, resizer } = renderResizer();
    fireEvent.pointerDown(resizer, { button: 0, clientX: 360 });
    window.dispatchEvent(new MouseEvent("pointermove", { clientX: 460 }));
    expect(onWidthChange).toHaveBeenLastCalledWith(460);
    window.dispatchEvent(new MouseEvent("pointerup", { clientX: 460 }));
    expect(onWidthCommit).toHaveBeenLastCalledWith(460);
  });

  it("adjusts with arrow keys", () => {
    const { onWidthChange, onWidthCommit, resizer } = renderResizer();
    fireEvent.keyDown(resizer, { key: "ArrowLeft" });
    expect(onWidthChange).toHaveBeenLastCalledWith(344);
    expect(onWidthCommit).toHaveBeenLastCalledWith(344);
    fireEvent.keyDown(resizer, { key: "ArrowRight" });
    expect(onWidthChange).toHaveBeenLastCalledWith(376);
    expect(onWidthCommit).toHaveBeenLastCalledWith(376);
  });

  it("resets to the default width on double click", () => {
    const { onWidthChange, onWidthCommit, resizer } = renderResizer();
    fireEvent.dblClick(resizer);
    expect(onWidthChange).toHaveBeenLastCalledWith(LIST_PANE_DEFAULT);
    expect(onWidthCommit).toHaveBeenLastCalledWith(LIST_PANE_DEFAULT);
  });
  it("commits and clears the drag overlay on pointercancel", () => {
    const { onWidthCommit, resizer } = renderResizer();
    fireEvent.pointerDown(resizer, { button: 0, clientX: 360, pointerId: 1 });
    window.dispatchEvent(new MouseEvent("pointercancel", { clientX: 500 }));
    expect(onWidthCommit).toHaveBeenCalled();
    expect(document.documentElement.classList.contains("is-pane-resizing")).toBe(false);
  });

});
