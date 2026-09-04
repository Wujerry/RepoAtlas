import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("project hero responsive layout", () => {
  it("resets the horizontal flex basis when the hero stacks vertically", () => {
    const css = readFileSync(resolve(process.cwd(), "src/styles/90-motion.css"), "utf8");

    expect(css).toMatch(
      /@media\s*\(max-width:\s*1099px\)[\s\S]*?\.project-hero-side\s*\{[^}]*flex:\s*0\s+0\s+auto;[^}]*align-content:\s*start;/,
    );
  });
});
