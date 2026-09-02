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
    title: "RepoAtlas: manage your past projects, run dev and packaging tasks",
    description:
      "Gather repositories scattered across drives into one library. Dev, test, and packaging tasks run with live output, and the exit code and log stay local.",
    skip: "Skip to content",
    homeAria: "RepoAtlas home",
    navAria: "Primary navigation",
    navTasks: "Tasks",
    navLibrary: "Library",
    navSafety: "Boundaries",
    navLang: "中文",
    navLangHref: "./zh/",
    navLangLang: "zh-CN",
    navSource: "GitHub",
    heroHeadline: [
      { text: "Years of projects, one library." },
      { text: "Dev, test, packaging," },
      { text: "run right here.", accent: true },
    ],
    heroSub:
      "One library for your repositories and saved tasks. Run them here and watch the output stream.",
    ctaPrimary: "Download the pre-release",
    ctaSecondary: "See tasks run",
    heroImageAlt:
      "The RepoAtlas library view: projects grouped by location with branch and dirty-file status",
    marqueeAria: "Sample commands you can save and run as tasks",
    marqueeCommands,
    tasksTitle: "Tasks run in the open, and every run leaves a record.",
    tasksBody:
      "A task is a program plus its arguments, not a hand-built shell string. It runs in a real PTY, output streams as it happens, and the exit code and full log stay with the run. Shell mode exists as a clearly marked, higher-risk option. It is not the default.",
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
        key: "exit + log",
        title: "Exit code and log, kept with the run",
        body: "Come back days later to work out why that build failed. The record is still there.",
      },
      {
        key: "agent asks",
        title: "Agents ask before they act",
        body: "An MCP task request becomes a time-bounded Pending Approval on your desktop. No approval, no run.",
      },
    ],
    tasksImageAlt:
      "The RepoAtlas task workbench: saved tasks on the left, program details and live output on the right",
    libraryTitle: "Projects scattered across drives, gathered into one library.",
    libraryBody:
      "Name a Scan Root and scanning only happens there. Projects group and sort the way folders do, searchable in English and Chinese, with branches, dirty files, and runtimes up front.",
    libraryCells: [
      {
        title: "Scan Root is the boundary",
        body: "Only directories you name get scanned. No disk crawl, no filesystem watchers, no silent symlink walks.",
        mono: "Scan Root: D:\work\legacy-app",
      },
      {
        title: "Repository lineage",
        body: "Multiple checkouts of the same repository are recognized and linked. A module can be promoted to its own project.",
      },
      {
        title: "Conservative Git",
        body: "Status, diffs, history, and fast-forward pulls. Destructive checkout and reset are not in the vocabulary.",
      },
      {
        title: "Project knowledge",
        body: "Notes, detected facts, and summaries stay with the checkout. Writing AI Memory needs your confirmation.",
      },
      {
        title: "MCP channel",
        body: "Agents read project knowledge over stdio, inside the same boundaries you work in.",
      },
    ],
    safetyTitle: "It removes a record, never your folders.",
    safetyBody: "These are not slogans. They are code in the core.",
    safetyRules: [
      "All data lives in local SQLite. No account, no sync server.",
      "Git operations are typed, and pulls are fast-forward only.",
      "MCP cannot write Git, evaluate shell strings, run arbitrary commands, or delete files.",
      "Before AI is contacted, you preview an explicit list of the evidence.",
    ],
    closeTitle: "Start with one folder that doesn't matter.",
    closeBody:
      "Add a Scan Root, save a task, run it once. Then judge the record against what you meant to do.",
    closePrimary: "View source",
    closeSecondary: "Contribute",
    footerBlurb: "Local-first, open source, Windows & macOS",
    footerSecurity: "Security policy",
  },
  zh: {
    lang: "zh-CN",
    dir: "ltr",
    title: "RepoAtlas：管理历史项目，就地执行开发打包任务",
    description:
      "把散在各个盘里的代码仓库收进一个项目库。开发、测试、打包点开就跑，输出实时滚动，退出码和日志留在本地。",
    skip: "跳到主要内容",
    homeAria: "RepoAtlas 中文首页",
    navAria: "主导航",
    navTasks: "任务执行",
    navLibrary: "项目库",
    navSafety: "边界",
    navLang: "English",
    navLangHref: "../",
    navLangLang: "en",
    navSource: "GitHub",
    heroHeadline: [
      { text: "历史项目，" },
      { text: "集中管理。" },
      { text: "从开发到打包，" },
      { text: "点一下就跑。", accent: true },
    ],
    heroSub:
      "本机仓库收进一个项目库，状态、笔记、保存的任务都在项目旁边，点了就跑，输出实时滚动。",
    ctaPrimary: "下载预发布版",
    ctaSecondary: "看任务怎么跑",
    heroImageAlt:
      "RepoAtlas 项目库界面截图：按位置分组的项目列表，标着分支和未提交改动",
    marqueeAria: "可以保存成任务反复运行的命令示例",
    marqueeCommands,
    tasksTitle: "任务在明处跑，结果留下来。",
    tasksBody:
      "任务是程序加参数，不是随手拼的 shell 字符串。跑在真实的 PTY 里，输出实时滚动；跑完，退出码和完整日志跟这次运行存在一起。Shell 是明确标出的高危模式，不是默认。",
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
        key: "exit + log",
        title: "退出码和日志随运行存档",
        body: "过两天回头查那次构建为什么挂，记录还在原地。",
      },
      {
        key: "agent asks",
        title: "Agent 先请示，再动手",
        body: "MCP 发来的任务请求变成桌面上有时限的待审批，你不批，它不跑。",
      },
    ],
    tasksImageAlt:
      "RepoAtlas 任务工作台截图：左边是保存的任务，右边是程序、参数和实时输出",
    libraryTitle: "散在各个盘里的项目，收进一本项目库。",
    libraryBody:
      "指定一个 Scan Root，扫描只发生在这里面。项目按文件夹的样子分组排序，中英文都能搜，分支、未提交改动、运行时一眼看清。",
    libraryCells: [
      {
        title: "Scan Root 就是边界",
        body: "只有你明确指定的目录会被扫描。不全盘爬取，不监听文件系统，不偷偷跟进符号链接。",
        mono: "Scan Root: D:\work\legacy-app",
      },
      {
        title: "仓库血缘",
        body: "同一个仓库的多个 checkout 会被认出来并互相关联，模块也能提升成独立项目。",
      },
      {
        title: "保守的 Git",
        body: "状态、差异、历史、fast-forward 拉取。没有破坏性的 checkout 和 reset。",
      },
      {
        title: "项目知识",
        body: "笔记、检测到的事实、摘要都挂在项目上。写进 AI Memory 之前，需要你确认。",
      },
      {
        title: "MCP 通道",
        body: "Agent 通过 stdio 读项目知识，跑在同一套边界里，越不了界。",
      },
    ],
    safetyTitle: "删掉的是记录，不是你的文件夹。",
    safetyBody: "这些不是宣传语，是核心逻辑里的代码。",
    safetyRules: [
      "数据全在本地 SQLite，没有账号，没有同步服务器。",
      "Git 只做类型化操作，拉取仅限 fast-forward。",
      "MCP 不能执行 Git 写入、shell 求值、任意命令和文件删除。",
      "AI 出手之前，先给你一份明确的证据清单预览。",
    ],
    closeTitle: "拿一个不重要的目录，先跑起来。",
    closeBody:
      "给一个 Scan Root，存一个任务，让它跑一遍。留下的记录和你本来的意图对不对得上，一试便知。",
    closePrimary: "查看源码",
    closeSecondary: "参与贡献",
    footerBlurb: "本地优先，开源，支持 Windows 和 macOS",
    footerSecurity: "安全策略",
  },
};
