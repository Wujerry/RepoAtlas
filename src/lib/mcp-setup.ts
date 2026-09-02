import type { McpSetupInfo } from "../types";

export type McpLaunchMode = "installed" | "development" | "unavailable";

export function mcpLaunchMode(info?: McpSetupInfo | null): McpLaunchMode {
  if (!info) return "unavailable";
  if (info.binaryPath) return info.binaryOrigin === "development" ? "development" : "installed";
  if (info.workspacePath) return "development";
  return "unavailable";
}

export function mcpLaunchCommand(info?: McpSetupInfo | null): { command: string; args: string[]; dbPath: string; mode: Exclude<McpLaunchMode, "unavailable"> } | null {
  if (!info) return null;
  if (info.binaryPath) return { command: info.binaryPath, args: [], dbPath: info.dbPath, mode: info.binaryOrigin === "development" ? "development" : "installed" };
  if (!info.workspacePath) return null;
  const separator = info.platform === "windows" ? "\\" : "/";
  return {
    command: "cargo",
    args: ["run", "--quiet", "--manifest-path", `${info.workspacePath}${separator}Cargo.toml`, "-p", "repoatlas-mcp"],
    dbPath: info.dbPath,
    mode: "development",
  };
}

export function buildMcpConfig(info?: McpSetupInfo): string {
  const launch = mcpLaunchCommand(info);
  if (!launch) return "";
  return JSON.stringify({
    mcpServers: {
      repoatlas: {
        command: launch.command,
        args: launch.args,
        env: { REPOATLAS_DB: launch.dbPath },
      },
    },
  }, null, 2);
}

function configSnippet(info: McpSetupInfo): string {
  return buildMcpConfig(info) || `REPOATLAS_DB=${info.dbPath}`;
}

export function buildAgentScanInstruction(info: McpSetupInfo | undefined, locale: "zh" | "en"): string {
  if (!info) return "";
  const launch = mcpLaunchCommand(info);
  if (!launch) return "";
  const config = configSnippet(info);
  if (locale === "zh") {
    const launchHint = launch.mode === "installed"
      ? `MCP 命令：${launch.command}\n共用数据库：${launch.dbPath}`
      : info.binaryPath
        ? `当前是开发配置，MCP 命令指向源码构建产物：${launch.command}\n共用数据库：${launch.dbPath}\n发布后应改用安装目录中的 ${info.binaryName}。`
        : `当前是开发配置。MCP 命令：${launch.command} ${launch.args.join(" ")}\n共用数据库：${launch.dbPath}\n发布后应改用安装目录中的 ${info.binaryName}。`;
    return `请配置 RepoAtlas MCP，然后引导我扫描并整理本地项目。RepoAtlas 只写入自己的记录，不会修改被扫描的目录。

${launchHint}

1. 先检查当前 Agent 客户端是否已经连接名为 repoatlas 的 MCP。
2. 如果还没有连接，新增一个 stdio MCP server，名称必须是 repoatlas，并使用下面的配置：
${config}
3. 配置后执行 initialize 和 tools/list，确认存在 add_scan_root、scan_root 和 register_project。
4. 先询问我需要扫描哪些目录，得到我的明确确认后再继续；不要自行猜测或挑选目录。
5. 对我确认的每个目录，先调用 add_scan_root 授权，再显式调用 scan_root 执行发现。扫描只读取目录内容，用于登记项目。
6. 扫描完成后向我总结发现的项目。每个项目优先给出项目描述和发现的任务（这两项最重要），再给名称、路径与语言、框架、VCS 等分类。项目描述和任务描述都必须使用与我交流的语言书写。描述必须来自项目内的真实依据（如 README、package.json、Cargo.toml），拿不准的内容不要编造。
7. 总结之后不需要等我确认，直接通过 MCP 写入：用 update_project 写入 description 和 tasks，用 set_project_icon 设置明确的项目图标，用 add_task/update_task 维护任务。只写入有把握的结构化任务，shell 模式任务一律不写；没有把握的字段保持原样。
8. 不要调用 run_task，也不要执行 Git 写入、Shell Mode、外部 AI 请求或文件系统删除。

以后要批量发现项目，直接说“扫描某个目录”；只想登记当前打开的一个项目时，也可以让 Agent 调用 register_project 登记该工作区根目录。`;
  }
  const launchHint = launch.mode === "installed"
    ? `MCP command: ${launch.command}\nShared database: ${launch.dbPath}`
    : info.binaryPath
      ? `This is a development build of the MCP binary: ${launch.command}\nShared database: ${launch.dbPath}\nAfter release, use the installed ${info.binaryName} instead.`
      : `This is a development fallback. MCP command: ${launch.command} ${launch.args.join(" ")}\nShared database: ${launch.dbPath}\nAfter release, use the installed ${info.binaryName} instead.`;
  return `Configure the RepoAtlas MCP server, then guide me through scanning and organizing local projects. RepoAtlas only writes its own records and never modifies scanned directories.

${launchHint}

1. Check whether the current agent client already has an MCP server named repoatlas.
2. If it is not connected, add a stdio MCP server named repoatlas with this configuration:
${config}
3. After configuring it, run initialize and tools/list and confirm that add_scan_root, scan_root, and register_project are available.
4. Ask me which directories should be scanned, and wait for my explicit confirmation before continuing; do not guess or pick directories on your own.
5. For each directory I confirm, call add_scan_root to authorize it, then explicitly call scan_root to run discovery. Scanning only reads directory contents to register projects.
6. After scanning, summarize the discovered projects for me. For each project, lead with the project description and the tasks you discovered (these two matter most), then names, paths, and classifications such as language, framework, and VCS. Write the project description and every task summary in the language I am using with you. Descriptions must come from real evidence inside the project (README, package.json, Cargo.toml, and similar); never invent what you cannot support.
7. After summarizing, write through MCP immediately without waiting for my confirmation: use update_project for description and tasks, set_project_icon for a clear icon, and add_task/update_task to maintain tasks. Only write structured tasks you are confident about, never shell-mode tasks, and leave fields you cannot support unchanged.
8. Do not call run_task, and do not perform Git writes, Shell Mode, external AI requests, or filesystem deletion.

For bulk discovery later, just say "scan this directory". To register only the currently open project, ask the agent to call register_project on that workspace root.`;
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
