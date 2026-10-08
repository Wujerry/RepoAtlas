export type Locale = "en" | "zh";

export type HeadlineLine = { text: string; accent?: boolean };

export type Fact = { key: string; title: string; body: string };
export type Cell = { title: string; body: string; mono?: string };

export type Copy = {
  lang: "en" | "zh-CN";
  dir: "ltr";
  title: string;
  description: string;
  skip: string;
  homeAria: string;
  navAria: string;
  navSessions: string;
  sessionsTitle: string;
  sessionsBody: string;
  sessionsSteps: Cell[];
  sessionsAgents: string;
  sessionsNote: string;
  sessionsImageAlt: string;
  searchImageAlt: string;
  searchImageLabel: string;
  navUsage: string;
  usageTitle: string;
  usageBody: string;
  usageImageAlt: string;
  usageNote: string;
  libraryImageAlt: string;
  showcaseCaption: string;
  navTasks: string;
  navLibrary: string;
  navSafety: string;
  navLang: string;
  navLangHref: string;
  navLangLang: "en" | "zh-CN";
  navSource: string;
  heroHeadline: HeadlineLine[];
  heroSub: string;
  ctaPrimary: string;
  ctaSecondary: string;
  heroImageAlt: string;
  marqueeAria: string;
  marqueeCommands: string[];
  tasksTitle: string;
  tasksBody: string;
  tasksFacts: Fact[];
  tasksImageAlt: string;
  libraryTitle: string;
  libraryBody: string;
  libraryCells: Cell[];
  safetyTitle: string;
  safetyBody: string;
  safetyRules: string[];
  closeTitle: string;
  closeBody: string;
  closePrimary: string;
  closeSecondary: string;
  footerBlurb: string;
  footerSecurity: string;
};

const marqueeCommands = [
  "pnpm dev",
  "pnpm build",
  "pnpm vitest run",
  "cargo test -p repoatlas-core",
  "pnpm tauri build",
  "git pull --ff-only",
  "cargo check",
  "pnpm typecheck",
];

