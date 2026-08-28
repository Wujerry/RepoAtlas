import { describe, expect, it } from "vitest";
import { parseAnsi, stripAnsi } from "../lib/ansi";

describe("ansi log rendering", () => {
  it("strips color codes and keeps the visible text", () => {
    const input = "\u001b[32m\u2713 built in 17.80s\u001b[39m";
    expect(stripAnsi(input)).toBe("\u2713 built in 17.80s");
    expect(parseAnsi(input)).toEqual([
      { text: "\u2713 built in 17.80s", className: "ansi-green" },
    ]);
  });

  it("resets styles and ignores non-color escape sequences", () => {
    const input = "\u001b[1;32mok\u001b[0m done\u001b[2K";
    expect(stripAnsi(input)).toBe("ok done");
    expect(parseAnsi(input)).toEqual([
      { text: "ok", className: "ansi-bold ansi-green" },
      { text: " done" },
    ]);
  });
});

