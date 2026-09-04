import { describe, expect, it } from "vitest";
import { buildAgentScanInstruction, buildAgentSetupInstruction, buildMcpConfig, mcpLaunchMode } from "../lib/mcp-setup";

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

  it("builds a guided scan instruction for the installed binary", () => {
    const instruction = buildAgentScanInstruction({
      dbPath: "C:\\Users\\dev\\repoatlas.sqlite",
      platform: "windows",
      binaryName: "repoatlas-mcp.exe",
      binaryPath: "C:\\Program Files\\RepoAtlas\\repoatlas-mcp.exe",
      workspacePath: null,
    }, "zh");
    expect(instruction).toContain("register_project");
    expect(instruction).toContain("C:\\Program Files\\RepoAtlas\\repoatlas-mcp.exe");
    expect(instruction).toContain("REPOATLAS_DB");
    expect(instruction).toContain("add_scan_root");
    expect(instruction).toContain("scan_root");
    expect(instruction).toContain("询问我需要扫描哪些目录");
    expect(instruction).toContain("项目描述和发现的任务");
    expect(instruction).toContain("使用与我交流的语言书写");
    expect(instruction).toContain("不需要等我确认");
    expect(instruction).toContain("update_project");
    expect(instruction).toContain("set_project_icon");
    expect(instruction).toContain("不要用 update_project 的 tasks 参数整体覆盖已有任务");
    expect(instruction).toContain("在项目目录内查找真实存在的图标文件");
    expect(instruction).toContain("找不到就跳过");
    expect(instruction).toContain("不要调用 run_task");
  });

  it("marks cargo as a development fallback in the scan instruction", () => {
    const instruction = buildAgentScanInstruction({
      dbPath: "C:\\Users\\dev\\repoatlas.sqlite",
      platform: "windows",
      binaryName: "repoatlas-mcp.exe",
      binaryPath: null,
      workspacePath: "F:\\code\\RepoAtlas",
    }, "en");
    expect(instruction).toContain("development fallback");
    expect(instruction).toContain("cargo");
    expect(instruction).toContain("register_project");
    expect(instruction).toContain("add_scan_root");
    expect(instruction).toContain("project description and the tasks you discovered");
    expect(instruction).toContain("in the language I am using with you");
    expect(instruction).toContain("without waiting for my confirmation");
    expect(instruction).toContain("update_project");
    expect(instruction).toContain("do not use the tasks parameter of update_project");
    expect(instruction).toContain("search the project directory for a real icon file");
    expect(instruction).toContain("skip when none is found");
    expect(instruction).toContain("Do not call run_task");
  });

  it("labels a source-built MCP binary as a development configuration", () => {
    const info = {
      dbPath: "C:\\Users\\dev\\repoatlas.sqlite",
      platform: "windows",
      binaryName: "repoatlas-mcp.exe",
      binaryPath: "F:\\code\\RepoAtlas\\target\\debug\\repoatlas-mcp.exe",
      binaryOrigin: "development" as const,
      workspacePath: null,
    };
    expect(mcpLaunchMode(info)).toBe("development");
    const instruction = buildAgentScanInstruction(info, "zh");
    expect(instruction).toContain("当前是开发配置");
    expect(instruction).toContain("F:\\code\\RepoAtlas\\target\\debug\\repoatlas-mcp.exe");
    expect(instruction).toContain("发布后应改用安装目录中的 repoatlas-mcp.exe");
  });

  it("treats a binary without origin information as installed", () => {
    expect(mcpLaunchMode({
      dbPath: "C:\\Users\\dev\\repoatlas.sqlite",
      platform: "windows",
      binaryName: "repoatlas-mcp.exe",
      binaryPath: "C:\\Program Files\\RepoAtlas\\repoatlas-mcp.exe",
      workspacePath: null,
    })).toBe("installed");
  });

  it("does not invent a scan instruction when MCP cannot launch", () => {
    expect(buildAgentScanInstruction({
      dbPath: "/data/repoatlas.sqlite",
      platform: "macos",
      binaryName: "repoatlas-mcp",
      binaryPath: null,
      workspacePath: null,
    }, "en")).toBe("");
  });
});

