export type AppView = "library" | "settings" | "help";
export type ProjectScope = "projects" | "favorites" | "recent" | "archived";
export type ProjectTab = "overview" | "files" | "git" | "tasks";

export type ToastTone = "info" | "success" | "warning" | "error";

export interface ToastMessage {
  id: string;
  tone: ToastTone;
  title: string;
  detail?: string;
}

export interface ScanRoot {
  id: string;
  path: string;
  createdAt: string;
  lastScannedAt: string | null;
}

export interface ProjectSummary {
  id: string;
  canonicalPath: string;
  displayName: string;
  detectedName: string | null;
  notes: string | null;
  description: string | null;
  vcsKind: string;
  availability: string;
  archived: boolean;
  favorite: boolean;
  origin: string;
  scanRootId: string | null;
  languages: string[];
  frameworks: string[];
  packageManagers: string[];
  tags: string[];
  sourceMtime: string | null;
  lastCommitAt: string | null;
  lastOpenedAt: string | null;
  updatedAt: string;
}

export interface DetectedFact {
  kind: string;
  value: string;
  confidence: number;
  source: string;
}

export interface TaskDefinition {
  id: string;
  kind: string;
  name: string;
  description?: string | null;
  executable: string;
  argv: string[];
  cwd: string | null;
  inferred: boolean;
  shellMode: boolean;
  expectedPorts?: number[];
  devUrlPath?: string | null;
  devUrlScheme?: "http" | "https" | null;
}

export interface GitSnapshot {
  branch: string | null;
  dirty: boolean | null;
  ahead: number | null;
  behind: number | null;
  lastCommitSha: string | null;
  lastCommitSubject: string | null;
  lastCommitAt: string | null;
  observedAt: string;
}

export interface GitFileStatus {
  path: string;
  status: string;
  staged: boolean;
}

export interface GitLogEntry {
  sha: string;
  subject: string;
  author: string;
  committedAt: string;
}

export interface GitStatus {
  snapshot: GitSnapshot;
  files: GitFileStatus[];
  branches: string[];
  log: GitLogEntry[];
}

export interface GitDiff {
  path: string | null;
  staged: boolean;
  patch: string;
}

export type GitOp =
  | { type: "status" }
  | { type: "diff"; path?: string | null; staged: boolean }
  | { type: "log"; limit?: number | null }
  | { type: "branch" }
  | { type: "checkout"; branch: string; create: boolean }
  | { type: "stage"; paths: string[] }
  | { type: "unstage"; paths: string[] }
  | { type: "commit"; message: string }
  | { type: "pull" }
  | { type: "push" }
  | { type: "stashPush"; message?: string | null }
  | { type: "stashPop" };

export interface GitCommandResult {
  ok: boolean;
  stdout: string;
  stderr: string;
}

export interface TaskPersistenceFailure {
  runId: string;
  error: string;
  run?: TaskRun | null;
}

export interface TaskRun {
  id: string;
  projectId: string;
  taskId: string | null;
  kind: string;
  executable: string;
  argv: string[];
  cwd: string;
  shellMode: boolean;
  status: string;
  exitCode: number | null;
  logPath: string;
  startedAt: string;
  finishedAt: string | null;
  peakCpuPercent?: number | null;
  peakMemoryBytes?: number | null;
  observedPorts?: number[];
}

export interface PortConflict {
  port: number;
  pid: number | null;
  processName: string | null;
}

export interface TaskRuntimeSnapshot {
  runId: string;
  capturedAt: string;
  cpuPercent: number;
  peakCpuPercent: number;
  memoryBytes: number;
  peakMemoryBytes: number;
  processCount: number;
  ports: number[];
  portInspectionAvailable: boolean;
  endpoints: string[];
  conflicts: PortConflict[];
}

export interface TaskSpec {
  projectId: string;
  taskId?: string | null;
  kind: string;
  executable: string;
  argv: string[];
  cwd?: string | null;
  shellMode: boolean;
}

export interface LogChunk {
  runId: string;
  stream: string;
  text: string;
}

export interface DependencySnapshot {
  packageManager: string | null;
  declared: string[];
  lockfile: string | null;
  observedAt: string;
}

export interface RuntimeRequirement {
  ecosystem: string;
  label: string;
  constraint: string | null;
  source: string | null;
}

export interface ProjectFile {
  kind: string;
  path: string;
  source: string;
}

