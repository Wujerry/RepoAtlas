import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("task console layout", () => {
  it("reserves the complete output height on the opening frame", () => {
    const styles = readFileSync(resolve(process.cwd(), "src/styles/40-tasks.css"), "utf8");

    expect(styles).toMatch(/\.task-console\.is-collapsed\s*\{[^}]*height:\s*40px;/);
    expect(styles).toMatch(/\.task-console\.is-open\s*\{[^}]*height:\s*clamp\(230px,\s*calc\(34vh \+ 40px\),\s*360px\);/);
    expect(styles).toMatch(/\.task-console \.task-xterm\s*\{[^}]*height:\s*100%;/);
    expect(styles).not.toMatch(/\.task-console[^}]*transition[^}]*height/);
  });

  it("opens the full-window workbench without an ancestor motion layer", () => {
    const component = readFileSync(resolve(process.cwd(), "src/components/TaskWorkbench.tsx"), "utf8");

    expect(component).not.toContain("fadeMotion");
    expect(component).not.toContain("<motion.section");
  });

  it("reveals xterm only after its first fitted viewport has painted", () => {
    const component = readFileSync(resolve(process.cwd(), "src/components/TaskTerminal.tsx"), "utf8");
    const styles = readFileSync(resolve(process.cwd(), "src/styles/40-tasks.css"), "utf8");

    expect(component).toContain('import "@xterm/xterm/css/xterm.css"');
    expect(component).toContain('host.classList.remove("is-ready")');
    expect(component).toMatch(/requestAnimationFrame\(\(\) => \{\s*resize\(\);\s*revealFrame = window\.requestAnimationFrame/);
    expect(component).toContain('host.classList.add("is-ready")');
    expect(styles).toMatch(/\.task-xterm:not\(\.is-ready\) \.xterm\s*\{\s*visibility:\s*hidden;\s*\}/);
  });
});
