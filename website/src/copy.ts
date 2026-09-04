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
  en: {
    lang: "en",
    dir: "ltr",
    title: "RepoAtlas: let your coding Agent build a local project library",
    description:
      "Let your coding Agent inventory approved directories, then run and review repeatable development tasks in one local project library.",
    skip: "Skip to content",
    homeAria: "RepoAtlas home",
    navAria: "Primary navigation",
    navTasks: "Tasks",
    navLibrary: "Library",
    navSafety: "Local rules",
    navLang: "中文",
    navLangHref: "./zh/",
    navLangLang: "zh-CN",
    navSource: "GitHub",
    heroHeadline: [
      { text: "Let your Agent build the library." },
      { text: "Run every task locally.", accent: true },
    ],
    heroSub:
      "Copy one onboarding instruction. Your coding Agent connects RepoAtlas MCP, asks what it may scan, then adds evidence-backed Project descriptions, tasks, and icons. You take over from a working local library.",
    ctaPrimary: "Download the pre-release",
    ctaSecondary: "See tasks run",
    heroImageAlt:
      "The RepoAtlas library view with project collections, branch and dirty-file status, and the attention center",
    marqueeAria: "Sample commands you can save and run as tasks",
    marqueeCommands,
    tasksTitle: "Tasks run in the open, and every run leaves a record.",
    tasksBody:
      "A task is a program plus its arguments, not a hand-built shell string. It runs in a real PTY, output streams as it happens, and the exit code and full log stay with the run. Development tasks can declare expected ports so RepoAtlas can show process-tree CPU, memory, listening ports, and safe localhost previews.",
    tasksFacts: [
      {
        key: "program + args",
        title: "Reviewed before it runs",
        body: "Program, argument vector, working directory: all on the panel beforehand, saveable as a task you reuse.",
      },
      {
        key: "live PTY",
        title: "Output that streams, not batches",
        body: "Long builds and big test suites scroll line by line, so you see exactly where things stick.",
      },
      {
        key: "runtime + ports",
        title: "See what the process tree is doing",
        body: "CPU, memory, child processes, listening ports, and safe localhost previews sit beside the live terminal.",
      },
      {
        key: "exit + log",
        title: "Exit code and log, kept with the run",
        body: "Come back days later to work out why that build failed. The record is still there.",
      },
    ],
    tasksImageAlt:
      "The RepoAtlas task workbench with saved tasks, a runtime monitor, and live terminal output",
    libraryTitle: "The setup work goes to your Agent. The durable records stay here.",
    libraryBody:
      "Start with one open checkout or an approved Scan Root. RepoAtlas turns the result into a searchable library for daily project work, not another chat history.",
    libraryCells: [
      {
        title: "Agent-assisted initialization",
        body: "Your Agent asks which directories it may scan, registers Projects, then writes descriptions, reviewed tasks, and recognizable icons from real checkout evidence.",
        mono: "Ask: scan D:\\work\\legacy-app",
      },
      {
        title: "Collections and Project Briefs",
        body: "Group related Projects without changing their identity, then read one structured view of facts, environment, tasks, runs, and recent activity.",
      },
      {
        title: "Read-only Files",
        body: "Browse and search on demand, preview Markdown and code, and stay inside the Project boundary. It is an inspector, not an editor.",
      },
      {
        title: "Attention center",
        body: "Pending Approvals, failed runs, unavailable Projects, and environment mismatches arrive in one actionable list.",
      },
      {
        title: "External Agent + MCP",
        body: "Open an installed coding Agent at the Project path. MCP supplies structured context; protected execution still waits for desktop approval.",
      },
    ],
    safetyTitle: "It removes a record, never your folders.",
    safetyBody: "These are not slogans. They are code in the core.",
    safetyRules: [
      "All data lives in local SQLite. No account, no sync server.",
      "Removing a Project or Collection changes RepoAtlas records, never the checkout on disk.",
      "Git operations are typed, pulls are fast-forward only, and SVN stays read-only.",
      "MCP cannot write Git, evaluate shell strings, run arbitrary commands, or delete files.",
      "RepoAtlas stores no model credentials and sends no project files to model providers. The external Agent owns those decisions.",
    ],
    closeTitle: "Give the first inventory pass to your Agent.",
    closeBody:
      "Open RepoAtlas, copy the onboarding instruction, and let your Agent register one Project or scan an approved directory. Review the library it builds, then run the first saved task.",
    closePrimary: "View source",
    closeSecondary: "Contribute",
    footerBlurb: "Windows x64 beta · macOS not yet tested · contributors wanted",
    footerSecurity: "Security policy",
  },
  zh: {
    lang: "zh-CN",
    dir: "ltr",
    title: "RepoAtlas：让编码 Agent 帮你建立本地项目库",
    description:
      "让编码 Agent 盘点你授权的目录，再回到本地项目库执行和检查可重复的开发任务。",
    skip: "跳到主要内容",
    homeAria: "RepoAtlas 中文首页",
    navAria: "主导航",
    navTasks: "任务执行",
    navLibrary: "项目库",
    navSafety: "本地规则",
    navLang: "English",
    navLangHref: "../",
    navLangLang: "en",
    navSource: "GitHub",
    heroHeadline: [
      { text: "让 Agent 建好" },
      { text: "本地项目库。" },
      { text: "开发任务，" },
      { text: "留在本地跑。", accent: true },
    ],
    heroSub:
      "复制一段初始化指令，编码 Agent 会连接 RepoAtlas MCP，询问允许扫描的目录，再根据真实依据补齐 Project 描述、任务和图标。打开桌面端时，项目库已经可以工作。",
    ctaPrimary: "下载预发布版",
    ctaSecondary: "看任务怎么跑",
    heroImageAlt:
      "RepoAtlas 项目库界面截图：包含项目集合、分支和未提交改动状态，以及待处理中心",
    marqueeAria: "可以保存成任务反复运行的命令示例",
    marqueeCommands,
    tasksTitle: "任务在明处跑，结果留下来。",
    tasksBody:
      "任务是程序加参数，不是随手拼的 shell 字符串。跑在真实的 PTY 里，输出实时滚动；开发任务还能声明预期端口，RepoAtlas 会把进程树 CPU、内存、监听端口和安全的 localhost 预览放在终端旁边。",
    tasksFacts: [
      {
        key: "program + args",
        title: "先看清楚，再开跑",
        body: "程序、参数向量、工作目录，运行前都摆在面板上，存成任务可以反复用。",
      },
      {
        key: "live PTY",
        title: "输出实时滚动",
        body: "长构建、大测试套件也是一行行出来，卡在哪一眼就看到。",
      },
      {
        key: "runtime + ports",
        title: "进程树在做什么，直接看",
        body: "CPU、内存、子进程、监听端口和安全的 localhost 预览都跟实时终端放在一起。",
      },
      {
        key: "exit + log",
        title: "退出码和日志随运行存档",
        body: "过两天回头查那次构建为什么挂，记录还在原地。",
      },
    ],
    tasksImageAlt:
      "RepoAtlas 任务工作台截图：包含保存的任务、运行监控和实时终端输出",
    libraryTitle: "盘点工作交给 Agent，长期记录留在 RepoAtlas。",
    libraryBody:
      "可以从当前 checkout 开始，也可以授权一个 Scan Root。RepoAtlas 把结果变成日常可用的本地项目库，而不是又一段散落的对话记录。",
    libraryCells: [
      {
        title: "Agent 辅助初始化",
        body: "Agent 先询问允许扫描哪些目录，再登记 Project，并依据真实 checkout 补上描述、可检查的任务和容易识别的图标。",
        mono: "告诉 Agent：扫描 D:\\work\\legacy-app",
      },
      {
        title: "项目集合和 Project Brief",
        body: "相关 Project 可以放进一个集合，但身份和路径不变；结构化简介集中展示事实、环境、任务、运行和最近活动。",
      },
      {
        title: "只读 Files",
        body: "目录和路径按需加载，Markdown 与代码可以直接预览，全程不越过 Project 边界，也不提供编辑。",
      },
      {
        title: "待处理中心",
        body: "待审批任务、运行失败、Project 不可用和环境不匹配集中在一张可操作的清单里。",
      },
      {
        title: "外部 Agent + MCP",
        body: "在 Project 路径打开已安装的编码 Agent。MCP 提供结构化上下文，受保护的执行仍要等桌面审批。",
      },
    ],
    safetyTitle: "删掉的是记录，不是你的文件夹。",
    safetyBody: "这些不是宣传语，是核心逻辑里的代码。",
    safetyRules: [
      "数据全在本地 SQLite，没有账号，没有同步服务器。",
      "删掉 Project 或 Collection 只改 RepoAtlas 记录，不动磁盘上的 checkout。",
      "Git 只做类型化操作，拉取仅限 fast-forward，SVN 维持只读。",
      "MCP 不能执行 Git 写入、shell 求值、任意命令和文件删除。",
      "RepoAtlas 不保存模型凭据，也不把项目文件发给模型 Provider；这些决定由外部 Agent 自己负责。",
    ],
    closeTitle: "第一次项目盘点，直接交给 Agent。",
    closeBody:
      "打开 RepoAtlas，复制初始化指令，让 Agent 登记当前 Project 或扫描你授权的目录。检查它整理出的项目库，再运行第一个保存任务。",
    closePrimary: "查看源码",
    closeSecondary: "参与贡献",
    footerBlurb: "Windows x64 Beta · macOS 尚未真机测试 · 欢迎参与贡献",
    footerSecurity: "安全策略",
  },
};
