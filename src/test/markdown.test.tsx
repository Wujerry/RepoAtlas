import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { MarkdownDocument } from "../components/Dashboard";
import { api } from "../lib/api";

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
<iframe src="https://example.com"></iframe>
<img src="data:image/svg+xml,bad" onerror="window.__unsafe = true" />
`;
    const { container } = render(<MarkdownDocument content={content} />);

    expect(screen.getByRole("heading", { name: "Project guide" })).toBeInTheDocument();
    expect(screen.getByRole("table")).toBeInTheDocument();
    expect(screen.getByRole("checkbox")).toBeDisabled();
    expect(container.querySelector("img")).toBeNull();
    expect(container.querySelector("script")).toBeNull();
    expect(container.querySelector("iframe")).toBeNull();
    expect(container.querySelector('[onerror]')).toBeNull();
    expect(screen.getByText("remote")).toHaveClass("readme-image-placeholder");
    expect(screen.getByText("danger").closest("a")?.getAttribute("href") ?? "").not.toMatch(/^javascript:/i);
  });

  it("renders math, GitHub alerts, details, and strips frontmatter", () => {
    const content = `---
private: true
---

> [!NOTE]
> Local only

Inline $x^2$ and block:

$$x = y + 1$$

<details><summary>More</summary><kbd>Ctrl</kbd> + <mark>K</mark></details>
`;
    const { container } = render(<MarkdownDocument content={content} />);

    expect(screen.queryByText("private: true")).toBeNull();
    expect(screen.getByText("Local only")).toBeInTheDocument();
    expect(container.querySelector(".markdown-alert")).not.toBeNull();
    expect(container.querySelector(".katex")).not.toBeNull();
    expect(screen.getByText("More").closest("details")).not.toBeNull();
    expect(screen.getByText("Ctrl").tagName).toBe("KBD");
  });

  it("renders fenced code without a second pre wrapper", () => {
    const { container } = render(<MarkdownDocument content={'```shell\npnpm install\npnpm tauri dev\n```'} />);

    const preview = container.querySelector(".code-preview");
    expect(preview).not.toBeNull();
    expect(preview?.closest("pre")).toBeNull();
    expect(container.querySelector(".readme-document > pre")).toBeNull();
  });

  it("preserves safe HTML image dimensions for local project images", async () => {
    const OriginalIntersectionObserver = globalThis.IntersectionObserver;
    const createObjectUrl = vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:repoatlas-logo");
    const revokeObjectUrl = vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => undefined);
    const readImage = vi.spyOn(api, "readProjectImage").mockResolvedValue(new ArrayBuffer(8));
    class VisibleObserver {
      private readonly callback: IntersectionObserverCallback;
      constructor(callback: IntersectionObserverCallback) { this.callback = callback; }
      observe(target: Element) { this.callback([{ isIntersecting: true, target } as IntersectionObserverEntry], this as unknown as IntersectionObserver); }
      disconnect() {}
      unobserve() {}
      takeRecords() { return []; }
      readonly root = null;
      readonly rootMargin = "0px";
      readonly thresholds = [0];
    }
    globalThis.IntersectionObserver = VisibleObserver as unknown as typeof IntersectionObserver;

    const { container, unmount } = render(<MarkdownDocument projectId="atlas" content={'<img src="assets/logo.png" width="96" height="64" alt="Logo" />'} />);
    await waitFor(() => expect(container.querySelector("img")).not.toBeNull());
    const image = container.querySelector("img");
    expect(image).toHaveAttribute("width", "96");
    expect(image).toHaveAttribute("height", "64");
    fireEvent.scroll(container);
    expect(container.querySelector("img")).toBe(image);
    expect(readImage).toHaveBeenCalledTimes(1);
    unmount();
    expect(revokeObjectUrl).toHaveBeenCalledWith("blob:repoatlas-logo");

    globalThis.IntersectionObserver = OriginalIntersectionObserver;
    createObjectUrl.mockRestore();
    revokeObjectUrl.mockRestore();
    readImage.mockRestore();
  });
});
