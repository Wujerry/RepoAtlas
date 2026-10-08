// Synthetic content for the isolated io.repoatlas.marketing desktop only.
import fs from 'node:fs';
import path from 'node:path';
import { DatabaseSync } from 'node:sqlite';

const locale = process.argv[2] ?? 'en';
if (!['en', 'zh-CN'].includes(locale)) throw new Error('Use en or zh-CN');
const zh = locale === 'zh-CN';
const root = 'C:\\RepoAtlas Showcase';
if (!fs.readFileSync(path.join(root, '.repoatlas-demo-marker'), 'utf8').includes('RepoAtlas demo showcase marker v1')) throw new Error('Missing showcase marker');
const dbPath = path.join(process.env.APPDATA, 'io.repoatlas.marketing', 'repoatlas.sqlite');
if (!fs.existsSync(dbPath)) throw new Error('Seed the isolated marketing database first; see docs/demo-showcase.md');
const db = new DatabaseSync(dbPath);
const projects = db.prepare('SELECT * FROM projects ORDER BY id').all();
if (projects.length !== 8 || projects.some(p => !p.canonical_path.startsWith(root + '\\'))) throw new Error('Expected only the eight fictional Projects');
const descriptions = [
  ['Product analytics, service health, and release progress in one team dashboard.', '团队运营看板，集中呈现产品指标、服务状态与交付进度。'],
  ['A Rust desktop console for local development tools and release checks.', '基于 Rust 的桌面开发控制台，管理本地工具与发布检查。'],
  ['Reproducible data experiments with Python notebooks and a FastAPI service.', '可复现的数据分析实验，结合 Python 笔记与 FastAPI 服务。'],
  ['A Go API with typed endpoints, health checks, and structured request logs.', 'Go 后端服务，提供类型化接口、健康检查与结构化请求日志。'],
  ['A team workspace for schedules, shared checklists, and service requests.', '团队协作门户，整合日程、共享清单与服务请求。'],
  ['Offline field notes and checklists that sync when the connection returns.', '离线优先的现场记录与工作清单，联网后同步处理进度。'],
  ['Deployment guides, release checklists, and operational runbooks.', '工程运维手册，整理部署流程、发布清单与排障步骤。'],
  ['A mobile inbox for messages, daily tasks, and personal work updates.', '跨平台移动工作台，统一消息、每日待办与工作进度。'],
];
const sessions = [
  [0, 'codex', 'Add release filters and saved views', '新增发布筛选与保存视图', 'Add a release filter to the overview and keep the selected view when returning to the dashboard.', '在概览中新增发布筛选，返回看板时保留当前选中的视图。'],
  [1, 'claude', 'Make desktop startup feel immediate', '优化桌面启动与缓存加载', 'Show cached Projects first, then refresh the local status without blocking navigation.', '优先显示缓存项目，在后台刷新本地状态，保持导航流畅。'],
  [7, 'codex', 'Finish the offline inbox', '完善离线收件箱体验', 'Keep unread messages available offline and retry pending changes after reconnecting.', '离线时保留未读消息，恢复连接后重试待同步的更新。'],
  [3, 'claude', 'Add request tracing to the API', '为接口补充请求追踪', 'Carry one request ID through validation, service calls, and structured logs.', '让请求标识贯穿参数校验、服务调用与结构化日志。'],
  [5, 'codex', 'Review the field checklist flow', '检查现场清单操作流程', 'Improve keyboard navigation and preserve unfinished checklist items between visits.', '优化键盘导航，再次打开时保留尚未完成的清单项目。'],
  [2, 'claude', 'Document the experiment pipeline', '整理实验流水线与复现说明', 'Document the input validation, reproducible setup, and report export steps.', '整理输入校验、可复现环境与分析报告导出步骤。'],
];
const now = Date.now();
const previewScript = path.join(root, 'marketing-history', 'preview.mjs');
fs.mkdirSync(path.dirname(previewScript), {recursive: true});
fs.writeFileSync(previewScript, `import http from 'node:http';
const [port, name] = process.argv.slice(2);
const server = http.createServer((request, response) => {
  console.log('  GET ' + request.url + '  200');
  response.writeHead(200, {'Content-Type': 'text/html; charset=utf-8'});
  response.end('<!doctype html><title>' + name + '</title><h1>' + name + '</h1><p>Local showcase preview is ready.</p>');
});
server.listen(Number(port), '127.0.0.1', () => {
  console.log('\\n  ' + name + ' / local preview\\n');
  console.log('  Ready on http://localhost:' + port + '/');
  console.log('  Listening on 127.0.0.1:' + port);
  console.log('  Press Ctrl+C to stop.\\n');
});
`);
db.exec('BEGIN');
try {
  for (let i = 0; i < projects.length; i++) {
    const p = projects[i];
    const tasks = JSON.parse(p.tasks_json);
    for (const t of tasks) {
      const names = {dev: ['Start local preview', '启动本地预览'], test: ['Run tests', '运行测试'], build: ['Production build', '生产构建'], check: ['Code checks', '代码检查']};
      if (names[t.id]) t.name = names[t.id][zh ? 1 : 0];
      if (t.id === 'dev' && [0, 7].includes(i)) {
        const port = i === 0 ? 4317 : 4318;
        Object.assign(t, {executable: 'node', argv: [previewScript, String(port), p.display_name], expectedPorts: [port], devUrlPath: '/', devUrlScheme: 'http', shellMode: false});
      }
    }
    db.prepare('UPDATE projects SET description=?, notes=?, tasks_json=? WHERE id=?').run(descriptions[i][zh ? 1 : 0], zh ? '本周迭代：补齐边界测试，优化交互，准备发布说明。' : 'This week: cover edge cases, refine interactions, and prepare the release notes.', JSON.stringify(tasks), p.id);
  }
  db.prepare("UPDATE settings SET value=? WHERE key='locale'").run(locale);
  db.exec('DELETE FROM pending_approvals; DELETE FROM environment_observations;');
  db.prepare("DELETE FROM task_runs WHERE executable='node' AND argv_json LIKE '%marketing-history%preview.mjs%'").run();
  db.prepare("UPDATE task_runs SET status='succeeded',exit_code=0 WHERE status='failed'").run();
  db.prepare('UPDATE project_collections SET name=?,description=? WHERE id=?').run(zh ? '本周交付' : 'This week', zh ? '当前迭代的测试、构建与发布准备。' : 'Tests, builds, and release preparation for the current iteration.', 'demo-collection-release');
  db.prepare('UPDATE project_collections SET name=?,description=? WHERE id=?').run(zh ? '移动产品' : 'Mobile products', zh ? '移动体验与离线工作流程。' : 'Mobile experiences and offline workflows.', 'demo-collection-field');
  db.exec('COMMIT');
} catch (e) { db.exec('ROLLBACK'); throw e; } finally { db.close(); }

