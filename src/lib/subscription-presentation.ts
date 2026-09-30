import type { MessageKey } from "../i18n";
import type { SubscriptionUsage, UsageWindow } from "./usage";
export type UsageT = (key: MessageKey) => string;
export const providers: Record<string, {name:string; icon:string; connection:string; kind?:"balance"}> = {
  factory:{name:"Factory Droid",icon:"factory",connection:"FACTORY_API_KEY · api.factory.ai"},
  zed:{name:"Zed",icon:"zed",connection:"ZED_COOKIE (zed.session) · cloud.zed.dev"},
  stepfun:{name:"StepFun",icon:"stepfun",connection:"STEPFUN_TOKEN (Oasis-Token) · platform.stepfun.com"},
  grok:{name:"Grok",icon:"grok",connection:"GROK_HOME/auth.json / GROK_BEARER_TOKEN · grok.com"},
  antigravity:{name:"Google Antigravity",icon:"antigravity",connection:"Antigravity · 127.0.0.1 (RetrieveUserQuotaSummary / GetUserStatus)"},
  codex:{name:"Codex",icon:"codex",connection:"CODEX_HOME/auth.json · chatgpt.com"},
  claude:{name:"Claude",icon:"claude",connection:"CLAUDE_CONFIG_DIR/.credentials.json / CLAUDE_CODE_OAUTH_TOKEN · api.anthropic.com"},
  copilot:{name:"GitHub Copilot",icon:"copilot",connection:"COPILOT_API_TOKEN / GITHUB_COPILOT_TOKEN · api.github.com"},
  "opencode-go":{name:"OpenCode Go",icon:"opencode",connection:"OpenCode auth.json · opencode.ai"},
  kimi:{name:"Kimi Code",icon:"kimi",connection:"KIMI_CODE_API_KEY / OpenCode kimi-for-coding · api.kimi.com"},
  cursor:{name:"Cursor",icon:"cursor",connection:"Cursor state.vscdb / CURSOR_SESSION_TOKEN · cursor.com"},
  zai:{name:"Z.ai",icon:"zai",connection:"ZAI_API_KEY / OpenCode zai-coding-plan · api.z.ai"},
  zhipu:{name:"GLM Coding Plan",icon:"zai",connection:"ZHIPU_API_KEY / GLM_API_KEY / OpenCode zhipuai-coding-plan · open.bigmodel.cn"},
  minimax:{name:"MiniMax Global",icon:"minimax",connection:"MINIMAX_CODING_API_KEY / OpenCode minimax-coding-plan · api.minimax.io"},
  "minimax-cn":{name:"MiniMax CN",icon:"minimax",connection:"MINIMAX_CN_CODING_API_KEY / OpenCode minimax-cn-coding-plan · api.minimaxi.com"},
  openrouter:{name:"OpenRouter",icon:"openrouter",kind:"balance",connection:"OPENROUTER_API_KEY / OpenCode openrouter · openrouter.ai"},
  deepseek:{name:"DeepSeek",icon:"deepseek",kind:"balance",connection:"DEEPSEEK_API_KEY / OpenCode deepseek · api.deepseek.com"},
};
export const statusKeys: Record<string,MessageKey> = {client_not_running:"usageClientNotRunning",disabled:"usageNotConnected",not_connected:"usageNoLogin",login_required:"usageLoginRequired",rate_limited:"usageRateLimited",network_error:"usageNetworkError",unsupported:"usageUnsupported",unavailable:"usageUnavailable"};
export function resetLabel(value:string|null, now:number, t:UsageT) {
  if (!value || !Number.isFinite(Date.parse(value))) return t("usageResetUnknown");
  const minutes=Math.ceil((Date.parse(value)-now)/60000);
  if(minutes<=0)return t("usageResetPending");
  const duration=minutes>=1440 ? `${Math.floor(minutes/1440)} ${t("usageDay")} ${Math.floor(minutes%1440/60)} ${t("usageHour")}` : minutes>=60 ? `${Math.floor(minutes/60)} ${t("usageHour")} ${minutes%60} ${t("usageMinute")}` : `${minutes} ${t("usageMinute")}`;
  return `${t("usageReset")} · ${duration}`;
}
export function windowLabel(w:UsageWindow,t:UsageT) {
  if (["rolling","weekly","monthly"].includes(w.label) && w.windowMinutes) {
    if(w.windowMinutes===10080)return t("usageWeekly");
    if(w.windowMinutes===1440)return t("usageDaily");
    if(w.windowMinutes<1440)return `${Number((w.windowMinutes/60).toFixed(1))} ${t("usageHour")}`;
    return `${Number((w.windowMinutes/1440).toFixed(1))} ${t("usageDay")}`;
  }
  const labels:Record<string,MessageKey>={rolling:"usageRolling",weekly:"usageWeekly",monthly:"usageMonthly",daily:"usageDaily",premium_interactions:"usagePremium",chat:"usageChat",completions:"usageCompletions",balance:"usageBalance",key_limit:"usageKeyLimit",on_demand:"usageOnDemand",team_pool:"usageTeamPool"};
  return labels[w.label] ? t(labels[w.label]) : w.label.replace(/_/g," ");
}
export function limitingWindow(s:SubscriptionUsage) {
  // Keep the provider's original order on equal usage. A depleted monthly
  // window must not disappear behind a healthy 5-hour window.
  return s.windows.filter(w=>w.usedPercent!=null).reduce<UsageWindow|undefined>((best,w)=>!best || (w.usedPercent??0)>(best.usedPercent??0)?w:best,undefined) ?? s.windows[0];
}
export function staleUsage(s:SubscriptionUsage, now:number) {
  return s.status!=="ready" || !s.fetchedAt || !Number.isFinite(Date.parse(s.fetchedAt)) || now-Date.parse(s.fetchedAt)>6*60_000 || s.windows.some(w=>w.resetsAt && Date.parse(w.resetsAt)<=now);
}