export interface RuntimeStatus {
  ecosystem: string;
  label: string;
  constraint: string | null;
  source: string | null;
  localVersion: string | null;
  matchState:
    | "match"
    | "mismatch"
    | "undeclared"
    | "missing"
    | "unknown"
    | string;
}

export interface EnvironmentInspection {
  projectId: string;
  runtimes: RuntimeStatus[];
  files: ProjectFile[];
}

export interface ProjectIcon {
  projectId: string;
  kind: "override" | "asset" | "language" | string;
  source: string | null;
  mimeType: string | null;
  dataUrl: string | null;
}

export interface ProjectDetail {
  project: ProjectSummary;
  facts: DetectedFact[];
  readmePath: string | null;
  readmeExcerpt: string | null;
  tasks: TaskDefinition[];
  git: GitSnapshot | null;
  dependencies: DependencySnapshot | null;
  runtimeRequirements?: RuntimeRequirement[];
  projectFiles?: ProjectFile[];
  startHere?: StartHereTask[];
  lineage?: CheckoutLineage | null;
  recentEvents?: ProjectEvent[];
}

export interface StartHereTask {
  taskId: string;
  kind: string;
  name: string;
  source: string;
  inferred: boolean;
}

export interface LineageCheckout {
  projectId: string;
  displayName: string;
  canonicalPath: string;
  branch: string | null;
}

export interface CheckoutLineage {
  remoteUrl: string | null;
  normalizedUrl: string | null;
  checkouts: LineageCheckout[];
}

export interface ProjectEvent {
  id: string;
  projectId: string;
  kind: string;
  title: string;
  detail: string | null;
  createdAt: string;
}

export interface AtlasReport {
  projectId: string;
  markdown: string;
  generatedAt: string;
}

export interface ProjectRemoval {
  project: ProjectSummary;
  removedTags: number;
  removedIcons: number;
  removedTaskRuns: number;
  removedMemoryItems: number;
  removedSummaries: number;
  removedConversation: boolean;
  removedEvents: number;
  removedPendingApprovals: number;
  removedAuditEvents: number;
  filesystemDeleted: boolean;
  pendingLogCleanup: number;
}

export interface ScanRootRemoval {
  root: ScanRoot;
  removedProjects: number;
  orphanedProjects: number;
  pendingLogCleanup: number;
  filesystemDeleted: boolean;
}

export interface PendingApproval {
  id: string;
  projectId: string;
  kind: string;
  title: string;
  detail: string;
  executable: string | null;
  argv: string[];
  cwd: string | null;
  taskId: string | null;
  shellMode?: boolean;
  expectedPorts?: number[];
  devUrlPath?: string | null;
  devUrlScheme?: string | null;
  status: string;
  origin?: string;
  runId?: string | null;
  error?: string | null;
  createdAt: string;
  resolvedAt: string | null;
}

export interface ProjectPatch {
  displayName?: string | null;
  notes?: string | null;
  description?: string | null;
  favorite?: boolean;
  archived?: boolean;
  tags?: string[];
  tasks?: TaskDefinition[];
}

export interface ProjectQuery {
  section?: string | null;
  search?: string | null;
  includeArchived?: boolean;
  collectionId?: string | null;
  limit?: number;
}

export interface ProjectCollection {
  id: string;
  name: string;
  description: string | null;
  projectCount: number;
  createdAt: string;
  updatedAt: string;
}

export interface CollectionUpsert {
  name: string;
  description?: string | null;
}

export interface ProjectBrief {
  project: ProjectSummary;
  facts: DetectedFact[];
  environment: EnvironmentInspection;
  git: GitSnapshot | null;
  readmePath: string | null;
  projectFiles: ProjectFile[];
  startHere: StartHereTask[];
  tasks: TaskDefinition[];
  recentRuns: TaskRun[];
  recentActivity: ProjectEvent[];
}

export interface AttentionItem {
  id: string;
  kind: string;
  severity: "action" | "warning" | "error" | string;
  projectId: string | null;
  runId: string | null;
  approvalId: string | null;
  title: string;
  detail: string;
  occurredAt: string;
  sourceVersion: string;
  acknowledgeable: boolean;
}

export interface AttentionCenterState {
  approvals: PendingApproval[];
  items: AttentionItem[];
}

export interface DashboardTaskRunSummary {
  kind: string;
  status: string;
  startedAt: string;
  finishedAt: string | null;
}

