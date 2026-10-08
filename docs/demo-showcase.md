# 宣传截图与隔离展示数据

README 和 GitHub Pages 使用同一套桌面实拍，尺寸为 **1600 × 1000**。中英文分别准备描述、会话和任务名称；首页和项目概览都有深浅主题。项目、会话、Token 计数、订阅额度和历史任务结果是虚构数据；任务工作台中的两个 HTTP 服务实际在本机运行。

## 数据边界

- 展示实例使用独立应用标识 **io.repoatlas.marketing**，数据库位于 `%APPDATA%\io.repoatlas.marketing\repoatlas.sqlite`，不替换日常使用的 `io.repoatlas.desktop` 数据库。
- 项目只使用带 `.repoatlas-demo-marker` 的 `C:\RepoAtlas Showcase` 下的 8 个 fixture：7 个活动 Project、1 个归档 Project、2 个 Project Collection。
- `scripts/prepare-marketing.mjs` 配置中英文介绍、开发任务及 6 条 Codex / Claude 虚构会话。历史文件位于 `C:\RepoAtlas Showcase\marketing-history`，通过真实 Core 索引读取。
- 会话包含演示 Token 计数及模型名；金额由现有产品逻辑折算，不代表真实消耗或账单。
- 导航栏展示 Codex、Claude、GitHub Copilot、OpenCode Go 的虚构剩余额度。截图脚本写入有效缓存，并在采集期间每分钟更新时间，因此普通自动刷新不会读取凭据或访问服务商。**不要点击强制刷新或连接真实账号。** 脚本退出时移除演示连接；强制中断后应关闭展示实例并清理其演示连接，避免缓存过期后查询真实账户。

## 准备独立实例

复用已经标记的本地 fixture，无需重建 Git 仓库。旧版 `scripts/seed-showcase.ps1 -Force` 会清空它所指向的应用数据，不能直接对日常配置运行。

使用开发版 seeder 向临时 APPDATA 写入初始数据，再复制到尚不存在的隔离数据库。环境变量只在当前 PowerShell 进程修改，执行后恢复：

```powershell
$seedRoot = Join-Path $PWD 'output/marketing-seed'
$savedAppData = $env:APPDATA
$destination = Join-Path $savedAppData 'io.repoatlas.marketing/repoatlas.sqlite'
if (Test-Path -LiteralPath $destination) { throw '已有展示数据库，请先检查，不自动覆盖。' }
try {
  $env:APPDATA = $seedRoot
  cargo run -p repoatlas-core --bin repoatlas-demo -- --db "$seedRoot/io.repoatlas.desktop/repoatlas.sqlite" --showcase 'C:\RepoAtlas Showcase'
  if ($LASTEXITCODE -ne 0) { throw '展示数据初始化失败' }
} finally { $env:APPDATA = $savedAppData }
New-Item -ItemType Directory -Path (Split-Path $destination) -Force | Out-Null
Copy-Item -LiteralPath "$seedRoot/io.repoatlas.desktop/repoatlas.sqlite" -Destination $destination
```

另一个终端运行 `pnpm dev`；若 1420 已有当前仓库的 Vite 则复用。用单独的 Cargo 输出目录编译展示实例，避免覆盖运行中的正常程序，同时保留增量编译：

```powershell
$env:CARGO_TARGET_DIR = Join-Path $PWD 'output/marketing-build'
$env:WEBVIEW2_USER_DATA_FOLDER = Join-Path $PWD 'output/marketing-webview'
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9337'
pnpm exec tauri dev --no-watch --config '{"identifier":"io.repoatlas.marketing","build":{"beforeDevCommand":""}}'
```

## 自动采集

需要 Node 24 和 Playwright。截图工具可以安装在被 Git 忽略的目录，不改动产品依赖：

```powershell
npm install --prefix output/marketing-tools playwright
$env:PLAYWRIGHT_MODULE_PATH = (Resolve-Path output/marketing-tools/node_modules/playwright).Path
node scripts/capture-marketing.mjs en
node scripts/capture-marketing.mjs zh-CN
```

脚本连接本机 9337 的真实 Tauri WebView，核对应用标识后才操作。依次准备内容、索引会话、切换语言与主题、截图，并将相同文件复制到 `website/assets/screenshots/`。不替换产品 API 或伪造界面元素。采集清单保存在 `output/marketing-2026-10-08/`。

通过 Playwright 渲染层截图，**不包含系统鼠标指针**；采集前把指针移到边缘、移除输入焦点、隐藏文本光标，等待字体与过渡完成。不进行涂抹、拼接或修图。

| 文件 | 内容 |
| --- | --- |
| `{en,zh}-{dark,light}-workspace.jpg` | 首页：导航额度、最近会话、项目与任务记录 |
| `{en,zh}-{dark,light}-library.png` | Atlas Dashboard 概览与项目库 |
| `en-dark-sessions.jpg` / `zh-light-sessions.jpg` | 跨 Agent 会话列表与 Markdown 消息 |
| `en-dark-usage.jpg` / `zh-light-usage.jpg` | 导航订阅额度、周期与重置倒计时 |
| `en-dark-tokens.jpg` / `zh-light-tokens.jpg` | 会话 Token 明细与美元等值估算 |
| `en-dark-search.jpg` / `zh-light-search.jpg` | Ctrl+K 统一搜索与匹配消息 |
| `{en,zh}-light-tasks.png` | 两个真实预览任务、终端与运行指标 |

预览任务监听 `127.0.0.1:4317` 和 `127.0.0.1:4318`。端口冲突时失败，不自动终止其他进程。脚本检查 HTTP 响应后截图，退出时停止自己启动的 Task Run。

## 发布前检查

1. 逐张检查原图：四家服务及百分比可见，无鼠标、个人路径、真实账号、加载遮罩、错误或无关菜单。额度及 Token 明细图允许对应功能面板展开。
2. 确认中英文对应、深浅主题构图一致、终端和端口就绪。
3. 核对 README / Pages 引用存在，两份图片 SHA-256 相同；删除不再引用的旧截图。
4. 运行 `pnpm exec tsc -p website/tsconfig.json --noEmit` 和 `pnpm exec vite build --config website/vite.config.ts`，检查中英文网页的桌面与手机布局。

只提交源文案、脚本和审核后的截图。output、WebView 配置、数据库、任务日志与本地构建产物不提交。提交发布按当前任务授权范围处理。
