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
  resumeImageAlt: string;
  resumeImageLabel: string;
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
    "resumeImageAlt": "RepoAtlas resume dialog showing CLI and App choices, a working directory and a session command",
    "resumeImageLabel": "See the resume options",
    "showcaseCaption": "Captured from the desktop app with fictional demonstration data.",
    "navSessions": "Sessions",
    "sessionsTitle": "Find the conversation. Continue the work.",
    "sessionsBody": "Search coding sessions across Agents in one place. Preview the conversation, find its Project, and reopen the selected session in the original Agent.",
    "sessionsSteps": [{"title": "Authorize your history sources", "body": "Review the absolute directories individually or use Authorize all. You choose which histories RepoAtlas indexes locally and exposes to connected MCP clients."}, {"title": "Search and preview", "body": "Search across Agents, or narrow by Project, date and archive status. Read matching messages before deciding which conversation to continue."}, {"title": "Resume the selected session", "body": "Review the command and working directory. Choose CLI or a supported App entry, then continue with the original Agent and session ID."}],
    "sessionsAgents": "Claude Code · Codex CLI · OpenCode · Cursor CLI · Gemini CLI · GitHub Copilot CLI · Kimi Code · Qwen Code",
    "sessionsNote": "History access is read-only. Revoking a source clears its local index without deleting Agent history. Resume requires the installed Agent and an available session; accounts and model access stay with the Agent.",
    "dir": "ltr",
    "title": "RepoAtlas: projects, Agent sessions, and development tasks",
    "description": "Find local projects, search Agent sessions, resume coding conversations, and run development tasks in one desktop app.",
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
    "heroSub": "Open a Project, resume a coding session, or run a saved task. Keep your local projects, Agent history, Git changes, and development tools within reach.",
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
    "footerBlurb": "0.1.2 · Windows UNSIGNED · macOS experimental, not notarized · Signed updates · MIT",
    "footerSecurity": "Security policy"
  },
  "zh": {
    "lang": "zh-CN",
    "sessionsImageAlt": "RepoAtlas Sessions：跨 Agent 会话列表与选中对话的消息预览",
    "resumeImageAlt": "RepoAtlas 恢复面板：CLI 与 App 选择、工作目录和指定会话命令",
    "resumeImageLabel": "查看会话恢复选项",
    "showcaseCaption": "桌面应用截图，项目与会话内容均为演示数据。",
    "navSessions": "Sessions",
    "sessionsTitle": "搜索历史对话，恢复编码会话",
    "sessionsBody": "统一搜索 Claude Code、Codex 等 Agent 的本地会话记录。查看对话内容和关联项目，选择需要恢复的会话，在原 Agent 中打开。",
    "sessionsSteps": [{"title": "授权历史来源", "body": "确认需要读取的历史目录，可逐项授权或全部授权。授权后建立本地搜索索引，已连接的 MCP 客户端也可读取这些会话。"}, {"title": "搜索并预览对话", "body": "跨 Agent 搜索会话内容，按项目、日期和归档状态筛选结果。选中会话可预览消息，不会直接启动 Agent。"}, {"title": "恢复指定会话", "body": "确认启动命令和工作目录，选择 CLI 或受支持的桌面应用，恢复指定会话。启动命令也可复制到终端执行。"}],
    "sessionsAgents": "Claude Code · Codex CLI · OpenCode · Cursor CLI · Gemini CLI · GitHub Copilot CLI · Kimi Code · Qwen Code",
    "sessionsNote": "历史读取不修改原文件。撤销来源会清理本地索引，不删除 Agent 历史。恢复需要已安装的 Agent 和可用会话；账号、模型及访问权限由 Agent 管理。",
    "dir": "ltr",
    "title": "RepoAtlas：本地项目、Agent 会话与开发任务",
    "description": "管理本地开发项目，搜索和恢复 Agent 会话，运行常用任务，查看 Git 改动与执行记录。",
    "skip": "跳到主要内容",
    "homeAria": "RepoAtlas 中文首页",
    "navAria": "主导航",
    "navTasks": "任务执行",
    "navLibrary": "项目管理",
    "navSafety": "本地数据",
    "navLang": "English",
    "navLangHref": "../",
    "navLangLang": "en",
    "navSource": "GitHub",
    "heroHeadline": [{"text": "本地开发项目，"}, {"text": "统一管理。"}, {"text": "Agent 编码会话，"}, {"text": "搜索与恢复。", "accent": true}],
    "heroSub": "RepoAtlas 是一款本地开发项目管理工具。按名称、路径或技术栈查找项目，搜索并恢复 Agent 会话，运行开发任务，查看 Git 改动。",
    "ctaPrimary": "查看发布版本",
    "ctaSecondary": "了解 Sessions",
    "heroImageAlt": "RepoAtlas 首页工作台：最近 Agent 会话、项目图标、最近项目与任务运行",
    "marqueeAria": "可保存为任务的常用命令示例",
    "marqueeCommands": marqueeCommands,
    "tasksTitle": "保存常用命令，统一管理开发任务",
    "tasksBody": "为项目配置启动、测试和打包任务，无需反复输入命令或切换目录。支持同时运行多个任务，在终端中查看输出、输入指令或停止进程。",
    "tasksFacts": [
      {
        "key": "任务配置",
        "title": "保存命令和运行目录",
        "body": "每个项目独立保存任务配置，包含可执行程序、参数和工作目录。"
      },
      {
        "key": "实时终端",
        "title": "查看输出，输入指令",
        "body": "构建和测试输出实时显示，支持交互输入和停止运行。"
      },
      {
        "key": "运行监控",
        "title": "查看资源占用与端口",
        "body": "显示 CPU、内存、子进程和监听端口，可从本地开发地址打开预览页面。"
      },
      {
        "key": "运行记录",
        "title": "保留日志和退出码",
        "body": "记录每次运行的状态、耗时、退出码和日志，便于定位失败原因。"
      }
    ],
    "tasksImageAlt": "RepoAtlas 任务工作台：已保存任务、进程监控与实时终端",
    "libraryTitle": "按项目组织代码、工具和运行记录",
    "libraryBody": "添加存放代码的目录作为扫描根目录，或单独添加一个项目。通过名称、路径和技术栈查找项目，用收藏和集合整理常用项目。",
    "libraryCells": [
      {
        "title": "项目发现与技术栈识别",
        "body": "识别语言、框架、包管理器和项目模块。目录有变化时，手动重新扫描即可更新。",
        "mono": "项目名称 · 路径 · 语言 · 框架"
      },
      {
        "title": "Git 状态与改动",
        "body": "查看当前分支、改动文件和差异，按文件暂存并提交，使用已有 Git 凭据拉取和推送。"
      },
      {
        "title": "目录浏览与文件预览",
        "body": "展开目录、搜索路径，直接阅读 Markdown、源代码和图片，快速查看项目内容。"
      },
      {
        "title": "工作台与最近项目",
        "body": "首页显示最近使用的项目、可恢复的会话和任务运行记录。点击项目名称可进入项目，查看配置、文件和 Git 状态。"
      },
      {
        "title": "编辑器、终端与编码 Agent",
        "body": "在项目目录打开已安装的工具。外部 Agent 可通过 MCP 管理项目记录、申请运行任务。"
      }
    ],
    "safetyTitle": "数据保存在本地，操作范围由你控制",
    "safetyBody": "项目记录、设置和任务历史保存在本机。扫描只在你授权的目录内进行，移除管理记录不会删除项目文件。",
    "safetyRules": [
      "无需注册账号，离线也能打开项目库。",
      "移除项目或集合只删除管理记录，磁盘上的项目文件保留。",
      "Git 拉取仅允许快进合并，SVN 提供只读查看。",
      "MCP 发起的任务运行请求需要在桌面端审批。",
      "编码 Agent 使用自己的账号、模型和对话。"
    ],
    "closeTitle": "安装 RepoAtlas，添加本地项目",
    "closeBody": "在 GitHub 下载 Windows 安装包，按安装说明启动应用后添加项目。源码、使用说明和问题反馈入口也在仓库中。",
    "closePrimary": "查看源码",
    "closeSecondary": "参与贡献",
    "footerBlurb": "0.1.2 · Windows 未签名 · macOS 实验版、未公证 · 更新包签名验证 · MIT 开源",
    "footerSecurity": "安全策略"
  }
};