export interface DashboardProjectItem {
  project: ProjectSummary;
  git: GitSnapshot | null;
  latestRun: DashboardTaskRunSummary | null;
}

export interface DashboardTaskRunItem {
  run: TaskRun;
  projectName: string;
}

export interface DashboardCollectionSummary {
  id: string;
  name: string;
  description: string | null;
  projectCount: number;
  archivedProjectCount: number;
  activeRunCount: number;
  attentionCount: number;
  lastActivityAt: string | null;
}

export interface DashboardTaskRunStats {
  total: number;
  succeeded: number;
  failed: number;
  cancelled: number;
}

export interface DashboardSnapshot {
  generatedAt: string;
  projectCount: number;
  availableProjectCount: number;
  unavailableProjectCount: number;
  collectionCount: number;
  activeRunCount: number;
  attentionCount: number;
  sevenDayRuns: DashboardTaskRunStats;
  collections: DashboardCollectionSummary[];
  recentProjects: DashboardProjectItem[];
  recentRuns: DashboardTaskRunItem[];
  attentionPreview: AttentionItem[];
}

export interface ScanProgress {
  scanId: string;
  rootPath: string;
  phase: string;
  visited: number;
  discovered: number;
  currentPath: string | null;
  message: string | null;
}

export interface ScanResult {
  scanId: string;
  visited: number;
  discovered: number;
  unavailable: number;
  cancelled: boolean;
  errors: string[];
}

export interface SearchHit {
  project: ProjectSummary;
  score: number;
}

export interface AppSettings {
  theme: "system" | "light" | "dark" | string;
  locale: "system" | "zh" | "en" | string;
  uiFont?: string;
  consoleFont?: string;
}

export interface ExternalTool {
  id: string;
  kind: "ide" | "terminal" | "agent" | string;
  name: string;
}

export interface ExternalTools {
  agents: ExternalTool[];
  ides: ExternalTool[];
  terminals: ExternalTool[];
}

export type UpdateStatus =
  | "idle"
  | "checking"
  | "available"
  | "downloading"
  | "ready"
  | "error";

export type UpdateErrorStage = "check" | "download" | "install" | "restart";

export interface UpdateState {
  status: UpdateStatus;
  currentVersion: string;
  version?: string;
  notes?: string;
  date?: string;
  downloadedBytes: number;
  contentLength?: number;
  checkedAt?: string;
  error?: string;
  errorStage?: UpdateErrorStage;
  restartRequired: boolean;
  installed?: boolean;
  deferred?: boolean;
}

export interface Bootstrap {
  dataVersion: number;
  settings: AppSettings;
  scanRoots: ScanRoot[];
  projects: ProjectSummary[];
  collections: ProjectCollection[];
}

export interface ReadmeDocument {
  path: string;
  content: string;
  truncated: boolean;
}

export type ProjectFileEntryKind = "file" | "directory" | "symlink" | "other";

export interface ProjectDirectoryEntry {
  name: string;
  path: string;
  kind: ProjectFileEntryKind;
  generated: boolean;
}

export interface ProjectDirectoryListing {
  path: string;
  entries: ProjectDirectoryEntry[];
  skippedCount: number;
}

export type ProjectFilePreviewKind = "markdown" | "code" | "text" | "image" | "unsupported";

export interface ProjectFilePreview {
  path: string;
  kind: ProjectFilePreviewKind;
  size: number;
  mime: string | null;
  language: string | null;
  content: string | null;
  truncated: boolean;
  width: number | null;
  height: number | null;
  message: string | null;
}

export type ProjectPathIndexState = "idle" | "building" | "ready" | "limited" | "failed" | "canceled";

export interface ProjectPathIndexStatus {
  projectId: string;
  generation: number;
  state: ProjectPathIndexState;
  scannedCount: number;
  indexedCount: number;
  includeGenerated: boolean;
  message: string | null;
}

export interface ProjectPathSearchResult {
  name: string;
  path: string;
  kind: ProjectFileEntryKind;
  score: number;
}

export interface ProjectPathSearchResponse {
  status: ProjectPathIndexStatus;
  results: ProjectPathSearchResult[];
}

export interface McpSetupInfo {
  dbPath: string;
  platform: string;
  binaryName: string;
  binaryPath: string | null;
  binaryOrigin?: "installed" | "development" | null;
  workspacePath: string | null;
}
