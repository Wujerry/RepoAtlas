export type AppView = "library" | "settings" | "help";
export type ProjectScope = "projects" | "favorites" | "recent" | "archived";
export type ProjectTab = "overview" | "git" | "tasks" | "knowledge";

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
  matchState: "match" | "mismatch" | "undeclared" | "missing" | "unknown" | string;
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

export interface Bootstrap {
  settings: AppSettings;
  scanRoots: ScanRoot[];
  projects: ProjectSummary[];
}

export interface ReadmeDocument {
  path: string;
  content: string;
  truncated: boolean;
}

export interface McpSetupInfo {
  dbPath: string;
  platform: string;
  binaryName: string;
  binaryPath: string | null;
  workspacePath: string | null;
}

export interface ProviderProfile {
  id: string;
  name: string;
  protocol: string;
  baseUrl: string | null;
  model: string;
  credentialRef: string;
  createdAt: string;
  updatedAt: string;
}

export interface ProviderPreset {
  name: string;
  protocol: string;
  baseUrl: string | null;
  defaultModel: string;
  credentialRef: string;
}

export interface ProviderUpsert {
  id?: string | null;
  name: string;
  protocol: string;
  baseUrl?: string | null;
  model: string;
  credentialRef: string;
}

export interface AiMemoryItem {
  id: string;
  projectId: string;
  text: string;
  createdAt: string;
}

export interface AiSummary {
  id: string;
  projectId: string;
  providerId: string | null;
  model: string | null;
  evidenceSnapshot?: string | null;
  text: string;
  createdAt: string;
}

export interface AnalysisPlan {
  providerId: string;
  providerName: string;
  model: string;
  baseUrl: string | null;
  evidenceFiles: string[];
  characterCount: number;
  isLocal: boolean;
  suspiciousSecretCount: number;
}

export interface ChatMessage {
  role: string;
  content: string;
}

export interface ConversationSummary {
  projectId: string;
  messageCount: number;
  updatedAt: string;
}
