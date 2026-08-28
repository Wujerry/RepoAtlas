import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { MarkdownDocument } from "../components/Dashboard";

describe("MarkdownDocument", () => {
  it("renders GFM structure and blocks executable or remote image markup", () => {
    const content = `# Project guide

- [x] rendered task

| Name | Value |
| --- | --- |
| Mode | Local |

[safe](./docs/setup.md)
[danger](javascript:alert(1))

![remote](https://example.com/pixel.png)

<script>window.__unsafe = true</script>
`;
    const { container } = render(<MarkdownDocument content={content} />);

    expect(screen.getByRole("heading", { name: "Project guide" })).toBeInTheDocument();
    expect(screen.getByRole("table")).toBeInTheDocument();
    expect(screen.getByRole("checkbox")).toBeDisabled();
    expect(container.querySelector("img")).toBeNull();
    expect(container.querySelector("script")).toBeNull();
    expect(screen.getByText("remote")).toHaveClass("readme-image-placeholder");
    expect(screen.getByText("danger").closest("a")?.getAttribute("href") ?? "").not.toMatch(/^javascript:/i);
  });
});
