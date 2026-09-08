import { expect, it } from "vitest";
import { mkdtempSync, writeFileSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { execFileSync } from "node:child_process";

it.each(["", "-UNSIGNED-BETA"])("prefers NSIS over MSI with release label %s", (label) => {
  const root = mkdtempSync(join(tmpdir(), "repoatlas-manifest-test-"));
  try {
    const installer = `x86_64-pc-windows-msvc-RepoAtlas_0.1.0_x64-setup${label}.exe`;
    const names = [installer, `x86_64-pc-windows-msvc-RepoAtlas_0.1.0_x64${label}.msi`, "x86_64-apple-darwin-RepoAtlas.app.tar.gz", "aarch64-apple-darwin-RepoAtlas.app.tar.gz"];
    for (const name of names) {
      writeFileSync(join(root, name), "fixture");
      writeFileSync(join(root, name + ".sig"), "fixture-signature");
    }
    const output = join(root, "latest.json");
    execFileSync(process.execPath, ["scripts/build-updater-manifest.mjs", "--label", "UNSIGNED-BETA", "--version", "0.1.0-beta.1", "--artifacts", root, "--output", output, "--base-url", "https://example.invalid/releases"]);
    const manifest = JSON.parse(readFileSync(output, "utf8"));
    expect(manifest.platforms["windows-x86_64"].url).toBe(`https://example.invalid/releases/${installer}`);
    expect(manifest.platforms["windows-x86_64"].signature).toBe("fixture-signature");
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
