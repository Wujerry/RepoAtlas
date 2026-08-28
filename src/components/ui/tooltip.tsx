import { Tooltip } from "@base-ui/react/tooltip";
import type { ReactElement } from "react";

export function Hint({ label, children }: { label: string; children: ReactElement }) {
  return (
    <Tooltip.Root>
      <Tooltip.Trigger render={children} />
      <Tooltip.Portal>
        <Tooltip.Positioner side="bottom" sideOffset={6}>
          <Tooltip.Popup className="tooltip-popup">
            {label}
          </Tooltip.Popup>
        </Tooltip.Positioner>
      </Tooltip.Portal>
    </Tooltip.Root>
  );
}