export const copy: Record<Locale, Copy> = {
  "en": {
    "lang": "en",
    "sessionsImageAlt": "RepoAtlas Sessions with a cross-Agent session list and the selected coding conversation",
    "searchImageAlt": "RepoAtlas unified search with a matching session and message preview",
    "searchImageLabel": "Search Projects and conversations with Ctrl+K",
    "navUsage": "Usage",
    "usageTitle": "Your Agent quotas, right in the title bar.",
    "usageBody": "See remaining limits for connected services while you work. Open the readout for each usage window and its reset countdown. Choose which services appear in navigation without disconnecting them.",
    "usageImageAlt": "RepoAtlas navigation showing Codex, Claude, GitHub Copilot and OpenCode Go remaining quotas, with usage windows and reset times expanded",
    "usageNote": "Connect each service separately using its existing login. Seventeen usage sources are supported; account access stays with the external tools. Session token counts and API-equivalent cost estimates are separate from subscription limits and billing.",
    "libraryImageAlt": "RepoAtlas Project library and Atlas Dashboard overview, with coding sessions, task shortcuts and Git state",
    "showcaseCaption": "Desktop screenshots use fictional Projects, conversations, token counts, and subscription quotas.",
    "navSessions": "Sessions",
    "sessionsTitle": "Find the conversation. Continue the work.",
    "sessionsBody": "Search coding sessions across Agents in one place. Preview the conversation, find its Project, and reopen the selected session in the original Agent.",
    "sessionsSteps": [{"title": "Authorize your history sources", "body": "Review the absolute directories individually or use Authorize all. You choose which histories RepoAtlas indexes locally and exposes to connected MCP clients."}, {"title": "Search and preview", "body": "Search across Agents, or narrow by Project, date and archive status. Read matching messages before deciding which conversation to continue."}, {"title": "Resume the selected session", "body": "Review the command and working directory. Choose CLI or a supported App entry, then continue with the original Agent and session ID."}],
    "sessionsAgents": "Claude Code · Codex CLI · OpenCode · Cursor CLI · Gemini CLI · GitHub Copilot CLI · Kimi Code · Qwen Code",
    "sessionsNote": "History access is read-only. Revoking a source clears its local index without deleting Agent history. Resume requires the installed Agent and an available session; accounts and model access stay with the Agent.",
    "dir": "ltr",
    "title": "RepoAtlas: projects, Agent sessions, and development tasks",
    "description": "Find local Projects, resume Agent sessions, track subscription quotas, and run development tasks in one desktop app.",
    "skip": "Skip to content",
    "homeAria": "RepoAtlas home",
    "navAria": "Primary navigation",
    "navTasks": "Tasks",
    "navLibrary": "Projects",
    "navSafety": "Local data",
    "navLang": "中文",
    "navLangHref": "./zh/",
    "navLangLang": "zh-CN",
    "navSource": "GitHub",
    "heroHeadline": [
      {
        "text": "Your projects in one place."
      },
      {
        "text": "Pick up where you left off.",
        "accent": true
      }
    ],
    "heroSub": "Open a Project, resume a coding session, or run a saved task. See your Agent quotas in the title bar, with project details, Git changes, and task output close at hand.",
    "ctaPrimary": "View releases",
    "ctaSecondary": "Explore Sessions",
    "heroImageAlt": "RepoAtlas home workspace showing recent Agent sessions, project icons, recent Projects and task runs",
    "marqueeAria": "Examples of commands saved as tasks",
    "marqueeCommands": marqueeCommands,
    "tasksTitle": "Save a task. Run it again when you need it.",
    "tasksBody": "Keep development, test, build, and packaging tasks with each project. Run several tasks side by side, type into their terminals, and check the output as it arrives.",
    "tasksFacts": [
      {
        "key": "configuration",
        "title": "Saved commands",
        "body": "Save the program, arguments, and working directory. Each task keeps its own configuration."
      },
      {
        "key": "terminal",
        "title": "Live terminal output",
        "body": "Read build output, answer interactive prompts, and stop a running task from the workbench."
      },
      {
        "key": "processes",
        "title": "CPU, memory, and ports",
        "body": "Check resource usage, child processes, and listening ports. Open a running local development page from its address."
      },
      {
        "key": "history",
        "title": "Logs and exit codes",
        "body": "Review previous runs with their status, duration, exit code, and output."
      }
    ],
    "tasksImageAlt": "RepoAtlas task workbench showing saved tasks, process metrics, and live terminals",
    "libraryTitle": "Find a project and get back to work.",
    "libraryBody": "Add a Scan Root to discover projects in a directory, or register one project directly. Search the library and group related projects into collections.",
    "libraryCells": [
      {
        "title": "Project discovery",
        "body": "Identify languages, frameworks, package managers, and modules. Refresh the scan when your directories change.",
        "mono": "Name · Path · Language · Framework"
      },
      {
        "title": "Git status and changes",
        "body": "See the current branch and changed files. Review diffs, stage files, commit, and pull or push using your existing Git credentials."
      },
      {
        "title": "File browsing and preview",
        "body": "Browse folders, search paths, and read Markdown, source code, and images without opening an editor."
      },
      {
        "title": "Workspace and recent projects",
        "body": "Start with recent sessions, project icons, recent Projects and task runs. Open a Project from its title; use collections and favorites to organize your library."
      },
      {
        "title": "Editors, terminals, and coding agents",
        "body": "Open installed tools at the project path. Connect an external agent through MCP to manage project records and request task runs."
      }
    ],
    "safetyTitle": "Your project library stays on your computer.",
    "safetyBody": "RepoAtlas stores project records, settings, and task history locally. You choose which directories it scans.",
    "safetyRules": [
      "Use the library offline without creating an account.",
      "Removing a project or collection removes its records; project files stay on disk.",
      "Git pulls use fast-forward only. SVN support is read-only.",
      "Task requests from MCP wait for desktop approval.",
      "External coding agents manage their own accounts, models, and conversations."
    ],
    "closeTitle": "Start with your project directory.",
    "closeBody": "Add a Scan Root, review the discovered projects, and save the commands you use most often. The source and setup instructions are on GitHub.",
    "closePrimary": "View source",
    "closeSecondary": "Contribute",
    "footerBlurb": "0.1.7 · Windows UNSIGNED · macOS experimental, not notarized · Signed updates · MIT",
    "footerSecurity": "Security policy"
  },
  "zh": {
    "lang": "zh-CN",
    "sessionsImageAlt": "RepoAtlas 会话页：各 Agent 的历史会话与选中会话的消息预览",
    "searchImageAlt": "RepoAtlas 统一搜索：匹配会话与消息预览",
    "searchImageLabel": "按 Ctrl+K 搜索项目与编码会话",
    "navUsage": "用量",
    "usageTitle": "Agent 剩余额度，抬头就能看到",
    "usageBody": "导航栏直接显示已连接服务的剩余额度，点击展开各个用量周期和重置倒计时。可以单独选择哪些服务显示在导航栏，隐藏后仍保留连接。",
    "usageImageAlt": "RepoAtlas 导航栏显示 Codex、Claude、GitHub Copilot 和 OpenCode Go 的剩余额度，展开面板显示用量周期与重置时间",
    "usageNote": "支持 17 个用量来源，每个服务需使用已有登录单独连接，账号仍由外部工具管理。会话中的 Token 计数和 API 美元等值估算独立显示，不等同于订阅额度或实际账单。",
    "libraryImageAlt": "RepoAtlas 项目库与 Atlas Dashboard 概览：最近会话、任务入口和 Git 状态",
    "showcaseCaption": "桌面应用实拍，项目、会话、Token 计数及订阅额度均为虚构演示数据。",
    "navSessions": "会话",
    "sessionsTitle": "Agent 会话检索与恢复",
    "sessionsBody": "检索 Claude Code、Codex 等编码 Agent 的本地会话记录，查看匹配消息及上下文，并在原 Agent 中恢复指定会话。支持按项目、Agent、日期和归档状态筛选。",
    "sessionsSteps": [
      {"title": "历史来源授权", "body": "确认需要读取的历史目录后，RepoAtlas 在本机建立搜索索引。已连接的 MCP 客户端可只读访问已授权的会话。"},
      {"title": "消息搜索与预览", "body": "按关键词查找会话原文，预览匹配消息及前后文。支持 Markdown、代码块、表格和数学公式。"},
      {"title": "会话恢复", "body": "核对工作目录和启动命令，通过 CLI 或受支持的桌面应用恢复会话。启动命令可复制到终端执行。"}
    ],
    "sessionsAgents": "Claude Code · Codex CLI · OpenCode · Cursor CLI · Gemini CLI · GitHub Copilot CLI · Kimi Code · Qwen Code",
    "sessionsNote": "会话历史以只读方式访问。撤销授权会清理对应的本地索引，保留 Agent 原始文件。恢复会话需要已安装的 Agent、有效会话记录和可用工作目录。",
    "dir": "ltr",
    "title": "RepoAtlas：本地项目管理工具",
    "description": "在本机整理项目、搜索并继续 Agent 会话、查看订阅剩余额度，运行开发任务和检查 Git 改动。",
    "skip": "跳到主要内容",
    "homeAria": "RepoAtlas 中文首页",
    "navAria": "主导航",
    "navTasks": "任务执行",
    "navLibrary": "项目管理",
    "navSafety": "数据与权限",
    "navLang": "English",
    "navLangHref": "../",
    "navLangLang": "en",
    "navSource": "GitHub",
    "heroHeadline": [{"text": "本地项目"}, {"text": "与开发任务管理", "accent": true}],
    "heroSub": "找到项目，继续上次的编码会话，运行常用开发任务。导航栏随时显示 Agent 剩余额度，项目资料、Git 改动与任务记录都在本机查看。",
    "ctaPrimary": "下载安装包",
    "ctaSecondary": "查看会话功能",
    "heroImageAlt": "RepoAtlas 首页工作台：最近 Agent 会话、项目图标、最近项目与任务运行",
    "marqueeAria": "可保存为任务的常用命令示例",
    "marqueeCommands": marqueeCommands,
    "tasksTitle": "开发任务配置与执行",
    "tasksBody": "为项目保存启动、测试、构建和打包任务，按配置的程序、参数和工作目录执行。任务工作台支持跨项目查看运行状态，最多并排显示四个交互式终端。",
    "tasksFacts": [
      {
        "key": "任务配置",
        "title": "程序、参数与工作目录",
        "body": "每个任务独立保存执行配置，支持查看、编辑和重复运行。"
      },
      {
        "key": "实时终端",
        "title": "终端输出与交互",
        "body": "实时显示任务输出，支持键盘输入、交互提示响应和停止操作。"
      },
      {
        "key": "运行监控",
        "title": "资源占用与监听端口",
        "body": "查看 CPU、内存、子进程和监听端口，通过本地开发地址打开预览页面。"
      },
      {
        "key": "运行记录",
        "title": "日志与退出状态",
        "body": "保留每次运行的状态、耗时、退出码和输出日志，供后续检查。"
      }
    ],
    "tasksImageAlt": "RepoAtlas 任务工作台：已保存任务、进程监控与实时终端",
    "libraryTitle": "项目检索与管理",
    "libraryBody": "支持单独添加项目，或通过已授权的扫描根目录批量发现项目。可按名称、路径和技术栈检索，并使用收藏与项目集合进行分类。",
    "libraryCells": [
      {
        "title": "项目扫描与技术栈识别",
        "body": "根据本地文件识别语言、框架、包管理器和模块。目录变更后可手动重新扫描。",
        "mono": "项目名称 · 路径 · 语言 · 框架"
      },
      {
        "title": "Git 状态与改动",
        "body": "查看分支、文件差异和提交历史，按文件暂存并提交，使用系统 Git 凭据拉取和推送。"
      },
      {
        "title": "目录浏览与文件预览",
        "body": "浏览项目目录、搜索文件路径，以只读方式预览 README、源代码、配置文件和图片。"
      },
      {
        "title": "工作台与最近记录",
        "body": "汇总待处理事项、最近项目、Agent 会话和任务运行记录，提供对应的项目与会话入口。"
      },
      {
        "title": "编辑器、终端与编码 Agent",
        "body": "在指定项目目录启动已安装的开发工具。编码 Agent 可通过 MCP 管理项目记录并请求运行任务。"
      }
    ],
    "safetyTitle": "本地数据与操作权限",
    "safetyBody": "项目记录、设置和任务历史保存在本机。项目扫描由用户或 Agent 发起，范围限于已授权的扫描根目录。",
    "safetyRules": [
      "项目库支持离线访问，无需注册 RepoAtlas 账号。",
      "移除项目或集合仅更改管理记录，不删除磁盘上的项目文件。",
      "Git 拉取仅支持快进合并，SVN 仅提供只读操作。",
      "MCP 任务请求须经桌面端批准后执行。",
      "编码 Agent 独立管理账号、模型配置和对话。"
    ],
    "closeTitle": "下载与源码",
    "closeBody": "Windows 安装包通过 GitHub Releases 提供。项目采用 MIT 许可证，源码、安装说明和贡献指南均在 GitHub 仓库中维护。",
    "closePrimary": "查看源码",
    "closeSecondary": "参与贡献",
    "footerBlurb": "0.1.7 · Windows 安装包未签名 · macOS 实验版、未经公证 · 更新包经签名验证 · MIT 许可证",
    "footerSecurity": "安全策略"
  }
};
