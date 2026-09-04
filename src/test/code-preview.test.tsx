import { render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const highlightCode = vi.hoisted(() => vi.fn(async () => []));

vi.mock("../lib/shiki-client", () => ({ highlightCode }));

import { CodePreview } from "../components/CodePreview";

describe("CodePreview theme", () => {
  const originalTheme = document.documentElement.dataset.theme;

  beforeEach(() => highlightCode.mockClear());

  afterEach(() => {
    if (originalTheme) document.documentElement.dataset.theme = originalTheme;
    else delete document.documentElement.dataset.theme;
  });

  it("uses the explicit application theme instead of the operating-system preference", async () => {
    document.documentElement.dataset.theme = "dark";
    const { rerender } = render(<CodePreview code="const value = 1" language="typescript" />);
    await waitFor(() => expect(highlightCode).toHaveBeenLastCalledWith("const value = 1", "typescript", true, expect.any(AbortSignal)));

    document.documentElement.dataset.theme = "light";
    rerender(<CodePreview code="const value = 2" language="typescript" />);
    await waitFor(() => expect(highlightCode).toHaveBeenLastCalledWith("const value = 2", "typescript", false, expect.any(AbortSignal)));
  });
});
