import { describe, expect, it } from "vitest";
import { formatTime, setFormatLocale } from "../lib/format";

describe("formatTime follows the app locale", () => {
  const timestamp = "2026-08-28T18:27:00";

  it("formats dates in English when the app locale is en", () => {
    setFormatLocale("en");
    expect(formatTime(timestamp)).toMatch(/Aug 28/);
  });

  it("formats dates in Chinese when the app locale is zh", () => {
    setFormatLocale("zh");
    expect(formatTime(timestamp)).toMatch(/8月28日/);
  });

  it("falls back to the raw value for missing or invalid input", () => {
    setFormatLocale("en");
    expect(formatTime(null)).toBe("—");
    expect(formatTime("not-a-date")).toBe("not-a-date");
  });
  it("uses an explicit locale even when the module locale differs", () => {
    setFormatLocale("en");
    expect(formatTime(timestamp, "zh")).toMatch(/8月28日/);
  });

});
