import { render, screen } from "@testing-library/react";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { WorkspaceSkeleton } from "../components/ui/feedback";

describe("WorkspaceSkeleton", () => {
  it("spans the full desktop workspace instead of occupying the list column", () => {
    const { container } = render(<WorkspaceSkeleton />);
    expect(screen.getByLabelText("Loading RepoAtlas")).toBeInTheDocument();
    expect(container.firstElementChild).toHaveClass("workspace-skeleton");

    const cssPath = resolve(process.cwd(), "src/styles/50-feedback-settings.css");
    const css = readFileSync(cssPath, "utf8");
    expect(css).toMatch(/\.workspace-skeleton\s*\{[^}]*grid-column:\s*1\s*\/\s*-1\s*;?[^}]*\}/s);
  });
});
