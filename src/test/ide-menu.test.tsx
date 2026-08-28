import { fireEvent, render, screen } from "@testing-library/react";
import { dictionaries, type MessageKey } from "../i18n";
import { describe, expect, it, vi } from "vitest";
import { ActionMenu } from "../components/ui/menu";
import { IdeGlyph } from "../lib/project-identity";
import { Button } from "../components/ui/button";

const t = (key: MessageKey) => dictionaries.en[key];

describe("IDE menu", () => {
  it("renders decorative IDE icons without changing accessible names", async () => {
    const onOpen = vi.fn();
    render(
      <ActionMenu
        trigger={<Button>{t("openIde")}</Button>}
        items={[
          { label: "Cursor", icon: <IdeGlyph ide="cursor" />, onClick: () => onOpen("cursor") },
          { label: "VS Code", icon: <IdeGlyph ide="vscode" />, onClick: () => onOpen("vscode") },
        ]}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: t("openIde") }));
    const cursor = await screen.findByRole("menuitem", { name: "Cursor" });
    expect(cursor).toHaveAccessibleName("Cursor");
    expect(cursor.querySelector(".menu-item-icon")).toHaveAttribute("aria-hidden", "true");
    fireEvent.click(cursor);
    expect(onOpen).toHaveBeenCalledWith("cursor");
  });
});
