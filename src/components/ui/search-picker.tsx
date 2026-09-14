import { Popover } from "@base-ui/react/popover";
import { FolderOpen } from "@phosphor-icons/react";
import type { ReactNode } from "react";
import { useMemo, useState } from "react";

/** A bounded searchable picker with Escape dismissal and trigger-focus restoration. */
export function SearchPicker({ items, value, onChange, allLabel, searchLabel, emptyLabel, renderMark }: {
  items: { id: string; name: string }[]; value: string; onChange: (value: string) => void;
  allLabel: string; searchLabel: string; emptyLabel: string;
  renderMark?: (id: string) => ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState("");
  const filtered = useMemo(() => items.filter(item => item.name.toLocaleLowerCase().includes(search.toLocaleLowerCase())).slice(0, 100), [items, search]);
  function choose(id: string) { onChange(id); setOpen(false); }
  return <Popover.Root open={open} onOpenChange={next => { setOpen(next); if (!next) setSearch(""); }}>
    <Popover.Trigger className="fp-project-trigger">{value && renderMark ? renderMark(value) : <FolderOpen />}<span>{items.find(item => item.id === value)?.name ?? allLabel}</span></Popover.Trigger>
    <Popover.Portal><Popover.Positioner sideOffset={6} align="end" className="fp-picker-positioner">
      <Popover.Popup className="fp-picker-popup">
        <Popover.Title className="sr-only">{searchLabel}</Popover.Title>
        <input aria-label={searchLabel} placeholder={searchLabel} value={search} onChange={e => setSearch(e.target.value)} />
        <div className="fp-picker-options" onKeyDown={e => {
          if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
          const buttons = Array.from(e.currentTarget.querySelectorAll("button"));
          const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
          buttons[Math.max(0, Math.min(buttons.length - 1, index + (e.key === "ArrowDown" ? 1 : -1)))]?.focus(); e.preventDefault();
        }}>
          <button onClick={() => choose("")} aria-pressed={!value}>{renderMark && <span className="fp-picker-mark-space" aria-hidden="true" />}{allLabel}</button>
          {filtered.map(item => <button key={item.id} aria-pressed={item.id === value} onClick={() => choose(item.id)}>{renderMark?.(item.id)}<span>{item.name}</span></button>)}
          {!filtered.length && <p>{emptyLabel}</p>}
        </div>
      </Popover.Popup>
    </Popover.Positioner></Popover.Portal>
  </Popover.Root>;
}
