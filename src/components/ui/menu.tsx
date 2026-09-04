import { ContextMenu } from "@base-ui/react/context-menu";
import { Menu } from "@base-ui/react/menu";
import type { ReactElement } from "react";
import type { ReactNode } from "react";
import { cn } from "../../lib/cn";

export interface MenuAction {
  label: string;
  onClick: () => void;
  danger?: boolean;
  disabled?: boolean;
  icon?: ReactNode;
}

export function ActionMenu({
  trigger,
  items,
}: {
  trigger: ReactElement;
  items: MenuAction[];
}) {
  return (
    <Menu.Root>
      <Menu.Trigger render={trigger} />
      <Menu.Portal>
        <Menu.Positioner sideOffset={6} className="menu-positioner">
          <Menu.Popup className="menu-popup">
            {items.map((item) => (
              <Menu.Item
                key={item.label}
                onClick={item.onClick}
                disabled={item.disabled}
                className={cn(
                  "menu-item",
                  item.danger && "menu-item-danger",
                )}
              >
                {item.icon ? <span className="menu-item-icon" aria-hidden="true">{item.icon}</span> : null}
                {item.label}
              </Menu.Item>
            ))}
          </Menu.Popup>
        </Menu.Positioner>
      </Menu.Portal>
    </Menu.Root>
  );
}

export function ItemContextMenu({
  trigger,
  items,
}: {
  trigger: ReactElement;
  items: MenuAction[];
}) {
  return (
    <ContextMenu.Root>
      <ContextMenu.Trigger data-repoatlas-context-menu="true" render={trigger} />
      <ContextMenu.Portal>
        <ContextMenu.Positioner sideOffset={4} className="menu-positioner">
          <ContextMenu.Popup className="menu-popup">
            {items.map((item) => (
              <ContextMenu.Item
                key={item.label}
                onClick={item.onClick}
                className={cn(
                  "menu-item",
                  item.danger && "menu-item-danger",
                )}
              >
                {item.icon ? <span className="menu-item-icon" aria-hidden="true">{item.icon}</span> : null}
                {item.label}
              </ContextMenu.Item>
            ))}
          </ContextMenu.Popup>
        </ContextMenu.Positioner>
      </ContextMenu.Portal>
    </ContextMenu.Root>
  );
}
