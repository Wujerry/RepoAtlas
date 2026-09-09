import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
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
  ProjectSummary,
  ReadmeDocument,
  ExternalTools,
  EnvironmentInspection,
  AtlasReport,
  PendingApproval,
  ProjectIcon,
  ProjectRemoval,
  ScanProgress,
  ScanResult,
  ScanRoot,
  ScanRootRemoval,
  SearchHit,
  TaskPersistenceFailure,
  TaskRun,
  TaskRuntimeSnapshot,
  PortConflict,
  ProjectCollection,
  CollectionUpsert,
  ProjectBrief,
  AttentionItem,
  AttentionCenterState,
  DashboardSnapshot,
  ProjectDirectoryListing,
  ProjectFilePreview,
  ProjectPathIndexStatus,
  ProjectPathSearchResponse,
} from "../types";
export const api = {
  bootstrap: () => invoke<Bootstrap>("bootstrap"),
  getDataVersion: () => invoke<number>("get_data_version"),
  getSettings: () => invoke<AppSettings>("get_settings"),
  updateSettings: (settings: AppSettings) => invoke<AppSettings>("update_settings", { settings }),
  listScanRoots: () => invoke<ScanRoot[]>("list_scan_roots"),
  addScanRoot: (path: string) => invoke<ScanRoot>("add_scan_root", { path }),
  removeScanRoot: (id: string, alsoRemoveRecords = false) =>
    invoke<ScanRootRemoval>("remove_scan_root", { id, alsoRemoveRecords }),
  listProjects: (query: ProjectQuery = {}) => invoke<ProjectSummary[]>("list_projects", { query }),
  listCollections: () => invoke<ProjectCollection[]>("list_collections"),
  createCollection: (upsert: CollectionUpsert) => invoke<ProjectCollection>("create_collection", { upsert }),
  updateCollection: (id: string, upsert: CollectionUpsert) => invoke<ProjectCollection>("update_collection", { id, upsert }),
  deleteCollection: (id: string) => invoke<void>("delete_collection", { id }),
  collectionMemberIds: (id: string) => invoke<string[]>("collection_member_ids", { id }),
  setCollectionMembers: (id: string, projectIds: string[]) => invoke<ProjectCollection>("set_collection_members", { id, projectIds }),
  saveCollection: (id: string | undefined, upsert: CollectionUpsert, projectIds: string[]) => invoke<ProjectCollection>("save_collection", { id: id ?? null, upsert, projectIds }),
  searchProjects: (query: string) => invoke<SearchHit[]>("search_projects", { query }),
  getProject: (id: string) => invoke<ProjectDetail>("get_project", { id }),
  getProjectBrief: (projectId: string) => invoke<ProjectBrief>("get_project_brief", { projectId }),
  readProjectReadme: (projectId: string) => invoke<ReadmeDocument>("read_project_readme", { projectId }),
  readProjectDocument: (projectId: string, path: string) => invoke<ReadmeDocument>("read_project_document", { projectId, path }),
  inspectProjectEnvironment: (projectId: string) => invoke<EnvironmentInspection>("inspect_project_environment", { projectId }),
  readProjectIcons: (projectIds: string[]) => invoke<ProjectIcon[]>("read_project_icons", { projectIds }),
  setProjectIcon: (projectId: string, path: string) => invoke<ProjectIcon>("set_project_icon", { projectId, path }),
  clearProjectIcon: (projectId: string) => invoke<ProjectIcon>("clear_project_icon", { projectId }),
  readProjectFile: (projectId: string, path: string) => invoke<ReadmeDocument>("read_project_file", { projectId, path }),
  listProjectDirectory: (projectId: string, path = "", showGenerated = false) =>
    invoke<ProjectDirectoryListing>("list_project_directory", { projectId, path, showGenerated }),
  readProjectFilePreview: (projectId: string, path: string) =>
    invoke<ProjectFilePreview>("read_project_file_preview", { projectId, path }),
  readProjectImage: (projectId: string, path: string) =>
    invoke<ArrayBuffer>("read_project_image", { projectId, path }),
  ensureProjectPathIndex: (projectId: string, includeGenerated = false) =>
    invoke<ProjectPathIndexStatus>("ensure_project_path_index", { projectId, includeGenerated }),
  searchProjectPaths: (projectId: string, query: string) =>
    invoke<ProjectPathSearchResponse>("search_project_paths", { projectId, query }),
  cancelProjectPathSearch: (projectId: string) => invoke<void>("cancel_project_path_search", { projectId }),
  cancelProjectPathIndex: (projectId: string) => invoke<void>("cancel_project_path_index", { projectId }),
  revealProjectPath: (projectId: string, path: string) => invoke<void>("reveal_project_path", { projectId, path }),
  revealProjectFile: (projectId: string, path: string) => invoke<void>("reveal_project_file", { projectId, path }),
  openProjectFile: (projectId: string, path: string) => invoke<void>("open_project_file", { projectId, path }),
  mcpSetupInfo: () => invoke<McpSetupInfo>("mcp_setup_info"),
  promoteModule: (projectId: string, moduleId: string) => invoke<ProjectSummary>("promote_module", { projectId, moduleId }),
  setDirectoryGroup: (projectId: string, enabled: boolean) => invoke<ProjectSummary>("set_directory_group", { projectId, enabled }),
  resolveModulePath: (projectId: string, moduleId: string) => invoke<string>("resolve_module_path", { projectId, moduleId }),
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
  listPendingApprovals: () => invoke<PendingApproval[]>("list_pending_approvals"),
  listAttentionItems: () => invoke<AttentionItem[]>("list_attention_items"),
  getAttentionCenter: () => invoke<AttentionCenterState>("get_attention_center"),
  getDashboardSnapshot: () => invoke<DashboardSnapshot>("get_dashboard_snapshot"),
  acknowledgeAttentionItem: (itemId: string, sourceVersion: string) => invoke<void>("acknowledge_attention_item", { itemId, sourceVersion }),
  resolvePendingApproval: (approvalId: string, approved: boolean, allowPortConflicts = false) => invoke<PendingApproval>("resolve_pending_approval", { approvalId, approved, allowPortConflicts }),
  startScan: (rootId?: string, folderPath?: string) => invoke<void>("start_scan", { rootId: rootId ?? null, folderPath: folderPath ?? null }),
  cancelScan: () => invoke<void>("cancel_scan"),
  gitStatus: (projectId: string) => invoke<GitStatus>("git_status", { projectId }),
  gitDiff: (projectId: string, path?: string, staged = false) =>
    invoke<GitDiff>("git_diff", { projectId, path: path ?? null, staged }),
  gitExecute: (projectId: string, op: GitOp) =>
    invoke<GitCommandResult>("git_execute", { projectId, op }),
  listTaskRuns: (projectId: string) => invoke<TaskRun[]>("list_task_runs", { projectId }),
  listActiveTaskRuns: () => invoke<TaskRun[]>("list_active_task_runs"),
  getTaskRun: (runId: string) => invoke<TaskRun>("get_task_run", { runId }),
  readTaskLog: (runId: string) => invoke<string>("read_task_log", { runId }),
  getTaskRuntimeSnapshot: (runId: string) => invoke<TaskRuntimeSnapshot>("get_task_runtime_snapshot", { runId }),
  getTaskRuntimeSnapshots: (runIds: string[]) => invoke<TaskRuntimeSnapshot[]>("get_task_runtime_snapshots", { runIds }),
  preflightTaskPorts: (projectId: string, taskId: string) => invoke<PortConflict[]>("preflight_task_ports", { projectId, taskId }),
  startTask: (projectId: string, taskId: string, allowPortConflicts = false) => invoke<TaskRun>("start_task", { projectId, taskId, allowPortConflicts }),
  openDevEndpoint: (runId: string, endpoint: string) => invoke<void>("open_dev_endpoint", { runId, endpoint }),
  writeTaskStdin: (runId: string, text: string) => invoke<void>("write_task_stdin", { runId, text }),
  stopTask: (runId: string) => invoke<TaskRun>("stop_task", { runId }),
  resizeTaskRun: (runId: string, cols: number, rows: number) => invoke<void>("resize_task_run", { runId, cols, rows }),
  showMainWindow: () => invoke<void>("show_main_window"),
  openInExplorer: (path: string) => invoke<void>("open_in_explorer", { path }),
  listExternalTools: () => invoke<ExternalTools>("list_external_tools"),
  openInTerminal: (path: string, terminal?: string) => invoke<void>("open_in_terminal", { path, terminal }),
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

export async function onTaskRuntime(handler: (snapshot: TaskRuntimeSnapshot) => void): Promise<UnlistenFn> {
  return listen<TaskRuntimeSnapshot>("task://runtime", (event) => handler(event.payload));
}

export async function onAttentionChanged(handler: () => void): Promise<UnlistenFn> {
  return listen("attention://changed", handler);
}

export async function onTaskPersistenceFailed(handler: (failure: TaskPersistenceFailure) => void): Promise<UnlistenFn> {
  return listen<TaskPersistenceFailure>("task://persistence-failed", (event) => handler(event.payload));
}

export async function onFileIndexProgress(handler: (status: ProjectPathIndexStatus) => void): Promise<UnlistenFn> {
  return listen<ProjectPathIndexStatus>("files://index-progress", (event) => handler(event.payload));
}
