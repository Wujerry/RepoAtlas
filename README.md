# RepoAtlas

本地代码资产控制中心。帮助找回、识别和操作多年积累的本地源码项目，而不是做成又一个 Git GUI。

## Current capabilities

当前可运行闭环：

1. 启动后立即从本地 SQLite 缓存恢复项目列表
2. 添加 Scan Root 并手动扫描
3. 识别 Git / SVN / 清单文件项目
4. 虚拟化列表、中英文搜索、收藏 / 标签 / 归档
5. 独立 Project Dashboard
6. 打开 IDE / Terminal / Explorer
7. `Ctrl/Cmd+K` 命令面板

- 任务 Command Broker（结构化 argv、桌面审批、运行日志）
- 本地 Atlas Report、环境探测、Checkout Lineage、AI Summary / Memory / 对话历史
- MCP 项目级完整管理面（含 Scan Root、任务、图标、知识和只读 Git 查询）
- MCP 删除只清理 RepoAtlas 数据和应用日志，永不操作真实项目目录

MCP 的 `remove_project` 必须传入 `confirm: true`；`remove_scan_root` 还必须选择 `orphan` 或 `removeRecords`。两者返回值都会明确给出 `filesystemDeleted: false`。MCP 的 `run_task` 只创建桌面 Pending Approval，不会直接启动进程；Git 写入、Shell Mode、真实文件删除、Provider 凭据与外部 AI 请求均不在 MCP 管理面内。

## Develop

```bash
pnpm install
cargo test -p repoatlas-core
pnpm tauri dev
```

产品数据保存在系统应用数据目录，不会写入被管理的项目目录。移除项目只删除 RepoAtlas 记录，永不删除真实文件夹。
