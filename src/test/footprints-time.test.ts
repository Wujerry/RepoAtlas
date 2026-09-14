import { afterEach, describe, expect, it, vi } from "vitest";
import { dayRange, timelineRows } from "../lib/footprints";
import type { ActivityHistoryItem } from "../types";

afterEach(() => vi.unstubAllEnvs());
const event = (id: string, occurredAt: string) => ({ id, occurredAt } as ActivityHistoryItem);
describe("Footprints local calendar boundaries", () => {
  it("uses 23 and 25 hour UTC intervals across daylight saving changes", () => {
    vi.stubEnv("TZ", "America/New_York");
    const spring = dayRange("2026-03-08");
    const autumn = dayRange("2026-11-01");
    expect(spring.endAt - spring.startAt).toBe(23 * 3600);
    expect(autumn.endAt - autumn.startAt).toBe(25 * 3600);
  });
  it("separates local hours in a half-hour timezone", () => {
    vi.stubEnv("TZ", "Asia/Kolkata");
    const formatter = new Intl.DateTimeFormat("en", { hour: "2-digit", hour12: false });
    const rows = timelineRows([event("a", "2026-09-01T00:45:00Z"), event("b", "2026-09-01T00:15:00Z")], formatter);
    expect(rows.filter(row => row.type === "hour")).toHaveLength(2);
  });
  it("keeps the repeated autumn hour as two distinct timeline groups", () => {
    vi.stubEnv("TZ", "America/New_York");
    const formatter = new Intl.DateTimeFormat("en", { hour: "2-digit", hour12: false });
    const rows = timelineRows([event("a", "2026-11-01T06:30:00Z"), event("b", "2026-11-01T05:30:00Z")], formatter);
    const hours = rows.filter(row => row.type === "hour");
    expect(hours).toHaveLength(2);
    expect(hours[0].id).not.toBe(hours[1].id);
  });
});
