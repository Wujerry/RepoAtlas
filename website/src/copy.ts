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
    "dir": "ltr",
    "title": "RepoAtlas: local projects, Git, and development tasks",
    "description": "Find local projects, inspect files and Git changes, run development tasks, and review logs in one desktop app.",
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
        "text": "Run, build, and check.",
        "accent": true
      }
    ],
    "heroSub": "Find projects by name, path, or technology. Open your editor, inspect Git changes, preview files, and run saved development tasks from the same window.",
    "ctaPrimary": "View releases",
    "ctaSecondary": "Explore tasks",
    "heroImageAlt": "RepoAtlas project library showing folders, project details, Git status, and launch buttons",
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
        "title": "Collections and recent projects",
        "body": "Group related projects, mark favorites, and return to recently opened work. Failed tasks and unavailable projects appear in the attention center."
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
    "footerBlurb": "Windows x64 beta · macOS testing pending · MIT license",
    "footerSecurity": "Security policy"
  },
  "zh": {
    "lang": "zh-CN",
    "dir": "ltr",
    "title": "RepoAtlas：本地项目管理、Git 与开发任务",
    "description": "集中查找本地项目，查看文件和 Git 改动，运行开发任务，保留日志与运行记录。",
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
    "heroHeadline": [
      {
        "text": "本地项目，"
      },
      {
        "text": "集中管理。"
      },
      {
        "text": "开发到打包，"
      },
      {
        "text": "点击运行。",
        "accent": true
      }
    ],
    "heroSub": "按名称、路径和技术栈查找项目，打开编辑器和终端，查看 Git 改动与项目文件。常用开发命令保存为任务，下次打开即可运行。",
    "ctaPrimary": "查看发布版本",
    "ctaSecondary": "查看任务功能",
    "heroImageAlt": "RepoAtlas 项目库：文件夹目录、项目详情、Git 状态与工具启动按钮",
    "marqueeAria": "可保存为任务的常用命令示例",
    "marqueeCommands": marqueeCommands,
    "tasksTitle": "常用命令存成任务，开发、测试、打包直接运行。",
    "tasksBody": "每个项目保存自己的开发任务，工作台可并排运行多个终端。查看实时输出、输入交互指令，也可以随时停止任务。",
    "tasksFacts": [
      {
        "key": "任务配置",
        "title": "保存命令和运行目录",
        "body": "为每个任务设置程序、参数和工作目录，切换项目后仍能使用各自的配置。"
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
        "body": "按运行记录查看成功或失败状态、耗时、退出码和日志，方便回查。"
      }
    ],
    "tasksImageAlt": "RepoAtlas 任务工作台：已保存任务、进程监控与实时终端",
    "libraryTitle": "找到项目，接着做。",
    "libraryBody": "添加扫描根目录，发现其中的项目；也可以单独添加一个项目。项目库支持搜索、筛选、收藏和集合分组。",
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
        "title": "项目集合与最近访问",
        "body": "把相关项目放进集合，收藏常用项目，从最近访问继续工作。失败任务和不可用项目集中显示在待处理中心。"
      },
      {
        "title": "编辑器、终端与编码 Agent",
        "body": "在项目目录打开已安装的工具。外部 Agent 可通过 MCP 管理项目记录、申请运行任务。"
      }
    ],
    "safetyTitle": "项目资料保存在本机。",
    "safetyBody": "项目记录、设置和任务历史保存在本地，扫描范围由你选择。",
    "safetyRules": [
      "无需注册账号，离线也能打开项目库。",
      "移除项目或集合只删除管理记录，磁盘上的项目文件保留。",
      "Git 拉取仅允许快进合并，SVN 提供只读查看。",
      "MCP 发起的任务运行请求需要在桌面端审批。",
      "编码 Agent 使用自己的账号、模型和对话。"
    ],
    "closeTitle": "从存放项目的目录开始。",
    "closeBody": "添加扫描根目录，查看发现的项目，再保存常用开发命令。源码和安装说明都在 GitHub。",
    "closePrimary": "查看源码",
    "closeSecondary": "参与贡献",
    "footerBlurb": "Windows x64 Beta · macOS 待实机验证 · MIT 开源",
    "footerSecurity": "安全策略"
  }
};
