import type { McpSetupInfo } from "../types";

function commandFor(info: McpSetupInfo): { command: string; args: string[] } | null {
  if (info.binaryPath) return { command: info.binaryPath, args: [] };
  if (!info.workspacePath) return null;
  const separator = info.platform === "windows" ? "\\" : "/";
  return {
    command: "cargo",
    args: ["run", "--quiet", "--manifest-path", `${info.workspacePath}${separator}Cargo.toml`, "-p", "repoatlas-mcp"],
  };
}

export function buildMcpConfig(info?: McpSetupInfo): string {
  if (!info) return "";
  const launch = commandFor(info);
  if (!launch) return "";
  return JSON.stringify({
    mcpServers: {
      repoatlas: {
        command: launch.command,
        args: launch.args,
        env: { REPOATLAS_DB: info.dbPath },
      },
    },
  }, null, 2);
}

export function buildAgentSetupInstruction(info: McpSetupInfo | undefined, locale: "zh" | "en"): string {
  if (!info) return "";
  if (info.binaryPath) {
    return locale === "zh"
      ? `请为你当前运行的 Agent 客户端配置 RepoAtlas MCP。\n\n1. MCP 命令：${info.binaryPath}\n2. 共用数据库：${info.dbPath}\n3. 在 MCP 配置中新增名为 repoatlas 的 stdio server。command 使用上面的绝对路径，env 必须设置 REPOATLAS_DB=${info.dbPath}。\n4. 配置后执行 initialize 和 tools/list 验证连接，并向我报告可用工具。\n5. MCP 可直接管理 Project、Scan Root、任务定义、图标与项目知识；run_task 只能创建桌面待审批请求。不要开放 Git 写入、Shell Mode、外部 AI 请求或磁盘删除能力。`
      : `Configure the RepoAtlas MCP server for your current agent client.\n\n1. MCP command: ${info.binaryPath}\n2. Shared database: ${info.dbPath}\n3. Add a stdio MCP server named repoatlas. Use that absolute path as command and set REPOATLAS_DB=${info.dbPath}.\n4. Verify initialize and tools/list, then report the available tools.\n5. MCP may directly manage Projects, Scan Roots, task definitions, icons, and project knowledge; run_task may only create a pending desktop approval. Do not add Git writes, Shell Mode, external AI requests, or disk deletion capabilities.`;
  }
  if (info.workspacePath) {
    return locale === "zh"
      ? `当前还没有找到已安装的 MCP 二进制，可先用源码目录开发配置。\n\n1. RepoAtlas 源码目录：${info.workspacePath}\n2. 共用数据库：${info.dbPath}\n3. 在源码目录运行 cargo build -p repoatlas-mcp --release。\n4. 发布后应改用安装目录中的 ${info.binaryName}，而不是源码目录。\n5. 开发期可以使用 cargo run --quiet --manifest-path ${info.workspacePath}/Cargo.toml -p repoatlas-mcp，并设置 REPOATLAS_DB=${info.dbPath}。`
      : `No installed MCP binary was found, so this is a development fallback.\n\n1. RepoAtlas source checkout: ${info.workspacePath}\n2. Shared database: ${info.dbPath}\n3. Run cargo build -p repoatlas-mcp --release in the checkout.\n4. After release, use the installed ${info.binaryName} instead of the source checkout.\n5. For development you can run cargo run --quiet --manifest-path ${info.workspacePath}/Cargo.toml -p repoatlas-mcp with REPOATLAS_DB=${info.dbPath}.`;
  }
  return locale === "zh"
    ? `未找到 RepoAtlas MCP 二进制。\n\n1. 共用数据库：${info.dbPath}\n2. 发布版应使用与桌面应用一起安装的 ${info.binaryName}。\n3. 在 MCP 配置中把 command 设为该二进制的绝对路径，并设置 REPOATLAS_DB=${info.dbPath}。\n4. 不要依赖源码目录或 cargo run。`
    : `The RepoAtlas MCP binary could not be located.\n\n1. Shared database: ${info.dbPath}\n2. A released app should use the installed ${info.binaryName} next to the desktop app.\n3. Set command to that absolute path and REPOATLAS_DB=${info.dbPath}.\n4. Do not depend on a source checkout or cargo run.`;
}

