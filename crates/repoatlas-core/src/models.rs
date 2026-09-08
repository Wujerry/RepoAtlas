use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScanRoot {
    pub id: String,
    pub path: String,
    pub created_at: String,
    pub last_scanned_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    #[serde(default)]
    pub directory_group: bool,
    pub id: String,
    pub canonical_path: String,
    pub display_name: String,
    pub detected_name: Option<String>,
    pub notes: Option<String>,
    pub description: Option<String>,
    pub vcs_kind: String,
    pub availability: String,
    pub archived: bool,
    pub favorite: bool,
    pub origin: String,
    pub scan_root_id: Option<String>,
    pub languages: Vec<String>,
    pub frameworks: Vec<String>,
    pub package_managers: Vec<String>,
    pub tags: Vec<String>,
    pub source_mtime: Option<String>,
    pub last_commit_at: Option<String>,
    pub last_opened_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DetectedFact {
    pub kind: String,
    pub value: String,
    pub confidence: f32,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskDefinition {
    pub id: String,
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub executable: String,
    pub argv: Vec<String>,
    pub cwd: Option<String>,
    pub inferred: bool,
    #[serde(default)]
    pub shell_mode: bool,
    #[serde(default)]
    pub expected_ports: Vec<u16>,
    #[serde(default)]
    pub dev_url_path: Option<String>,
    #[serde(default)]
    pub dev_url_scheme: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GitSnapshot {
    pub branch: Option<String>,
    pub dirty: Option<bool>,
    pub ahead: Option<i64>,
    pub behind: Option<i64>,
    pub last_commit_sha: Option<String>,
    pub last_commit_subject: Option<String>,
    pub last_commit_at: Option<String>,
    pub observed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GitFileStatus {
    pub path: String,
    #[serde(default)]
    pub original_path: Option<String>,
    pub status: String,
    pub staged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GitLogEntry {
    pub sha: String,
    pub subject: String,
    pub author: String,
    pub committed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GitStatus {
    pub snapshot: GitSnapshot,
    pub files: Vec<GitFileStatus>,
    pub branches: Vec<String>,
    pub log: Vec<GitLogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GitDiff {
    pub path: Option<String>,
    pub staged: bool,
    pub patch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum GitOp {
    Status,
    Diff { path: Option<String>, staged: bool },
    Log { limit: Option<u32> },
    Branch,
    Checkout { branch: String, create: bool },
    Stage { paths: Vec<String> },
    Unstage { paths: Vec<String> },
    Commit { message: String },
    Pull,
    Push,
    StashPush { message: Option<String> },
    StashPop,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GitCommandResult {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskRun {
    pub id: String,
    pub project_id: String,
    pub task_id: Option<String>,
    pub kind: String,
    pub executable: String,
    pub argv: Vec<String>,
    pub cwd: String,
    pub shell_mode: bool,
    pub status: String,
    pub exit_code: Option<i32>,
    pub log_path: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    #[serde(default)]
    pub peak_cpu_percent: Option<f32>,
    #[serde(default)]
    pub peak_memory_bytes: Option<u64>,
    #[serde(default)]
    pub observed_ports: Vec<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskRuntimeSnapshot {
    pub run_id: String,
    pub captured_at: String,
    pub cpu_percent: f32,
    pub peak_cpu_percent: f32,
    pub memory_bytes: u64,
    pub peak_memory_bytes: u64,
    pub process_count: usize,
    pub ports: Vec<u16>,
    pub port_inspection_available: bool,
    pub endpoints: Vec<String>,
    pub conflicts: Vec<PortConflict>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PortConflict {
    pub port: u16,
    pub pid: Option<u32>,
    pub process_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskSpec {
    pub project_id: String,
    pub task_id: Option<String>,
    pub kind: String,
    pub executable: String,
    pub argv: Vec<String>,
    pub cwd: Option<String>,
    pub shell_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LogChunk {
    pub run_id: String,
    pub stream: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DependencySnapshot {
    pub package_manager: Option<String>,
    pub declared: Vec<String>,
    pub lockfile: Option<String>,
    pub observed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRequirement {
    pub ecosystem: String,
    pub label: String,
    pub constraint: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFile {
    pub kind: String,
    pub path: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentInspection {
    pub project_id: String,
    pub runtimes: Vec<RuntimeStatus>,
    pub files: Vec<ProjectFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatus {
    pub ecosystem: String,
    pub label: String,
    pub constraint: Option<String>,
    pub source: Option<String>,
    pub local_version: Option<String>,
    pub match_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectIcon {
    pub project_id: String,
    pub kind: String,
    pub source: Option<String>,
    pub mime_type: Option<String>,
    pub data_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetail {
    #[serde(default)]
    pub modules: Vec<ProjectModule>,
    pub project: ProjectSummary,
    pub facts: Vec<DetectedFact>,
    pub readme_path: Option<String>,
    pub readme_excerpt: Option<String>,
    pub tasks: Vec<TaskDefinition>,
    pub git: Option<GitSnapshot>,
    pub dependencies: Option<DependencySnapshot>,
    #[serde(default)]
    pub runtime_requirements: Vec<RuntimeRequirement>,
    #[serde(default)]
    pub project_files: Vec<ProjectFile>,
    #[serde(default)]
    pub start_here: Vec<StartHereTask>,
    #[serde(default)]
    pub lineage: Option<CheckoutLineage>,
    #[serde(default)]
    pub recent_events: Vec<ProjectEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReadmeDocument {
    pub path: String,
    pub content: String,
    pub truncated: bool,
}

/// Cached evidence for a constituent directory, never an implicit Project.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectModule {
    pub id: String,
    pub canonical_path: String,
    pub relative_path: String,
    pub name: String,
    pub languages: Vec<String>,
    pub frameworks: Vec<String>,
    pub package_managers: Vec<String>,
    pub facts: Vec<DetectedFact>,
    pub runtime_requirements: Vec<RuntimeRequirement>,
    pub task_ids: Vec<String>,
    pub observed_at: String,
    pub availability: String,
    pub project_id: Option<String>,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StartHereTask {
    pub task_id: String,
    pub kind: String,
    pub name: String,
    pub source: String,
    pub inferred: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LineageCheckout {
    pub project_id: String,
    pub display_name: String,
    pub canonical_path: String,
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutLineage {
    pub remote_url: Option<String>,
    pub normalized_url: Option<String>,
    pub checkouts: Vec<LineageCheckout>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEvent {
    pub id: String,
    pub project_id: String,
    pub kind: String,
    pub title: String,
    pub detail: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AtlasReport {
    pub project_id: String,
    pub markdown: String,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PendingApproval {
    pub id: String,
    pub project_id: String,
    pub kind: String,
    pub title: String,
    pub detail: String,
    pub executable: Option<String>,
    pub argv: Vec<String>,
    pub cwd: Option<String>,
    pub task_id: Option<String>,
    #[serde(default)]
    pub shell_mode: bool,
    #[serde(default)]
    pub expected_ports: Vec<u16>,
    #[serde(default)]
    pub dev_url_path: Option<String>,
    #[serde(default)]
    pub dev_url_scheme: Option<String>,
    pub status: String,
    #[serde(default)]
    pub origin: String,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuditEvent {
    pub id: String,
    pub origin: String,
    pub action: String,
    pub target_type: String,
    pub target_id: Option<String>,
    pub detail: Option<String>,
    pub outcome: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRemoval {
    pub project: ProjectSummary,
    pub removed_tags: u64,
    pub removed_icons: u64,
    pub removed_task_runs: u64,
    pub removed_memory_items: u64,
    pub removed_summaries: u64,
    pub removed_conversation: bool,
    pub removed_events: u64,
    pub removed_pending_approvals: u64,
    pub removed_audit_events: u64,
    pub filesystem_deleted: bool,
    pub pending_log_cleanup: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScanRootRemoval {
    pub root: ScanRoot,
    pub removed_projects: u64,
    pub orphaned_projects: u64,
    pub pending_log_cleanup: u64,
    pub filesystem_deleted: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPatch {
    pub display_name: Option<String>,
    pub notes: Option<String>,
    /// `None` leaves the description unchanged; `Some(None)` clears it.
    pub description: Option<Option<String>>,
    pub favorite: Option<bool>,
    pub archived: Option<bool>,
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub tasks: Option<Vec<TaskDefinition>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectQuery {
    pub section: Option<String>,
    pub search: Option<String>,
    pub include_archived: Option<bool>,
    #[serde(default)]
    pub collection_id: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCollection {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub project_count: usize,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CollectionUpsert {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBrief {
    #[serde(default)]
    pub modules: Vec<ProjectModule>,
    pub project: ProjectSummary,
    pub facts: Vec<DetectedFact>,
    pub environment: EnvironmentInspection,
    pub git: Option<GitSnapshot>,
    pub readme_path: Option<String>,
    #[serde(default)]
    pub project_files: Vec<ProjectFile>,
    #[serde(default)]
    pub start_here: Vec<StartHereTask>,
    pub tasks: Vec<TaskDefinition>,
    pub recent_runs: Vec<TaskRun>,
    pub recent_activity: Vec<ProjectEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AttentionItem {
    pub id: String,
    pub kind: String,
    pub severity: String,
    pub project_id: Option<String>,
    pub run_id: Option<String>,
    pub approval_id: Option<String>,
    pub title: String,
    pub detail: String,
    pub occurred_at: String,
    pub source_version: String,
    pub acknowledgeable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AttentionCenterState {
    pub approvals: Vec<PendingApproval>,
    pub items: Vec<AttentionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DashboardTaskRunSummary {
    pub kind: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DashboardProjectItem {
    pub project: ProjectSummary,
    pub git: Option<GitSnapshot>,
    pub latest_run: Option<DashboardTaskRunSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DashboardTaskRunItem {
    pub run: TaskRun,
    pub project_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DashboardCollectionSummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub project_count: usize,
    pub archived_project_count: usize,
    pub active_run_count: usize,
    pub attention_count: usize,
    pub last_activity_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct DashboardTaskRunStats {
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub cancelled: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSnapshot {
    pub generated_at: String,
    pub project_count: usize,
    pub available_project_count: usize,
    pub unavailable_project_count: usize,
    pub collection_count: usize,
    pub active_run_count: usize,
    pub attention_count: usize,
    pub seven_day_runs: DashboardTaskRunStats,
    pub collections: Vec<DashboardCollectionSummary>,
    pub recent_projects: Vec<DashboardProjectItem>,
    pub recent_runs: Vec<DashboardTaskRunItem>,
    pub attention_preview: Vec<AttentionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub scan_id: String,
    pub root_path: String,
    pub phase: String,
    pub visited: u64,
    pub discovered: u64,
    pub current_path: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub scan_id: String,
    pub visited: u64,
    pub discovered: u64,
    pub unavailable: u64,
    pub cancelled: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub project: ProjectSummary,
    pub score: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub theme: String,
    pub locale: String,
    #[serde(default)]
    pub ui_font: String,
    #[serde(default)]
    pub console_font: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            locale: "system".into(),
            ui_font: String::new(),
            console_font: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProfile {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub base_url: Option<String>,
    pub model: String,
    #[serde(default, skip_serializing)]
    pub credential_ref: String,
    #[serde(default, skip_serializing)]
    pub credential_blob: Option<Vec<u8>>,
    pub has_credential: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPreset {
    pub name: String,
    pub protocol: String,
    pub base_url: Option<String>,
    pub default_model: String,
    pub credential_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderUpsert {
    pub id: Option<String>,
    pub name: String,
    pub protocol: String,
    pub base_url: Option<String>,
    pub model: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub credential_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiMemoryItem {
    pub id: String,
    pub project_id: String,
    pub text: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AiSummary {
    pub id: String,
    pub project_id: String,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    #[serde(default)]
    pub evidence_snapshot: Option<String>,
    #[serde(default)]
    pub evidence_fingerprint: Option<String>,
    pub text: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSummary {
    pub project_id: String,
    pub message_count: usize,
    pub updated_at: String,
}