for (const [i, spec] of sessions.entries()) {
  const [project, agent, enTitle, zhTitle, enRequest, zhRequest] = spec;
  const title = zh ? zhTitle : enTitle;
  const request = zh ? zhRequest : enRequest;
  const id = `a711a500-1000-4000-8000-${String(i + 1).padStart(12, '0')}`;
  const cwd = projects[project].canonical_path;
  const timestamp = new Date(now - (i + 1) * 36 * 60000).toISOString();
  const reply = zh
    ? `## ${title}\n\n已完成核心交互，并保留现有项目结构。\n\n- 支持键盘操作与清晰的焦点状态\n- 切换页面后保留用户选择\n- 加载失败时保留已有内容并提供重试\n\n### 验证\n\n| 检查 | 结果 |\n| --- | --- |\n| 类型检查 | 通过 |\n| 功能测试 | 通过 |\n| 生产构建 | 通过 |\n\n下一步：检查浅色与深色主题，整理本次发布说明。`
    : `## ${title}\n\nThe main interaction is ready, with the existing Project structure preserved.\n\n- Keyboard navigation and visible focus states\n- Selected views retained when navigating back\n- Cached content and retry actions when a refresh fails\n\n### Verification\n\n| Check | Result |\n| --- | --- |\n| Type checking | Passed |\n| Focused tests | Passed |\n| Production build | Passed |\n\nNext: review both themes and prepare the release notes.`;
  const folder = path.join(root, 'marketing-history', agent, agent === 'codex' ? 'sessions' : 'projects');
  fs.mkdirSync(folder, {recursive: true});
  const records = agent === 'codex' ? [
    {timestamp,type:'session_meta',payload:{id,cwd,timestamp}},
    {timestamp,type:'turn_context',payload:{model:'gpt-5.4'}},
    {timestamp,type:'event_msg',payload:{type:'token_count',info:{total_token_usage:{input_tokens:18200+i*3100,cached_input_tokens:14200+i*2000,output_tokens:4200+i*650,reasoning_output_tokens:900+i*150,total_tokens:22400+i*3750}}}},
    {timestamp,type:'event_msg',payload:{type:'user_message',message:title}},
    {timestamp,type:'event_msg',payload:{type:'user_message',message:request}},
    {timestamp,type:'event_msg',payload:{type:'agent_message',message:reply}},
  ] : [
    {timestamp,sessionId:id,cwd,type:'user',message:{role:'user',content:title}},
    {timestamp,sessionId:id,cwd,type:'user',message:{role:'user',content:request}},
    {timestamp,sessionId:id,cwd,type:'assistant',message:{id:`demo-${i}`,model:'claude-sonnet-4-6',role:'assistant',content:reply,usage:{input_tokens:12300+i*1700,cache_read_input_tokens:8400+i*1100,cache_creation_input_tokens:1600,output_tokens:3200+i*400}}},
  ];
  fs.writeFileSync(path.join(folder, `rollout-${id}.jsonl`), records.map(r => JSON.stringify(r)).join('\n') + '\n');
}
console.log(`Prepared ${locale}: 8 fictional Projects, 6 synthetic sessions; isolated database ${dbPath}`);
