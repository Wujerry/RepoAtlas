import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AnalysisPlan,
  AiMemoryItem,
  AiSummary,
  AppSettings,
  Bootstrap,
  GitCommandResult,
  GitDiff,
  GitOp,
  GitStatus,
  LogChunk,
  McpSetupInfo,
  ProjectDetail,
  ProjectPatch,
  ProjectQuery,
  ChatMessage,
  ConversationSummary,
  ProjectSummary,
  ReadmeDocument,
  ExternalTools,
  EnvironmentInspection,
  AtlasReport,
  PendingApproval,
  ProjectIcon,
  ProjectRemoval,
  ProviderPreset,
  ProviderProfile,
  ProviderUpsert,
  ScanProgress,
  ScanResult,
  ScanRoot,
  ScanRootRemoval,
  SearchHit,
  TaskPersistenceFailure,
  TaskRun,
} from "../types";
export const api = {
  bootstrap: () => invoke<Bootstrap>("bootstrap"),
  getSettings: () => invoke<AppSettings>("get_settings"),
  updateSettings: (settings: AppSettings) => invoke<AppSettings>("update_settings", { settings }),
  listScanRoots: () => invoke<ScanRoot[]>("list_scan_roots"),
  addScanRoot: (path: string) => invoke<ScanRoot>("add_scan_root", { path }),
  removeScanRoot: (id: string, alsoRemoveRecords = false) =>
    invoke<ScanRootRemoval>("remove_scan_root", { id, alsoRemoveRecords }),
  listProjects: (query: ProjectQuery = {}) => invoke<ProjectSummary[]>("list_projects", { query }),
  searchProjects: (query: string) => invoke<SearchHit[]>("search_projects", { query }),
  getProject: (id: string) => invoke<ProjectDetail>("get_project", { id }),
  readProjectReadme: (projectId: string) => invoke<ReadmeDocument>("read_project_readme", { projectId }),
  readProjectDocument: (projectId: string, path: string) => invoke<ReadmeDocument>("read_project_document", { projectId, path }),
  inspectProjectEnvironment: (projectId: string) => invoke<EnvironmentInspection>("inspect_project_environment", { projectId }),
  readProjectIcons: (projectIds: string[]) => invoke<ProjectIcon[]>("read_project_icons", { projectIds }),
  setProjectIcon: (projectId: string, path: string) => invoke<ProjectIcon>("set_project_icon", { projectId, path }),
  clearProjectIcon: (projectId: string) => invoke<ProjectIcon>("clear_project_icon", { projectId }),
  readProjectFile: (projectId: string, path: string) => invoke<ReadmeDocument>("read_project_file", { projectId, path }),
  revealProjectFile: (projectId: string, path: string) => invoke<void>("reveal_project_file", { projectId, path }),
  openProjectFile: (projectId: string, path: string) => invoke<void>("open_project_file", { projectId, path }),
  mcpSetupInfo: () => invoke<McpSetupInfo>("mcp_setup_info"),
  registerProject: (path: string) => invoke<ProjectSummary>("register_project", { path }),
  updateProject: (id: string, patch: ProjectPatch) =>
    invoke<ProjectSummary>("update_project", { id, patch }),
  removeProject: (id: string) => invoke<ProjectRemoval>("remove_project", { id }),
  removeFolderRecords: (projectIds: string[], scanRootIds: string[]) =>
    invoke<ProjectRemoval[]>("remove_folder_records", { projectIds, scanRootIds }),
  refreshProject: (id: string) => invoke<ProjectSummary>("refresh_project", { id }),
  markOpened: (id: string) => invoke<ProjectSummary>("mark_project_opened", { id }),
  relocateProject: (id: string, path: string) => invoke<ProjectSummary>("relocate_project", { id, path }),
  atlasReport: (projectId: string) => invoke<AtlasReport>("atlas_report", { projectId }),
  exportAtlasReportTo: (projectId: string, path: string) => invoke<void>("export_atlas_report_to", { projectId, path }),
  listSummaries: (projectId: string) => invoke<AiSummary[]>("list_summaries", { projectId }),
  getSummary: (summaryId: string) => invoke<AiSummary>("get_summary", { summaryId }),
  acceptSummaryMemory: (summaryId: string) => invoke<AiMemoryItem>("accept_summary_memory", { summaryId }),
  listPendingApprovals: () => invoke<PendingApproval[]>("list_pending_approvals"),
  resolvePendingApproval: (approvalId: string, approved: boolean) => invoke<PendingApproval>("resolve_pending_approval", { approvalId, approved }),
  startScan: (rootId?: string) => invoke<void>("start_scan", { rootId: rootId ?? null }),
  cancelScan: () => invoke<void>("cancel_scan"),
  gitStatus: (projectId: string) => invoke<GitStatus>("git_status", { projectId }),
  gitDiff: (projectId: string, path?: string, staged = false) =>
    invoke<GitDiff>("git_diff", { projectId, path: path ?? null, staged }),
  gitExecute: (projectId: string, op: GitOp) =>
    invoke<GitCommandResult>("git_execute", { projectId, op }),
  listTaskRuns: (projectId: string) => invoke<TaskRun[]>("list_task_runs", { projectId }),
  listActiveTaskRuns: () => invoke<TaskRun[]>("list_active_task_runs"),
  readTaskLog: (runId: string) => invoke<string>("read_task_log", { runId }),
  startTask: (projectId: string, taskId: string) => invoke<TaskRun>("start_task", { projectId, taskId }),
  writeTaskStdin: (runId: string, text: string) => invoke<void>("write_task_stdin", { runId, text }),
  stopTask: (runId: string) => invoke<TaskRun>("stop_task", { runId }),
  resizeTaskRun: (runId: string, cols: number, rows: number) => invoke<void>("resize_task_run", { runId, cols, rows }),
  showMainWindow: () => invoke<void>("show_main_window"),
  openInExplorer: (path: string) => invoke<void>("open_in_explorer", { path }),
  listExternalTools: () => invoke<ExternalTools>("list_external_tools"),
  openInTerminal: (path: string, terminal?: string) => invoke<void>("open_in_terminal", { path, terminal }),
  listProviderProfiles: () => invoke<ProviderProfile[]>("list_provider_profiles"),
  upsertProviderProfile: (upsert: ProviderUpsert) => invoke<ProviderProfile>("upsert_provider_profile", { upsert }),
  deleteProviderProfile: (id: string) => invoke<void>("delete_provider_profile", { id }),
  providerPresets: () => invoke<ProviderPreset[]>("provider_presets"),
  listMemory: (projectId: string) => invoke<AiMemoryItem[]>("list_memory", { projectId }),
  addMemory: (projectId: string, text: string) => invoke<AiMemoryItem>("add_memory", { projectId, text }),
  deleteMemory: (id: string) => invoke<void>("delete_memory", { id }),
  latestSummary: (projectId: string) => invoke<AiSummary | null>("latest_summary", { projectId }),
  conversationSummary: (projectId: string) => invoke<ConversationSummary>("conversation_summary", { projectId }),
  listConversation: (projectId: string) => invoke<ChatMessage[]>("list_conversation", { projectId }),
  clearConversation: (projectId: string) => invoke<void>("clear_conversation", { projectId }),
  analysisPlan: (projectId: string, providerId: string) => invoke<AnalysisPlan>("analysis_plan", { projectId, providerId }),
  summarizeProject: (projectId: string, providerId: string, apiKey: string) =>
    invoke<AiSummary>("summarize_project", { projectId, providerId, apiKey }),
  askProject: (projectId: string, providerId: string, apiKey: string, question: string) =>
    invoke<string>("ask_project", { projectId, providerId, apiKey, question }),
  exportJson: () => invoke<string>("export_json"),
  exportJsonTo: (path: string) => invoke<void>("export_json_to", { path }),
  importJson: (data: string) => invoke<number>("import_json", { data }),
  importJsonFrom: (path: string) => invoke<number>("import_json_from", { path }),
  backupDb: (dest: string) => invoke<void>("backup_db", { dest }),
  openInIde: (path: string, ide: string) => invoke<void>("open_in_ide", { path, ide }),
  openInAgent: (path: string, agent: string) => invoke<void>("open_in_agent", { path, agent }),
};

export async function onScanProgress(handler: (progress: ScanProgress) => void): Promise<UnlistenFn> {
  return listen<ScanProgress>("scan://progress", (event) => handler(event.payload));
}

export async function onScanCompleted(handler: (results: ScanResult[]) => void): Promise<UnlistenFn> {
  return listen<ScanResult[]>("scan://completed", (event) => handler(event.payload));
}

export async function onScanFailed(handler: (message: string) => void): Promise<UnlistenFn> {
  return listen<string>("scan://failed", (event) => handler(event.payload));
}

export async function onTaskLog(handler: (chunk: LogChunk) => void): Promise<UnlistenFn> {
  return listen<LogChunk>("task://log", (event) => handler(event.payload));
}

export async function onTaskExited(handler: (run: TaskRun) => void): Promise<UnlistenFn> {
  return listen<TaskRun>("task://exited", (event) => handler(event.payload));
}

export async function onTaskPersistenceFailed(handler: (failure: TaskPersistenceFailure) => void): Promise<UnlistenFn> {
  return listen<TaskPersistenceFailure>("task://persistence-failed", (event) => handler(event.payload));
}
