// Capture the real, isolated Tauri WebView. No renderer mocks or screen cursor overlay.
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { execFileSync } from 'node:child_process';
import { DatabaseSync } from 'node:sqlite';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE_PATH || 'playwright');
const locale = process.argv[2] ?? 'en';
if (!['en', 'zh-CN'].includes(locale)) throw new Error('Use en or zh-CN');
const prefix = locale === 'en' ? 'en' : 'zh';
const zh = prefix === 'zh';
const browser = await chromium.connectOverCDP('http://127.0.0.1:9337');
const page = browser.contexts()[0].pages().find(p => p.url().includes('localhost:1420'));
if (!page) throw new Error('Expected the local Tauri development WebView');
const invoke = (cmd, args = {}) => page.evaluate(({cmd,args}) => window.__TAURI_INTERNALS__.invoke(cmd,args), {cmd,args});
const runIds = [];
const captures = [];
let fixtureDb;
let cacheTimer;
const demoProviders = ['codex','claude','copilot','opencode-go'];
function seedQuotas() {
  const now=Date.now(), stamp=new Date(now).toISOString();
  const window=(id,label,usedPercent,minutes,resetMinutes)=>({id,label,usedPercent,used:null,limit:null,unlimited:false,resetsAt:new Date(now+resetMinutes*60000).toISOString(),windowMinutes:minutes});
  const accounts=[
    {provider:'codex',plan:'Plus',windows:[window('rolling','rolling',28,300,142),window('weekly','weekly',16,10080,4320)]},
    {provider:'claude',plan:'Pro',windows:[window('rolling','rolling',46,300,95),window('weekly','weekly',32,10080,2880)]},
    {provider:'copilot',plan:'Pro',windows:[window('premium','premium_interactions',14,43200,14400)]},
    {provider:'opencode-go',plan:'Go',windows:[window('rolling','rolling',37,300,210)]},
  ];
  const insert=fixtureDb.prepare('INSERT INTO subscription_usage(provider,enabled,generation,snapshot) VALUES(?,1,?,?) ON CONFLICT(provider) DO UPDATE SET enabled=1,generation=excluded.generation,snapshot=excluded.snapshot');
  for(const account of accounts)insert.run(account.provider,'marketing-fixture',JSON.stringify({...account,enabled:true,status:'ready',fetchedAt:stamp,attemptedAt:stamp}));
}
try {
  if (await invoke('plugin:app|identifier') !== 'io.repoatlas.marketing') throw new Error('Refusing to alter a non-marketing instance');
  if ((await invoke('list_active_task_runs')).length) throw new Error('Stop previous showcase tasks before capture');
  execFileSync(process.execPath, ['scripts/prepare-marketing.mjs', locale], {stdio:'inherit', windowsHide:true});
  fixtureDb=new DatabaseSync(path.join(process.env.APPDATA,'io.repoatlas.marketing/repoatlas.sqlite'));
  // Fresh Core cache prevents credential reads/provider requests. Keep fresh only
  // while capturing; remove the fixture connections in finally, even on failure.
  seedQuotas(); cacheTimer=setInterval(seedQuotas,60_000);
  for (const adapter of ['codex', 'claude']) await invoke('set_session_source', {adapter, path:`C:\\RepoAtlas Showcase\\marketing-history\\${adapter}\\${adapter === 'codex' ? 'sessions' : 'projects'}`, enabled:true});
  await invoke('refresh_agent_sessions', {force:true});
  for (let i=0; i<60; i++) {
    const status=await invoke('session_refresh_status');
    if (!status.running) { if(status.errors) throw new Error('Session indexing failed'); break; }
    if(i===59) throw new Error('Session indexing timed out');
    await page.waitForTimeout(500);
  }
  const sessions=await invoke('search_agent_sessions',{query:{}});
  if(sessions.total!==6 || sessions.items.some(hit=>!hit.session.usage?.totalTokens)) throw new Error('Expected six sessions with recorded tokens');
  await page.setViewportSize({width:1600,height:1000});
  await page.emulateMedia({reducedMotion:'reduce'});
  const reset = async (theme, project=false) => {
    seedQuotas();
    await invoke('update_settings',{settings:{...await invoke('get_settings'),theme,locale}});
    await page.evaluate(({project})=>{
      localStorage.setItem('repoatlas.navigation.v1',JSON.stringify({page:'library',librarySurface:project?'project':'dashboard',selectedId:project?'11111111-1111-4111-8111-111111111111':undefined,scope:'projects',query:'',filters:{language:'',tag:''},sort:'default'}));
      localStorage.setItem('repoatlas.projectTab.v1',JSON.stringify('overview'));
    },{project});
    await page.reload();
    await page.locator(project?'.overview-layout':'.ah-session-card').first().waitFor();
    await page.waitForFunction(()=>!document.querySelector('.dashboard-loading,.skeleton-list'));
    await page.waitForTimeout(1200);
  };
  const capture = async name => {
    await page.mouse.move(1599,999);
    await page.evaluate(()=>{if(document.activeElement instanceof HTMLElement)document.activeElement.blur();});
    await page.waitForTimeout(600);
    await page.evaluate(()=>document.fonts.ready);
    const text=await page.locator('body').innerText();
    if(/C:\\Users\\|wujer|F:\\code\\|session_source_missing/i.test(text)) throw new Error('Private path or unresolved source in '+name);
    const dest=path.join('assets/screenshots',name);
    await page.screenshot({path:dest,animations:'disabled',caret:'hide',scale:'css',...(name.endsWith('.jpg')?{quality:94}: {})});
    fs.copyFileSync(dest,path.join('website/assets/screenshots',name));
    captures.push(name); console.log('Captured '+name);
  };
  for(const theme of ['dark','light']) {
    await reset(theme); await capture(`${prefix}-${theme}-workspace.jpg`);
    await page.locator('.project-row[aria-label="Atlas Dashboard"]').click();
    await page.locator('.overview-layout').waitFor();
    await page.waitForTimeout(1800);
    await capture(`${prefix}-${theme}-library.png`);
  }
  const theme=zh?'light':'dark';
  await reset(theme);
  await page.locator('.titlebar-quota').click();
  await page.locator('.quota-popover').waitFor();
  await capture(`${prefix}-${theme}-usage.jpg`);
  await page.keyboard.press('Escape');
  await page.getByRole('button',{name:zh?'会话':'Sessions',exact:true}).click();
  await page.locator('.ah-result').first().waitFor();
  await page.locator('.ah-result').first().click();
  await page.locator('.ah-message.assistant').waitFor();
  await page.locator('.ah-transcript').evaluate(e=>e.scrollTop=e.scrollHeight);
  await capture(`${prefix}-${theme}-sessions.jpg`);
  await page.locator('.ah-detail').getByRole('button',{name:zh?'用量明细':'Token details',exact:true}).click();
  await page.locator('.token-breakdown-body').waitFor();
  await capture(`${prefix}-${theme}-tokens.jpg`);
  await reset(theme);
  await page.locator('.titlebar-command').click();
  await page.locator('.command-search-input').fill(zh?'发布':'release');
  await page.getByRole('option').first().waitFor();
  await page.waitForTimeout(1200);
  await capture(`${prefix}-${theme}-search.jpg`);
  await reset('light');
  for (const projectId of ['11111111-1111-4111-8111-111111111111','88888888-8888-4888-8888-888888888888']) {
    const run=await invoke('start_task',{projectId,taskId:'dev',allowPortConflicts:false}); runIds.push(run.id);
  }
  await page.locator('.titlebar-tasks').click();
  await page.locator('.task-layout-toggle button').nth(1).click();
  await page.waitForFunction(()=>document.querySelectorAll('.task-terminal-pane').length===2);
  for(const port of [4317,4318]) {
    let ready=false;
    for(let i=0;i<30;i++){try{const response=await fetch(`http://127.0.0.1:${port}/`);if(response.ok){ready=true;break;}}catch{} await page.waitForTimeout(300);}
    if(!ready)throw new Error('Preview did not start on '+port);
  }
  await page.waitForTimeout(2200);
  await capture(`${prefix}-light-tasks.png`);
  fs.mkdirSync('output/marketing-2026-10-08',{recursive:true});
  fs.writeFileSync(`output/marketing-2026-10-08/${prefix}-captures.json`,JSON.stringify({viewport:{width:1600,height:1000},locale,captures},null,2));
} finally {
  clearInterval(cacheTimer);
  if(fixtureDb){fixtureDb.prepare("DELETE FROM subscription_usage WHERE generation='marketing-fixture'").run();fixtureDb.close();}
  for(const runId of runIds) await invoke('stop_task',{runId}).catch(console.error);
  await browser.close();
}
