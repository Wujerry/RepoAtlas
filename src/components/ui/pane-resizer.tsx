import { useEffect, useRef, type KeyboardEvent, type PointerEvent as ReactPointerEvent } from "react";

export const LIST_PANE_MIN = 280;
export const LIST_PANE_MAX = 560;
export const LIST_PANE_DEFAULT = 360;
export const LIST_PANE_STORAGE_KEY = "repoatlas.sidebarWidth";

export function clampListWidth(width: number, min = LIST_PANE_MIN, max = LIST_PANE_MAX): number {
  if (!Number.isFinite(width)) return LIST_PANE_DEFAULT;
  return Math.round(Math.min(max, Math.max(min, width)));
}

export function readStoredListWidth(storage: Pick<Storage, "getItem"> | undefined): number {
  try {
    const stored = storage?.getItem(LIST_PANE_STORAGE_KEY);
    if (stored === null || stored === undefined || stored === "") return LIST_PANE_DEFAULT;
    return clampListWidth(Number(stored));
  } catch {
    return LIST_PANE_DEFAULT;
  }
}

type PaneResizerProps = {
  width: number;
  label: string;
  onWidthChange: (width: number) => void;
  onWidthCommit: (width: number) => void;
};

export function PaneResizer({ width, label, onWidthChange, onWidthCommit }: PaneResizerProps) {
  const draggingRef = useRef(false);
  const originRef = useRef(0);

  useEffect(() => {
    return () => {
      draggingRef.current = false;
      document.documentElement.classList.remove("is-pane-resizing");
    };
  }, []);

  const handlePointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    event.preventDefault();
    const target = event.currentTarget;
    const workspace = target.parentElement;
    if (!workspace) return;
    draggingRef.current = true;
    originRef.current = workspace.getBoundingClientRect().left;
    document.documentElement.classList.add("is-pane-resizing");
    target.setPointerCapture?.(event.pointerId);
    const onMove = (move: PointerEvent) => {
      if (!draggingRef.current) return;
      onWidthChange(clampListWidth(move.clientX - originRef.current));
    };
    const finish = (up: PointerEvent) => {
      if (!draggingRef.current) return;
      draggingRef.current = false;
      document.documentElement.classList.remove("is-pane-resizing");
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", finish);
      window.removeEventListener("pointercancel", finish);
      window.removeEventListener("blur", onBlur);
      target.removeEventListener("lostpointercapture", onLostCapture);
      if (typeof target.hasPointerCapture === "function" && target.hasPointerCapture(up.pointerId)) target.releasePointerCapture(up.pointerId);
      onWidthCommit(clampListWidth(up.clientX - originRef.current));
    };
    const onBlur = () => finish(new PointerEvent("pointercancel", { clientX: originRef.current + width }));
    const onLostCapture = (lost: Event) => finish(lost as PointerEvent);
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", finish);
    window.addEventListener("pointercancel", finish);
    window.addEventListener("blur", onBlur);
    target.addEventListener("lostpointercapture", onLostCapture);
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    const delta = event.key === "ArrowLeft" ? -16 : 16;
    const next = clampListWidth(width + delta);
    onWidthChange(next);
    onWidthCommit(next);
  };

  return (
    <div
      role="separator"
      className="pane-resizer"
      aria-orientation="vertical"
      aria-label={label}
      aria-valuemin={LIST_PANE_MIN}
      aria-valuemax={LIST_PANE_MAX}
      aria-valuenow={width}
      tabIndex={0}
      onPointerDown={handlePointerDown}
      onDoubleClick={() => {
        onWidthChange(LIST_PANE_DEFAULT);
        onWidthCommit(LIST_PANE_DEFAULT);
      }}
      onKeyDown={handleKeyDown}
    />
  );
}
