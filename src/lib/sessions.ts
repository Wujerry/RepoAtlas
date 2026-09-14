import { invoke } from "@tauri-apps/api/core";
export const sessionAgents = { claude: "Claude Code", codex: "Codex CLI", opencode: "OpenCode", "cursor-cli": "Cursor CLI", gemini: "Gemini CLI", copilot: "GitHub Copilot CLI", kimi: "Kimi Code", qwen: "Qwen Code" };
export interface SessionSource { id: string; adapter: string; path: string; enabled: boolean; lastScannedAt: string | null; lastError: string | null }
export interface SessionCapabilities { search: boolean; transcript: boolean; directResume: boolean }
export interface AgentSession {
  id: string; sourceId: string; adapter: string; externalId: string; projectId: string | null; projectName: string | null;
  matchKind: string; cwd: string; title: string; lastUserExcerpt: string; startedAt: string; updatedAt: string;
  messageCount: number; archived: boolean; sourceMissing: boolean; sourceLocator: string; revision: string;
  resumeReason: string | null; capabilities: SessionCapabilities;
}
export interface SessionMessage { index: number; role: string; content: string; timestamp: string }
export interface SessionQuery { query?: string; projectId?: string; adapter?: string; after?: string; before?: string; archived?: boolean; offset?: number; limit?: number; resumableOnly?: boolean }
export interface SessionSearchHit { session: AgentSession; snippets: SessionMessage[] }
export interface SessionSearchResult { items: SessionSearchHit[]; total: number }
export interface SessionMessagePage { items: SessionMessage[]; total: number }
export interface SessionRefreshJob { running: boolean; canceled: boolean; processed: number; errors: number; sourceId: string | null }
export interface SessionResumeSpec { app: { id: string; name: string; canResume: boolean } | null; agent: string; args: string[]; cwd: string; command: string; env: Record<string, string> }
export const sessionApi = {
  sources: () => invoke<SessionSource[]>("session_sources"),
  setSource: (adapter: string, path: string, enabled: boolean) => invoke<SessionSource>("set_session_source", { adapter, path, enabled }),
  search: (query: SessionQuery = {}) => invoke<SessionSearchResult>("search_agent_sessions", { query }),
  get: (id: string) => invoke<AgentSession>("get_agent_session", { id }),
  recent: (projectId?: string) => invoke<AgentSession[]>("continue_agent_sessions", { projectId: projectId ?? null }),
  messages: (id: string, offset = 0, limit = 40) => invoke<SessionMessagePage>("agent_session_messages", { id, offset, limit }),
  link: (id: string, projectId: string | null, cwd: string | null = null) => invoke<void>("link_agent_session", { id, projectId, cwd }),
  resumeSpec: (id: string) => invoke<SessionResumeSpec>("agent_session_resume_spec", { id }),
  resume: (id: string, target: "cli" | "app" = "cli") => invoke<void>("resume_agent_session", { id, target }),
  refresh: (force = false) => invoke<SessionRefreshJob>("refresh_agent_sessions", { force }),
  status: () => invoke<SessionRefreshJob>("session_refresh_status"),
  cancel: () => invoke<void>("cancel_session_refresh"),
  rebuild: () => invoke<void>("rebuild_session_index"),
};
export function openSessionHistory(projectId?: string, session?: AgentSession) { window.dispatchEvent(new CustomEvent("repoatlas:session-history", { detail: { projectId, session } })); }
export function sessionAgentName(adapter: string) { return sessionAgents[adapter as keyof typeof sessionAgents] ?? adapter; }
export function sessionTime(value: string, chinese: boolean) {
  const delta = (Date.now() - new Date(value).getTime()) / 1000;
  if (!Number.isFinite(delta)) return "—";
  const format = new Intl.RelativeTimeFormat(chinese ? "zh" : "en", { numeric: "auto" });
  if (Math.abs(delta) < 60) return format.format(0, "minute");
  if (Math.abs(delta) < 3600) return format.format(-Math.floor(delta / 60), "minute");
  if (Math.abs(delta) < 86400) return format.format(-Math.floor(delta / 3600), "hour");
  return format.format(-Math.floor(delta / 86400), "day");
}
