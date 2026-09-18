import { Check } from "@phosphor-icons/react";
import { defaultRangeExtractor, useVirtualizer } from "@tanstack/react-virtual";
import { useLayoutEffect, useRef, useState } from "react";

/** One Tab stop; arrows move through all rows, including virtualized rows. */
export function SelectionList({ items, selected, onToggle, disabled, label, hint, emptyMessage }: {
  items: { id: string; name: string; detail: string }[];
  selected: Set<string>;
  onToggle: (id: string) => void;
  disabled: boolean;
  label: string;
  hint: string;
  emptyMessage: string;
}) {
  const scroll = useRef<HTMLDivElement>(null);
  const focusRequested = useRef(false);
  const [activeId, setActiveId] = useState<string>();
  const found = items.findIndex((item) => item.id === activeId);
  const activeIndex = Math.max(0, found);
  const virtual = items.length > 100;
  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => scroll.current,
    estimateSize: () => 58,
    getItemKey: (index) => items[index].id,
    overscan: 5,
    initialRect: { width: 500, height: 330 },
    rangeExtractor: (range) => [...new Set([...defaultRangeExtractor(range), activeIndex])].sort((a, b) => a - b),
  });
  useLayoutEffect(() => {
    if (!focusRequested.current) return;
    const row = scroll.current?.querySelector<HTMLButtonElement>(`[data-index="${activeIndex}"]`);
    (row ?? scroll.current)?.focus({ preventScroll: true });
    if (!virtual) row?.scrollIntoView({ block: "nearest" });
    focusRequested.current = false;
  }, [activeId, activeIndex, virtual, items]);
  const rows = virtual ? virtualizer.getVirtualItems().map((row) => ({ index: row.index, start: row.start })) : items.map((_, index) => ({ index, start: 0 }));
  if (virtual && !rows.some((row) => row.index === activeIndex)) rows.push({ index: activeIndex, start: activeIndex * 58 });
  return <div ref={scroll} tabIndex={-1} className="selection-list" role="group" aria-label={label} aria-description={hint}>
    {!items.length && <p className="selection-empty" role="status">{emptyMessage}</p>}
    <div className={virtual ? "selection-list-inner is-virtual" : "selection-list-inner"} style={virtual ? { height: virtualizer.getTotalSize() } : undefined}>
      {rows.map(({ index, start }) => {
        const item = items[index];
        return <button type="button" role="checkbox" aria-label={`${item.name} ${item.detail}`} aria-checked={selected.has(item.id)} disabled={disabled} key={item.id} data-index={index} tabIndex={index === activeIndex ? 0 : -1}
          className={`selection-row${selected.has(item.id) ? " is-selected" : ""}`} style={virtual ? { position: "absolute", transform: `translateY(${start}px)` } : undefined}
          onFocus={() => setActiveId(item.id)} onClick={() => { focusRequested.current = true; onToggle(item.id); }}
          onKeyDown={(event) => {
            if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
            event.preventDefault();
            const next = event.key === "Home" ? 0 : event.key === "End" ? items.length - 1 : Math.min(items.length - 1, Math.max(0, index + (event.key === "ArrowDown" ? 1 : -1)));
            if (next === index) return;
            focusRequested.current = true;
            if (virtual) virtualizer.scrollToIndex(next, { align: "auto" });
            setActiveId(items[next].id);
          }}>
          <span className="selection-check" aria-hidden="true">{selected.has(item.id) && <Check weight="bold" />}</span>
          <span className="selection-copy"><strong title={item.name}>{item.name}</strong><code title={item.detail}>{item.detail}</code></span>
        </button>;
      })}
    </div>
  </div>;
}
