import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { SplashScreen } from "../components/SplashScreen";

vi.mock("../lib/api", () => ({
  api: {
    showMainWindow: vi.fn(async () => undefined),
  },
}));

describe("SplashScreen", () => {
  it("finishes immediately under reduced motion once bootstrap is ready", async () => {
    window.matchMedia = ((query: string) => ({
      matches: query.includes("prefers-reduced-motion"),
      media: query,
      onchange: null,
      addListener: () => undefined,
      removeListener: () => undefined,
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
      dispatchEvent: () => false,
    })) as typeof window.matchMedia;
    const onFinished = vi.fn();
    render(<SplashScreen ready onFinished={onFinished} />);
    expect(screen.getByRole("status")).toHaveClass("is-static");
    await waitFor(() => expect(onFinished).toHaveBeenCalled());
  });

  it("renders a readable RepoAtlas wordmark instead of letter paths", () => {
    render(<SplashScreen ready={false} onFinished={vi.fn()} />);
    expect(screen.getAllByText("RepoAtlas").length).toBeGreaterThan(0);
    expect(document.querySelector(".splash-wordmark span")?.textContent).toBe("RepoAtlas");
  });
});
