import { Popover } from "@base-ui/react/popover";
import { X } from "@phosphor-icons/react";
import type { ReactNode, Ref } from "react";
import { Button } from "./button";

/** Portal details without changing the reader's geometry or scroll position. */
export function DetailPopover({ title, closeLabel, children, triggerRef }: {
  title: string; closeLabel: string; children: ReactNode; triggerRef?: Ref<HTMLButtonElement>;
}) {
  return <Popover.Root>
    <Popover.Trigger ref={triggerRef} className="button button-quiet button-sm detail-popover-trigger">{title}</Popover.Trigger>
    <Popover.Portal><Popover.Positioner side="bottom" align="end" sideOffset={8} className="detail-popover-positioner">
      <Popover.Popup className="detail-popover">
        <header><Popover.Title>{title}</Popover.Title><Popover.Close render={<Button size="icon" variant="quiet" aria-label={closeLabel}/>}><X aria-hidden/></Popover.Close></header>
        <div className="detail-popover-body">{children}</div>
      </Popover.Popup>
    </Popover.Positioner></Popover.Portal>
  </Popover.Root>;
}
