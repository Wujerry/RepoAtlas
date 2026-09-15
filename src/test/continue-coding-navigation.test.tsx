import { render, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { dictionaries, type MessageKey } from "../i18n";

const mocks = vi.hoisted(() => ({
  recent: vi.fn(async (_id?: string) => []),
  refresh: vi.fn(async () => ({ running: false })),
  status: vi.fn(async () => ({ running: false })),
}));
vi.mock("../lib/sessions", () => ({ sessionApi: mocks }));
vi.mock("../lib/api", () => ({ api: { readProjectIcons: vi.fn(async () => []) } }));
import { ContinueCoding } from "../components/AIHistory";
const t = (key: MessageKey) => dictionaries.en[key];
beforeEach(() => vi.clearAllMocks());

it("reads indexed sessions without starting another index refresh on project changes", async () => {
  const view = render(<ContinueCoding t={t} projectId="first" />);
  await waitFor(() => expect(mocks.recent).toHaveBeenCalledWith("first"));
  view.rerender(<ContinueCoding t={t} projectId="second" />);
  await waitFor(() => expect(mocks.recent).toHaveBeenCalledWith("second"));
  expect(mocks.refresh).not.toHaveBeenCalled();
});

it("still starts background indexing on the home surface", async () => {
  render(<ContinueCoding t={t} />);
  await waitFor(() => expect(mocks.refresh).toHaveBeenCalledTimes(1));
});
