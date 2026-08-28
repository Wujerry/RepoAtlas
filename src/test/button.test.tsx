import { render, screen } from "@testing-library/react";
import { Button } from "../components/ui/button";
import { describe, expect, it } from "vitest";

describe("Button", () => {
  it("marks loading buttons busy and disabled while retaining their label", () => {
    render(<Button loading>Save</Button>);

    const button = screen.getByRole("button", { name: "Save" });
    expect(button).toBeDisabled();
    expect(button).toHaveAttribute("aria-busy", "true");
    expect(button.querySelector(".button-spinner")).toBeInTheDocument();
  });

  it("keeps explicitly disabled buttons disabled without a loading state", () => {
    render(<Button disabled>Remove</Button>);

    const button = screen.getByRole("button", { name: "Remove" });
    expect(button).toBeDisabled();
    expect(button).not.toHaveAttribute("aria-busy");
  });
});
