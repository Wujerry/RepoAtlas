# 本地截图展示数据

截图数据固定放在仓库外的 `C:\RepoAtlas Showcase`。其中只有 8 个虚构 Project，不包含远程仓库地址、账号、凭据、API Key 或个人目录。

## 准备数据

先关闭 RepoAtlas 桌面端和 RepoAtlas MCP 进程，再在仓库根目录运行：

```powershell
.\scripts\seed-showcase.ps1 -Force
```

必须显式传入 `-Force`。脚本只会清理 RepoAtlas 自己的应用数据库（`repoatlas.sqlite`、对应的 `-wal`/`-shm` 文件以及 `task-logs` 下的直接文件），随后重建截图记录。如果 RepoAtlas 仍在运行、`APPDATA` 没有指向 `io.repoatlas.desktop`，或者已有展示目录缺少 `.repoatlas-demo-marker`，脚本会拒绝继续。

脚本会创建真实的本地示例文件，初始化 7 个没有 remote 的本地 Git 仓库；`pulse-mobile` 保留 1 个 staged 和 1 个 unstaged 改动，`ops-playbook` 不使用版本控制。前 4 个 Project 带有独立的本地图标。随后 `repoatlas-demo` 写入 Project Collection、环境检查、Task Run、运行峰值、监听端口、Project Event 和 Pending Approval。数据中不创建 Provider Profile、生成式摘要、AI Memory 或项目对话。

## 截图清单

统一使用 `1600 × 1000` 的 RepoAtlas 窗口。搜索框保持为空；截图前关闭设置、帮助、菜单和 tooltip。原始 PNG 全部保存到 `assets/screenshots/`，通过隐私检查后再同步到 GitHub Pages。

### 项目库

4 张项目库截图都选择 `Atlas Dashboard`，停留在「概览」页。左侧应能看到不同的项目图标，并保留 Project Collection 和待处理入口。

| 文件名 | 截图状态 |
| --- | --- |
| `en-dark-library.png` | 英文、深色主题。 |
| `en-light-library.png` | 英文、浅色主题；构图和深色版一致。 |
| `zh-dark-library.png` | 中文、深色主题；构图和英文版一致。 |
| `zh-light-library.png` | 中文、浅色主题；构图和英文版一致。 |

### 任务工作台

任务数据已经预置。先进入 `Atlas Dashboard` 的「任务」页，启动 `Start local preview`（中文界面中对应「启动本地预览」）；再进入 `Pulse Mobile` 的「任务」页，启动 `Start mobile preview`（中文界面中对应「启动移动端预览」）。然后打开标题栏的「运行中任务 / Active Tasks」，等到以下内容都出现后再截图：

- 工作台同时显示 Atlas Dashboard 和 Pulse Mobile 两个运行中任务；
- 两个实时终端都显示本地 HTTP 服务已经启动；
- 两条运行监控都显示 CPU、内存和进程数；
- Atlas Dashboard 的监听端口显示 `4173`，并提供 localhost 预览入口；
- Pulse Mobile 的终端显示 Vite 提供的 localhost 地址；
- 工作台中没有第三条运行中任务。

| 文件名 | 截图状态 |
| --- | --- |
| `en-light-tasks.png` | 英文、浅色主题，任务工作台和实时终端完整可见。 |
| `zh-light-tasks.png` | 中文、浅色主题，保持相同任务和构图。该图完成后会替换中文 README 和中文 Pages 当前暂用的英文任务图。 |

两张任务工作台截图完成后，停止 `Start local preview` 和 `Start mobile preview`。

## 截图前检查

每张图都要按原始尺寸检查：路径只能来自 `C:\RepoAtlas Showcase`，名称必须是虚构项目，不能出现其他应用通知、个人目录、加载状态、菜单、tooltip 或鼠标悬停态。

## 安全说明

脚本不会删除、移动或修改 `C:\RepoAtlas Showcase` 之外的任何项目。目录内也只会在 marker 校验通过后重建 8 个固定 fixture。fixture 和 RepoAtlas 展示逻辑不会主动发起网络请求；首次执行 `cargo run` 时，如果构建依赖尚未缓存，Cargo 仍可能联网下载依赖。脚本不写入秘密，也不会把展示入口接到桌面命令、MCP 工具或发布产物中。

只清理 RepoAtlas 应用数据、不重建 fixture 时运行：

```powershell
.\scripts\clear-repoatlas-data.ps1 -Force
```

这个命令不会删除 `C:\RepoAtlas Showcase` 或任何真实项目目录。
