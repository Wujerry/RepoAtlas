<p align="center">
  <img src="assets/brand/repoatlas-mark.png" width="96" height="96" alt="RepoAtlas 标志" />
</p>

<h1 align="center">RepoAtlas</h1>

<p align="center">
  <a href="README.md">English</a>
</p>

<p align="center">
  <a href="https://github.com/wujer/RepoAtlas/actions/workflows/ci.yml"><img src="https://github.com/wujer/RepoAtlas/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI 状态" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-FFA31A.svg" alt="MIT License" /></a>
</p>

RepoAtlas 把散在各个盘里的代码项目收进一个本地库。存好的任务点开就跑，输出实时滚动，跑完留下退出码和完整日志。所有数据都在本机：一个本地 SQLite，没有账号，没有同步服务器。


## 项目库

![RepoAtlas 项目库界面：按位置分组的项目列表，标着分支和未提交改动](assets/screenshots/zh-dark-library.png)

你授权一个 **Scan Root**（一个目录），扫描只发生在这里面。不全盘爬取，不监听文件系统，不偷偷跟进符号链接。每个规范的本地 checkout 是一个 **Project**。同一个仓库的多个 checkout 通过 **Repository Lineage** 互相关联，模块也能提升成独立项目。

项目按文件夹的样子分组排序，中英文都能搜，分支、未提交改动、检测到的运行时直接摆在列表上。

## 任务

![RepoAtlas 任务工作台：左边是保存的任务，右边是程序、参数和实时输出](assets/screenshots/en-light-tasks.png)

一个 **Task Definition** 是程序、参数向量和工作目录，不是随手拼的 shell 字符串。跑在真实的 PTY 里，输出一行行实时出来；每次 **Task Run** 都把退出码和完整日志存下来。停掉任务会终止整个进程树。Shell 模式是明确标出的高危选项，不是默认。

## Git、知识和 MCP

- **保守的 Git**：状态、差异、历史、fast-forward 拉取。没有破坏性的 checkout 和 reset。SVN 目前只读。
- **项目知识**：笔记、检测到的事实、AI 摘要都挂在项目上。写进 AI Memory 之前需要你确认。
- **MCP**：Agent 通过 stdio 读项目知识，跑在同一套边界里。Agent 发来的任务请求变成桌面上有时限的 Pending Approval——你不批，它不跑。

## 边界

这些规则写进了 `repoatlas-core` 的代码里：

- 所有数据都在本地 SQLite，没有账号，没有同步服务器。
- 只有你明确授权的 Scan Root 会被扫描，而且由你手动触发。
- 删掉 Project 或 Scan Root 删的是记录，不是你的文件夹。
- Git 操作都是类型化的；拉取仅限 fast-forward。
- MCP 不能执行 Git 写入、shell 求值、任意命令和文件删除。
- 你预览并确认证据清单之前，AI 看不到项目的任何内容。

## 下载（Windows x64 Beta）

1. 从 [Releases](https://github.com/wujer/RepoAtlas/releases) 下载最新安装包。
2. 校验 SHA-256，和发布时附带的 `SHA256SUMS` 对比：

   ```powershell
   Get-FileHash .\RepoAtlas_0.1.0_x64-setup.exe -Algorithm SHA256
   ```

3. 运行安装程序。SmartScreen 会提示「无法识别的发布者」——未签名构建的正常现象。点 **更多信息**，再点 **仍要运行**。

macOS 安装包暂时没有，等真机验证之后发布。

## 从源码运行

需要 Node.js 24.11 以上（25 以下）、pnpm 10.20.0、Rust stable，以及平台的 [Tauri 2 依赖](https://v2.tauri.app/start/prerequisites/)。

```sh
pnpm install
pnpm tauri dev
```

首次启动按引导对话框走：连接 MCP 客户端，调用 `register_project` 授权第一个 Scan Root，项目就进了库。

## 架构和开发

- `src/` — React 19 + TypeScript 桌面界面。`src/lib/api.ts` 是类型化的 Tauri 边界。
- `src-tauri/` — Tauri 2 命令和桌面集成。
- `crates/repoatlas-core/` — 权威核心：SQLite、扫描、Git、任务、审计、AI Provider。
- `crates/repoatlas-mcp/` — stdio MCP 适配器。复用 `repoatlas-core`，不建并行的持久化路径。

常用检查：

```sh
pnpm typecheck
pnpm test -- --run
cargo test -p repoatlas-core
cargo check -p repoatlas
pnpm build
```

见 [CONTRIBUTING.md](CONTRIBUTING.md)、[CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)、[SECURITY.md](SECURITY.md) 和 [docs/releasing.md](docs/releasing.md)。

## 许可证

[MIT](LICENSE)
