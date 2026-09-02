// Builds the repoatlas-mcp release binary for the current target triple and
// copies it into src-tauri/resources so the desktop installers always ship
// a sidecar that matches the packaged app. The build is incremental, so
// packaging stays fast when nothing changed.
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const rootDir = join(dirname(fileURLToPath(import.meta.url)), "..");
const triple = process.env.TAURI_ENV_TARGET_TRIPLE || null;
const isWindows = triple ? triple.includes("windows") : process.platform === "win32";
const binaryName = isWindows ? "repoatlas-mcp.exe" : "repoatlas-mcp";
const resourceDir = join(rootDir, "src-tauri", "resources", "repoatlas-mcp");
const resourceBinary = join(resourceDir, binaryName);
const cargoArgs = ["build", "--release", "-p", "repoatlas-mcp"];
if (triple) cargoArgs.push("--target", triple);
const targetBinary = triple
  ? join(rootDir, "target", triple, "release", binaryName)
  : join(rootDir, "target", "release", binaryName);

console.log(`[copy-mcp-sidecar] building repoatlas-mcp (cargo ${cargoArgs.join(" ")})`);
const result = spawnSync("cargo", cargoArgs, { cwd: rootDir, stdio: "inherit" });
if (result.status !== 0) {
  console.error(`[copy-mcp-sidecar] cargo build -p repoatlas-mcp failed with exit code ${result.status}.`);
  process.exit(1);
}

if (!existsSync(targetBinary)) {
  console.error(`[copy-mcp-sidecar] repoatlas-mcp binary was not found at ${targetBinary}.`);
  process.exit(1);
}

mkdirSync(resourceDir, { recursive: true });
copyFileSync(targetBinary, resourceBinary);
console.log(`[copy-mcp-sidecar] ${targetBinary} -> ${resourceBinary}`);
