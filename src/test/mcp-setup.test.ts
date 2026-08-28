import { describe, expect, it } from "vitest";
import { buildAgentSetupInstruction, buildMcpConfig } from "../lib/mcp-setup";

describe("MCP setup copy", () => {
  it("uses the installed binary path after release", () => {
    const config = buildMcpConfig({
      dbPath: "C:\\Users\\dev\\repoatlas.sqlite",
      platform: "windows",
      binaryName: "repoatlas-mcp.exe",
      binaryPath: "C:\\Program Files\\RepoAtlas\\repoatlas-mcp.exe",
      workspacePath: null,
    });
    const parsed = JSON.parse(config);
    expect(parsed.mcpServers.repoatlas.command).toBe("C:\\Program Files\\RepoAtlas\\repoatlas-mcp.exe");
    expect(parsed.mcpServers.repoatlas.args).toEqual([]);
    expect(parsed.mcpServers.repoatlas.env.REPOATLAS_DB).toBe("C:\\Users\\dev\\repoatlas.sqlite");
  });

  it("falls back to cargo only when a source checkout exists", () => {
    const config = buildMcpConfig({
      dbPath: "C:\\Users\\dev\\repoatlas.sqlite",
      platform: "windows",
      binaryName: "repoatlas-mcp.exe",
      binaryPath: null,
      workspacePath: "F:\\code\\RepoAtlas",
    });
    const parsed = JSON.parse(config);
    expect(parsed.mcpServers.repoatlas.command).toBe("cargo");
    expect(parsed.mcpServers.repoatlas.args).toContain("F:\\code\\RepoAtlas\\Cargo.toml");
  });

  it("does not invent a source path when no binary is installed", () => {
    const info = { dbPath: "/data/repoatlas.sqlite", platform: "macos", binaryName: "repoatlas-mcp", binaryPath: null, workspacePath: null };
    expect(buildMcpConfig(info)).toBe("");
    const instruction = buildAgentSetupInstruction(info, "en");
    expect(instruction).toContain("could not be located");
    expect(instruction).toContain("REPOATLAS_DB=/data/repoatlas.sqlite");
    expect(instruction).not.toContain("source checkout containing crates/repoatlas-mcp");
  });
});

