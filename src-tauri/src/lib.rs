use repoatlas_core::{
    launch,
    project_files::{self, ProjectPathIndex, ProjectPathSearchResult},
    scan::ScanEngine,
    AppSettings, AtlasReport, AttentionCenterState, Broker, Core, DashboardSnapshot, ExternalTools,
    GitDiff, GitOp, GitStatus, LogChunk, PendingApproval, PortConflict, ProjectCollection,
    ProjectPatch, ProjectQuery, ProjectRemoval, ProjectSummary, ScanProgress, ScanResult, ScanRoot,
    ScanRootRemoval, SearchHit, TaskDefinition, TaskRun, TaskRuntimeRequest, TaskSpec,
};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

#[derive(Debug, Clone)]
struct TaskRuntimeConfig {
    expected_ports: Vec<u16>,
    dev_url_scheme: Option<String>,
    dev_url_path: Option<String>,
}

#[derive(Debug, Clone)]
struct CachedPortPreflight {
    captured_at: std::time::Instant,
    conflicts: Vec<PortConflict>,
}

pub struct AppState {
    pub core: Mutex<Core>,
    pub broker: Broker,
    pub cancel: Arc<AtomicBool>,
    runtime_configs: Mutex<HashMap<String, TaskRuntimeConfig>>,
    runtime_sampler_started: AtomicBool,
    port_preflights: Mutex<HashMap<(String, String), CachedPortPreflight>>,
    file_indexes: Mutex<HashMap<String, Arc<ProjectFileIndexJob>>>,
    file_index_build_lock: Arc<Mutex<()>>,
    file_search_lock: Arc<Mutex<()>>,
    next_file_index_generation: AtomicU64,
}

struct ProjectFileIndexJob {
    project_id: String,
    generation: u64,
    include_generated: bool,
    cancel: Arc<AtomicBool>,
    scanned: Arc<AtomicUsize>,
    indexed: Arc<AtomicUsize>,
    search_cancel: Mutex<Option<Arc<AtomicBool>>>,
    state: Mutex<ProjectFileIndexJobState>,
}

enum ProjectFileIndexJobState {
    Building,
    Ready(Arc<ProjectPathIndex>),
    Failed(String),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectPathIndexStatus {
    project_id: String,
    generation: u64,
    state: &'static str,
    scanned_count: usize,
    indexed_count: usize,
    include_generated: bool,
    message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectPathSearchResponse {
    status: ProjectPathIndexStatus,
    results: Vec<ProjectPathSearchResult>,
}

impl ProjectFileIndexJob {
    fn status(&self) -> ProjectPathIndexStatus {
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        match &*state {
            ProjectFileIndexJobState::Building => ProjectPathIndexStatus {
                project_id: self.project_id.clone(),
                generation: self.generation,
                state: "building",
                scanned_count: self.scanned.load(Ordering::Relaxed),
                indexed_count: self.indexed.load(Ordering::Relaxed),
                include_generated: self.include_generated,
                message: None,
            },
            ProjectFileIndexJobState::Ready(index) => ProjectPathIndexStatus {
                project_id: self.project_id.clone(),
                generation: self.generation,
                state: if index.canceled {
                    "canceled"
                } else if index.limited {
                    "limited"
                } else {
                    "ready"
                },
                scanned_count: index.scanned_count,
                indexed_count: index.entries.len(),
                include_generated: self.include_generated,
                message: index
                    .limited
                    .then(|| "The path index reached its safety limit.".to_string()),
            },
            ProjectFileIndexJobState::Failed(error) => ProjectPathIndexStatus {
                project_id: self.project_id.clone(),
                generation: self.generation,
                state: "failed",
                scanned_count: self.scanned.load(Ordering::Relaxed),
                indexed_count: self.indexed.load(Ordering::Relaxed),
                include_generated: self.include_generated,
                message: Some(error.clone()),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskPersistenceFailure {
    run_id: String,
    error: String,
    run: Option<TaskRun>,
}

fn retry_core_write<T, F>(mut operation: F) -> repoatlas_core::Result<T>
where
    F: FnMut() -> repoatlas_core::Result<T>,
{
    const ATTEMPTS: usize = 5;
    let mut last_error = None;
    for attempt in 1..=ATTEMPTS {
        match operation() {
            Ok(value) => return Ok(value),
            Err(error) => {
                last_error = Some(error);
                if attempt < ATTEMPTS {
                    std::thread::yield_now();
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
            }
        }
    }
    Err(last_error.expect("retry_core_write always attempts at least once"))
}

fn emit_task_persistence_failure(
    app: &AppHandle,
    run_id: &str,
    error: impl Into<String>,
    run: Option<TaskRun>,
) {
    let payload = TaskPersistenceFailure {
        run_id: run_id.into(),
        error: error.into(),
        run,
    };
    if let Err(emit_error) = app.emit("task://persistence-failed", payload) {
        eprintln!("could not emit task persistence failure for {run_id}: {emit_error}");
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    pub settings: AppSettings,
    pub scan_roots: Vec<ScanRoot>,
    pub projects: Vec<ProjectSummary>,
    pub collections: Vec<ProjectCollection>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpSetupInfo {
    pub db_path: String,
    pub platform: String,
    pub binary_name: String,
    pub binary_path: Option<String>,
    pub binary_origin: Option<String>,
    pub workspace_path: Option<String>,
}

fn is_cargo_target_artifact(path: &std::path::Path) -> bool {
    path.ancestors().any(|dir| {
        matches!(
            dir.file_name().and_then(|name| name.to_str()),
            Some("debug") | Some("release")
        ) && dir
            .parent()
            .is_some_and(|parent| parent.file_name() == Some(std::ffi::OsStr::new("target")))
    })
}

#[cfg(test)]
mod mcp_setup_tests {
    use super::is_cargo_target_artifact;
    use std::path::Path;

    #[test]
    fn classifies_cargo_target_artifacts_as_development() {
        let debug_binary = Path::new("F:\\code\\RepoAtlas")
            .join("target")
            .join("debug")
            .join("repoatlas-mcp.exe");
        let release_binary = Path::new("F:\\code\\RepoAtlas")
            .join("target")
            .join("release")
            .join("repoatlas-mcp.exe");
        assert!(is_cargo_target_artifact(&debug_binary));
        assert!(is_cargo_target_artifact(&release_binary));
    }

    #[test]
    fn classifies_installed_binaries_as_installed() {
        let installed = Path::new("C:\\Program Files\\RepoAtlas").join("repoatlas-mcp.exe");
        let resource_binary = Path::new("C:\\Program Files\\RepoAtlas")
            .join("resources")
            .join("repoatlas-mcp")
            .join("repoatlas-mcp.exe");
        assert!(!is_cargo_target_artifact(&installed));
        assert!(!is_cargo_target_artifact(&resource_binary));
    }
}

#[tauri::command]
fn bootstrap(state: State<Arc<AppState>>) -> Result<Bootstrap, String> {
    let core = state.core.lock().map_err(|err| err.to_string())?;
    Ok(Bootstrap {
        settings: core.settings().map_err(err_to_string)?,
        scan_roots: core.list_scan_roots().map_err(err_to_string)?,
        projects: core
            .list_projects(ProjectQuery::default())
            .map_err(err_to_string)?,
        collections: core.list_collections().map_err(err_to_string)?,
    })
}

#[tauri::command]
fn get_settings(state: State<Arc<AppState>>) -> Result<AppSettings, String> {
    state
        .core
        .lock()
        .map_err(|err| err.to_string())?
        .settings()
        .map_err(err_to_string)
}

#[tauri::command]
fn update_settings(
    state: State<Arc<AppState>>,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    state
        .core
        .lock()
        .map_err(|err| err.to_string())?
        .update_settings(settings)
        .map_err(err_to_string)
}

#[tauri::command]
async fn export_json(state: State<'_, Arc<AppState>>) -> Result<String, String> {
    run_blocking(state.inner().clone(), |core| core.export_json()).await
}

#[tauri::command]
async fn export_json_to(state: State<'_, Arc<AppState>>, path: String) -> Result<(), String> {
    let data = run_blocking(state.inner().clone(), |core| core.export_json()).await?;
    tauri::async_runtime::spawn_blocking(move || {
        std::fs::write(path, data).map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
async fn import_json(state: State<'_, Arc<AppState>>, data: String) -> Result<usize, String> {
    run_blocking(state.inner().clone(), move |core| core.import_json(&data)).await
}

#[tauri::command]
async fn import_json_from(state: State<'_, Arc<AppState>>, path: String) -> Result<usize, String> {
    let data = tauri::async_runtime::spawn_blocking(move || {
        std::fs::read_to_string(path).map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())??;
    run_blocking(state.inner().clone(), move |core| core.import_json(&data)).await
}

#[tauri::command]
async fn backup_db(state: State<'_, Arc<AppState>>, dest: String) -> Result<(), String> {
    run_blocking(state.inner().clone(), move |core| core.backup_db(dest)).await
}

#[tauri::command]
fn list_scan_roots(state: State<Arc<AppState>>) -> Result<Vec<ScanRoot>, String> {
    state
        .core
        .lock()
        .map_err(|err| err.to_string())?
        .list_scan_roots()
        .map_err(err_to_string)
}

#[tauri::command]
fn add_scan_root(state: State<Arc<AppState>>, path: String) -> Result<ScanRoot, String> {
    let core = state.core.lock().map_err(|err| err.to_string())?;
    core.add_scan_root_with_origin(&path, "desktop")
        .map_err(err_to_string)
}

#[tauri::command]
fn remove_scan_root(
    state: State<Arc<AppState>>,
    id: String,
    also_remove_records: bool,
) -> Result<ScanRootRemoval, String> {
    let core = state.core.lock().map_err(|err| err.to_string())?;
    core.remove_scan_root_with_origin(&id, also_remove_records, "desktop")
        .map_err(err_to_string)
}

#[tauri::command]
fn list_projects(
    state: State<Arc<AppState>>,
    query: ProjectQuery,
) -> Result<Vec<ProjectSummary>, String> {
    state
        .core
        .lock()
        .map_err(|err| err.to_string())?
        .list_projects(query)
        .map_err(err_to_string)
}

#[tauri::command]
fn list_collections(state: State<Arc<AppState>>) -> Result<Vec<ProjectCollection>, String> {
    state
        .core
        .lock()
        .map_err(|error| error.to_string())?
        .list_collections()
        .map_err(err_to_string)
}

#[tauri::command]
fn create_collection(
    state: State<Arc<AppState>>,
    upsert: repoatlas_core::CollectionUpsert,
) -> Result<ProjectCollection, String> {
    state
        .core
        .lock()
        .map_err(|error| error.to_string())?
        .create_collection_with_origin(upsert, "desktop")
        .map_err(err_to_string)
}

#[tauri::command]
fn update_collection(
    state: State<Arc<AppState>>,
    id: String,
    upsert: repoatlas_core::CollectionUpsert,
) -> Result<ProjectCollection, String> {
    state
        .core
        .lock()
        .map_err(|error| error.to_string())?
        .update_collection_with_origin(&id, upsert, "desktop")
        .map_err(err_to_string)
}

#[tauri::command]
fn delete_collection(state: State<Arc<AppState>>, id: String) -> Result<(), String> {
    state
        .core
        .lock()
        .map_err(|error| error.to_string())?
        .delete_collection_with_origin(&id, "desktop")
        .map_err(err_to_string)
}

#[tauri::command]
fn collection_member_ids(state: State<Arc<AppState>>, id: String) -> Result<Vec<String>, String> {
    state
        .core
        .lock()
        .map_err(|error| error.to_string())?
        .collection_member_ids(&id)
        .map_err(err_to_string)
}

#[tauri::command]
fn set_collection_members(
    state: State<Arc<AppState>>,
    id: String,
    project_ids: Vec<String>,
) -> Result<ProjectCollection, String> {
    state
        .core
        .lock()
        .map_err(|error| error.to_string())?
        .set_collection_members_with_origin(&id, &project_ids, "desktop")
        .map_err(err_to_string)
}

#[tauri::command]
fn save_collection(
    state: State<Arc<AppState>>,
    id: Option<String>,
    upsert: repoatlas_core::CollectionUpsert,
    project_ids: Vec<String>,
) -> Result<ProjectCollection, String> {
    state
        .core
        .lock()
        .map_err(|error| error.to_string())?
        .save_collection_with_members_with_origin(id.as_deref(), upsert, &project_ids, "desktop")
        .map_err(err_to_string)
}

#[tauri::command]
fn search_projects(state: State<Arc<AppState>>, query: String) -> Result<Vec<SearchHit>, String> {
    state
        .core
        .lock()
        .map_err(|err| err.to_string())?
        .search_projects(&query, 50)
        .map_err(err_to_string)
}

#[tauri::command]
fn get_project(
    state: State<Arc<AppState>>,
    id: String,
) -> Result<repoatlas_core::ProjectDetail, String> {
    state
        .core
        .lock()
        .map_err(|err| err.to_string())?
        .get_project(&id)
        .map_err(err_to_string)
}

#[tauri::command]
async fn get_project_brief(
    state: State<'_, Arc<AppState>>,
    project_id: String,
) -> Result<repoatlas_core::ProjectBrief, String> {
    run_blocking(state.inner().clone(), move |core| {
        core.get_project_brief(&project_id)
    })
    .await
}

#[tauri::command]
async fn read_project_readme(
    state: State<'_, Arc<AppState>>,
    project_id: String,
) -> Result<repoatlas_core::ReadmeDocument, String> {
    run_blocking(state.inner().clone(), move |core| {
        core.read_project_readme(&project_id)
    })
    .await
}

#[tauri::command]
async fn read_project_document(
    state: State<'_, Arc<AppState>>,
    project_id: String,
    path: String,
) -> Result<repoatlas_core::ReadmeDocument, String> {
    run_blocking(state.inner().clone(), move |core| {
        core.read_project_document(&project_id, &path)
    })
    .await
}

#[tauri::command]
fn mcp_setup_info(app: AppHandle) -> Result<McpSetupInfo, String> {
    let binary_name = if cfg!(target_os = "windows") {
        "repoatlas-mcp.exe".to_string()
    } else {
        "repoatlas-mcp".to_string()
    };
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(PathBuf::from));
    let mut candidates = Vec::new();
    if let Some(dir) = &exe_dir {
        candidates.push(dir.join(&binary_name));
        candidates.push(dir.join("repoatlas-mcp").join(&binary_name));
    }
    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join(&binary_name));
        candidates.push(resource_dir.join("repoatlas-mcp").join(&binary_name));
        candidates.push(
            resource_dir
                .join("resources")
                .join("repoatlas-mcp")
                .join(&binary_name),
        );
    }
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(PathBuf::from)
        .filter(|path| {
            path.join("crates")
                .join("repoatlas-mcp")
                .join("Cargo.toml")
                .is_file()
        });
    if let Some(workspace) = &workspace {
        candidates.push(workspace.join("target").join("release").join(&binary_name));
        candidates.push(workspace.join("target").join("debug").join(&binary_name));
    }
    let binary_path = candidates.into_iter().find(|path| path.is_file());
    let binary_origin = binary_path.as_ref().map(|path| {
        if is_cargo_target_artifact(path) {
            "development"
        } else {
            "installed"
        }
    });
    Ok(McpSetupInfo {
        db_path: db_path(&app)?.to_string_lossy().into_owned(),
        platform: std::env::consts::OS.to_string(),
        binary_name,
        binary_path: binary_path.map(|path| path.to_string_lossy().into_owned()),
        binary_origin: binary_origin.map(str::to_string),
        workspace_path: workspace.map(|path| path.to_string_lossy().into_owned()),
    })
}

#[tauri::command]
async fn register_project(
    state: State<'_, Arc<AppState>>,
    path: String,
) -> Result<ProjectSummary, String> {
    run_blocking(state.inner().clone(), move |core| {
        core.register_project_with_origin(path, "desktop")
    })
    .await
}

#[tauri::command]
fn update_project(
    state: State<Arc<AppState>>,
    id: String,
    patch: ProjectPatch,
) -> Result<ProjectSummary, String> {
    let core = state.core.lock().map_err(|err| err.to_string())?;
    core.update_project_with_origin(&id, patch, "desktop")
        .map_err(err_to_string)
}

#[tauri::command]
fn remove_project(state: State<Arc<AppState>>, id: String) -> Result<ProjectRemoval, String> {
    let core = state.core.lock().map_err(|err| err.to_string())?;
    core.remove_project_with_origin(&id, "desktop")
        .map_err(err_to_string)
}

#[tauri::command]
fn remove_folder_records(
    state: State<Arc<AppState>>,
    project_ids: Vec<String>,
    scan_root_ids: Vec<String>,
) -> Result<Vec<ProjectRemoval>, String> {
    let core = state.core.lock().map_err(|err| err.to_string())?;
    core.remove_folder_records_with_origin(&project_ids, &scan_root_ids, "desktop")
        .map_err(err_to_string)
}

#[tauri::command]
async fn refresh_project(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<ProjectSummary, String> {
    run_blocking(state.inner().clone(), move |core| {
        core.refresh_project_with_origin(&id, "desktop")
    })
    .await
}

#[tauri::command]
fn mark_project_opened(state: State<Arc<AppState>>, id: String) -> Result<ProjectSummary, String> {
    state
        .core
        .lock()
        .map_err(|err| err.to_string())?
        .mark_opened_with_origin(&id, "desktop")
        .map_err(err_to_string)
}

#[tauri::command]
async fn relocate_project(
    state: State<'_, Arc<AppState>>,
    id: String,
    path: String,
) -> Result<ProjectSummary, String> {
    run_blocking(state.inner().clone(), move |core| {
        core.relocate_project_with_origin(&id, path, "desktop")
    })
    .await
}

#[tauri::command]
fn atlas_report(state: State<Arc<AppState>>, project_id: String) -> Result<AtlasReport, String> {
    state
        .core
        .lock()
        .map_err(|err| err.to_string())?
        .atlas_report(&project_id)
        .map_err(err_to_string)
}

#[tauri::command]
fn export_atlas_report_to(
    state: State<Arc<AppState>>,
    project_id: String,
    path: String,
) -> Result<(), String> {
    let report = state
        .core
        .lock()
        .map_err(|err| err.to_string())?
        .atlas_report(&project_id)
        .map_err(err_to_string)?;
    std::fs::write(path, report.markdown).map_err(|err| err.to_string())?;
    state
        .core
        .lock()
        .map_err(|err| err.to_string())?
        .record_audit_event(
            "desktop",
            "export_atlas_report",
            "project",
            Some(&project_id),
            Some("report exported"),
            "success",
        )
        .map(|_| ())
        .map_err(err_to_string)
}

#[tauri::command]
fn list_pending_approvals(state: State<Arc<AppState>>) -> Result<Vec<PendingApproval>, String> {
    state
        .core
        .lock()
        .map_err(|err| err.to_string())?
        .list_pending_approvals()
        .map_err(err_to_string)
}

#[tauri::command]
fn list_attention_items(
    state: State<Arc<AppState>>,
) -> Result<Vec<repoatlas_core::AttentionItem>, String> {
    state
        .core
        .lock()
        .map_err(|error| error.to_string())?
        .list_attention_items()
        .map_err(err_to_string)
}

#[tauri::command]
fn get_attention_center(state: State<Arc<AppState>>) -> Result<AttentionCenterState, String> {
    state
        .core
        .lock()
        .map_err(|error| error.to_string())?
        .attention_center()
        .map_err(err_to_string)
}

#[tauri::command]
fn get_dashboard_snapshot(state: State<Arc<AppState>>) -> Result<DashboardSnapshot, String> {
    state
        .core
        .lock()
        .map_err(|err| err.to_string())?
        .dashboard_snapshot()
        .map_err(err_to_string)
}

#[tauri::command]
fn acknowledge_attention_item(
    app: AppHandle,
    state: State<Arc<AppState>>,
    item_id: String,
    source_version: String,
) -> Result<(), String> {
    state
        .core
        .lock()
        .map_err(|error| error.to_string())?
        .acknowledge_attention_item(&item_id, &source_version)
        .map_err(err_to_string)?;
    let _ = app.emit("attention://changed", ());
    Ok(())
}

#[tauri::command]
fn start_scan(
    app: AppHandle,
    state: State<Arc<AppState>>,
    root_id: Option<String>,
) -> Result<(), String> {
    state.cancel.store(false, Ordering::Relaxed);
    let cancel = state.cancel.clone();
    let roots = {
        let core = state.core.lock().map_err(|err| err.to_string())?;
        if let Some(root_id) = root_id {
            vec![(
                root_id.clone(),
                core.scan_root_path(&root_id).map_err(err_to_string)?,
            )]
        } else {
            core.list_scan_roots()
                .map_err(err_to_string)?
                .into_iter()
                .map(|root| (root.id, root.path))
                .collect()
        }
    };
    let app_handle = app.clone();
    std::thread::spawn(move || {
        let Some(core_state) = app_handle.try_state::<Arc<AppState>>() else {
            return;
        };
        let mut results = Vec::new();
        for (root_id, root_path) in roots {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            match walk_scan_root(
                &app_handle,
                core_state.inner().as_ref(),
                &root_id,
                &root_path,
                &cancel,
            ) {
                Ok(result) => results.push(result),
                Err(err) => {
                    let _ = app_handle.emit("scan://failed", err);
                    return;
                }
            }
        }
        let _ = app_handle.emit("scan://completed", results);
    });
    Ok(())
}

#[tauri::command]
fn cancel_scan(state: State<Arc<AppState>>) -> Result<(), String> {
    state.cancel.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
async fn git_status(
    state: State<'_, Arc<AppState>>,
    project_id: String,
) -> Result<GitStatus, String> {
    let root = project_root_for_files(state.inner().clone(), project_id).await?;
    tauri::async_runtime::spawn_blocking(move || {
        repoatlas_core::git::status(&root).map_err(err_to_string)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn git_diff(
    state: State<'_, Arc<AppState>>,
    project_id: String,
    path: Option<String>,
    staged: bool,
) -> Result<GitDiff, String> {
    let root = project_root_for_files(state.inner().clone(), project_id).await?;
    tauri::async_runtime::spawn_blocking(move || {
        repoatlas_core::git::diff(&root, path.as_deref(), staged).map_err(err_to_string)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn git_execute(
    state: State<'_, Arc<AppState>>,
    project_id: String,
    op: GitOp,
) -> Result<repoatlas_core::GitCommandResult, String> {
    run_blocking(state.inner().clone(), move |core| {
        core.git_execute(&project_id, op)
    })
    .await
}

#[tauri::command]
fn list_task_runs(state: State<Arc<AppState>>, project_id: String) -> Result<Vec<TaskRun>, String> {
    state
        .core
        .lock()
        .map_err(|err| err.to_string())?
        .list_task_runs(&project_id)
        .map_err(err_to_string)
}

#[tauri::command]
fn list_active_task_runs(state: State<Arc<AppState>>) -> Result<Vec<TaskRun>, String> {
    state
        .core
        .lock()
        .map_err(|err| err.to_string())?
        .list_active_task_runs()
        .map_err(err_to_string)
}

#[tauri::command]
fn get_task_run(state: State<Arc<AppState>>, run_id: String) -> Result<TaskRun, String> {
    state
        .core
        .lock()
        .map_err(|error| error.to_string())?
        .get_task_run(&run_id)
        .map_err(err_to_string)
}

#[tauri::command]
fn read_task_log(state: State<Arc<AppState>>, run_id: String) -> Result<String, String> {
    state
        .core
        .lock()
        .map_err(|err| err.to_string())?
        .read_task_log(&run_id)
        .map_err(err_to_string)
}

#[tauri::command]
fn start_task(
    app: AppHandle,
    state: State<Arc<AppState>>,
    project_id: String,
    task_id: String,
    allow_port_conflicts: Option<bool>,
) -> Result<TaskRun, String> {
    let preflight = state
        .port_preflights
        .lock()
        .map_err(|error| error.to_string())?
        .remove(&(project_id.clone(), task_id.clone()))
        .filter(|cached| cached.captured_at.elapsed() < std::time::Duration::from_secs(3))
        .map(|cached| cached.conflicts);
    let (spec, runtime_task) = {
        let core = state.core.lock().map_err(|err| err.to_string())?;
        let (canonical_path, task) = core
            .task_for_start(&project_id, &task_id)
            .map_err(err_to_string)?;
        let project_path = PathBuf::from(canonical_path);
        let cwd = task.cwd.as_deref().map_or_else(
            || project_path.clone(),
            |value| {
                let configured = PathBuf::from(value);
                if configured.is_absolute() {
                    configured
                } else {
                    project_path.join(configured)
                }
            },
        );
        (
            TaskSpec {
                project_id,
                task_id: Some(task.id.clone()),
                kind: task.kind.clone(),
                executable: task.executable.clone(),
                argv: task.argv.clone(),
                cwd: Some(cwd.to_string_lossy().into_owned()),
                shell_mode: task.shell_mode,
            },
            task,
        )
    };
    start_task_spec(
        &app,
        state.inner().as_ref(),
        spec,
        allow_port_conflicts.unwrap_or(false),
        Some(runtime_task),
        preflight,
    )
}

fn start_task_spec(
    app: &AppHandle,
    state: &AppState,
    spec: TaskSpec,
    allow_port_conflicts: bool,
    supplied_runtime_metadata: Option<TaskDefinition>,
    supplied_preflight: Option<Vec<PortConflict>>,
) -> Result<TaskRun, String> {
    let core = state.core.lock().map_err(|err| err.to_string())?;
    let runtime_metadata = supplied_runtime_metadata.or_else(|| {
        spec.task_id.as_deref().and_then(|task_id| {
            core.get_project(&spec.project_id)
                .ok()?
                .tasks
                .into_iter()
                .find(|task| task.id == task_id)
        })
    });
    drop(core);
    if let Some(task) = runtime_metadata.as_ref().filter(|_| !allow_port_conflicts) {
        let conflicts = match supplied_preflight {
            Some(conflicts) => conflicts,
            None => state
                .broker
                .preflight_ports(&task.expected_ports)
                .map_err(err_to_string)?,
        };
        if !conflicts.is_empty() {
            let detail = conflicts
                .iter()
                .map(|conflict| match (&conflict.process_name, conflict.pid) {
                    (Some(name), Some(pid)) => format!("{} ({name}, PID {pid})", conflict.port),
                    (_, Some(pid)) => format!("{} (PID {pid})", conflict.port),
                    _ => conflict.port.to_string(),
                })
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "Expected development port is already in use: {detail}"
            ));
        }
    }
    let app_handle = app.clone();
    let core = state.core.lock().map_err(|err| err.to_string())?;
    let run = state
        .broker
        .start(
            core.connection(),
            spec,
            {
                let app_handle = app_handle.clone();
                move |chunk: LogChunk| {
                    let _ = app_handle.emit("task://log", chunk);
                }
            },
            {
                let app_handle = app_handle.clone();
                move |run_id, code| {
                    let app_handle = app_handle.clone();
                    std::thread::spawn(move || {
                        let Some(state) = app_handle.try_state::<Arc<AppState>>() else {
                            emit_task_persistence_failure(
                                &app_handle,
                                &run_id,
                                "application state was unavailable while finishing the task",
                                None,
                            );
                            return;
                        };
                        let core = match state.core.lock() {
                            Ok(core) => core,
                            Err(error) => {
                                emit_task_persistence_failure(
                                    &app_handle,
                                    &run_id,
                                    format!("task persistence lock failed: {error}"),
                                    None,
                                );
                                return;
                            }
                        };
                        let status = if code == Some(0) {
                            "succeeded"
                        } else {
                            "failed"
                        };
                        match retry_core_write(|| core.finish_task_run(&run_id, status, code)) {
                            Ok(run) => {
                                let mut persistence_errors = Vec::new();
                                if let Err(error) = retry_core_write(|| {
                                    core.record_project_event(
                                        &run.project_id,
                                        "task",
                                        "Finished task",
                                        Some(status),
                                    )
                                }) {
                                    persistence_errors.push(format!(
                                        "project completion event could not be persisted: {error}"
                                    ));
                                }
                                if let Err(error) = retry_core_write(|| {
                                    core.record_audit_event(
                                        "desktop",
                                        "finish_task_run",
                                        "task_run",
                                        Some(&run.id),
                                        Some(status),
                                        status,
                                    )
                                }) {
                                    persistence_errors.push(format!(
                                        "task completion audit could not be persisted: {error}"
                                    ));
                                }
                                if !persistence_errors.is_empty() {
                                    emit_task_persistence_failure(
                                        &app_handle,
                                        &run.id,
                                        persistence_errors.join("; "),
                                        Some(run.clone()),
                                    );
                                }
                                if let Err(error) = app_handle.emit("task://exited", &run) {
                                    emit_task_persistence_failure(
                                        &app_handle,
                                        &run.id,
                                        format!("task exit event could not be delivered: {error}"),
                                        Some(run.clone()),
                                    );
                                }
                            }
                            Err(error) => {
                                let finish_error = error.to_string();
                                let mut persistence_error = finish_error.clone();
                                if let Err(audit_error) = retry_core_write(|| {
                                    core.record_audit_event(
                                        "desktop",
                                        "finish_task_run",
                                        "task_run",
                                        Some(&run_id),
                                        Some(&finish_error),
                                        "failed",
                                    )
                                }) {
                                    persistence_error.push_str(&format!(
                                        "; completion failure audit could not be persisted: {audit_error}"
                                    ));
                                }
                                let run = retry_core_write(|| core.get_task_run(&run_id)).ok();
                                emit_task_persistence_failure(
                                    &app_handle,
                                    &run_id,
                                    persistence_error,
                                    run,
                                );
                            }
                        }
                    });
                }
            },
        )
        .map_err(err_to_string)?;
    drop(core);
    let runtime_config = runtime_metadata.map(|task| TaskRuntimeConfig {
        expected_ports: task.expected_ports,
        dev_url_scheme: task.dev_url_scheme,
        dev_url_path: task.dev_url_path,
    });
    if let Some(config) = runtime_config.as_ref() {
        state
            .runtime_configs
            .lock()
            .map_err(|error| error.to_string())?
            .insert(run.id.clone(), config.clone());
    }
    ensure_runtime_sampler(app.clone());
    let core = state.core.lock().map_err(|err| err.to_string())?;
    if let Err(error) = retry_core_write(|| {
        core.record_project_event(&run.project_id, "task", "Started task", Some(&run.kind))
    }) {
        emit_task_persistence_failure(
            app,
            &run.id,
            format!("task start event could not be persisted: {error}"),
            Some(run.clone()),
        );
    }
    if let Err(error) = retry_core_write(|| {
        core.record_audit_event(
            "desktop",
            "start_task",
            "task",
            run.task_id.as_deref().or(Some(&run.project_id)),
            Some(&run.kind),
            "started",
        )
    }) {
        // Do not leave a live process behind when the required audit write
        // fails. Stop/reap the child and close its run record before returning
        // the persistence error to the UI.
        let _ = state.broker.stop(core.connection(), &run.id);
        if let Err(finish_error) =
            retry_core_write(|| core.finish_task_run(&run.id, "failed", None))
        {
            emit_task_persistence_failure(
                app,
                &run.id,
                format!("task start audit failed and task completion could not be persisted: {finish_error}"),
                Some(run.clone()),
            );
        }
        return Err(err_to_string(error));
    }
    Ok(run)
}

#[tauri::command]
fn get_task_runtime_snapshot(
    state: State<Arc<AppState>>,
    run_id: String,
) -> Result<repoatlas_core::TaskRuntimeSnapshot, String> {
    task_runtime_snapshots(state.inner().as_ref(), &[run_id])?
        .pop()
        .ok_or_else(|| "task is not running".to_string())
}

#[tauri::command]
fn get_task_runtime_snapshots(
    state: State<Arc<AppState>>,
    run_ids: Vec<String>,
) -> Result<Vec<repoatlas_core::TaskRuntimeSnapshot>, String> {
    task_runtime_snapshots(state.inner().as_ref(), &run_ids)
}

fn task_runtime_snapshots(
    state: &AppState,
    run_ids: &[String],
) -> Result<Vec<repoatlas_core::TaskRuntimeSnapshot>, String> {
    let configs = state
        .runtime_configs
        .lock()
        .map_err(|error| error.to_string())?;
    let requests = run_ids
        .iter()
        .map(|run_id| {
            let config = configs.get(run_id).cloned().unwrap_or(TaskRuntimeConfig {
                expected_ports: Vec::new(),
                dev_url_scheme: None,
                dev_url_path: None,
            });
            TaskRuntimeRequest {
                run_id: run_id.clone(),
                expected_ports: config.expected_ports,
                dev_url_scheme: config.dev_url_scheme,
                dev_url_path: config.dev_url_path,
            }
        })
        .collect::<Vec<_>>();
    drop(configs);
    let mut snapshots = state
        .broker
        .collect_runtime_snapshots(&requests)
        .map_err(err_to_string)?;
    let core = state.core.lock().map_err(|error| error.to_string())?;
    repoatlas_core::broker::persist_runtime_snapshots(core.connection(), &mut snapshots)
        .map_err(err_to_string)?;
    Ok(snapshots)
}

fn ensure_runtime_sampler(app: AppHandle) {
    let Some(state) = app.try_state::<Arc<AppState>>() else {
        return;
    };
    if state
        .runtime_sampler_started
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_err()
    {
        return;
    }
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(5));
        let Some(state) = app.try_state::<Arc<AppState>>() else {
            return;
        };
        let active = match state.broker.active() {
            Ok(runs) => runs.into_iter().collect::<HashSet<_>>(),
            Err(_) => continue,
        };
        let requests = match state.runtime_configs.lock() {
            Ok(mut configs) => {
                configs.retain(|run_id, _| active.contains(run_id));
                configs
                    .iter()
                    .map(|(run_id, config)| TaskRuntimeRequest {
                        run_id: run_id.clone(),
                        expected_ports: config.expected_ports.clone(),
                        dev_url_scheme: config.dev_url_scheme.clone(),
                        dev_url_path: config.dev_url_path.clone(),
                    })
                    .collect::<Vec<_>>()
            }
            Err(_) => continue,
        };
        let Ok(mut snapshots) = state.broker.collect_runtime_snapshots(&requests) else {
            continue;
        };
        if snapshots.is_empty() {
            continue;
        }
        let persisted = state
            .core
            .lock()
            .map_err(|error| error.to_string())
            .and_then(|core| {
                repoatlas_core::broker::persist_runtime_snapshots(core.connection(), &mut snapshots)
                    .map_err(err_to_string)
            });
        if persisted.is_ok() {
            for snapshot in snapshots {
                let _ = app.emit("task://runtime", snapshot);
            }
        }
    });
}

#[tauri::command]
fn preflight_task_ports(
    state: State<Arc<AppState>>,
    project_id: String,
    task_id: String,
) -> Result<Vec<repoatlas_core::PortConflict>, String> {
    let task = state
        .core
        .lock()
        .map_err(|error| error.to_string())?
        .task_for_start(&project_id, &task_id)
        .map_err(err_to_string)?
        .1;
    let conflicts = state
        .broker
        .preflight_ports(&task.expected_ports)
        .map_err(err_to_string)?;
    let mut preflights = state
        .port_preflights
        .lock()
        .map_err(|error| error.to_string())?;
    preflights.retain(|_, cached| cached.captured_at.elapsed() < std::time::Duration::from_secs(3));
    preflights.insert(
        (project_id, task_id),
        CachedPortPreflight {
            captured_at: std::time::Instant::now(),
            conflicts: conflicts.clone(),
        },
    );
    Ok(conflicts)
}

#[tauri::command]
fn open_dev_endpoint(
    app: AppHandle,
    state: State<Arc<AppState>>,
    run_id: String,
    endpoint: String,
) -> Result<(), String> {
    let snapshot = get_task_runtime_snapshot(state, run_id)?;
    if !snapshot.port_inspection_available
        || !snapshot.endpoints.iter().any(|value| value == &endpoint)
    {
        return Err("development endpoint is no longer owned by this running task".into());
    }
    let url = url::Url::parse(&endpoint).map_err(|error| error.to_string())?;
    if !matches!(url.scheme(), "http" | "https")
        || !matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"))
    {
        return Err("development endpoint must be a local HTTP URL".into());
    }
    app.opener()
        .open_url(endpoint, None::<&str>)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn write_task_stdin(
    state: State<Arc<AppState>>,
    run_id: String,
    text: String,
) -> Result<(), String> {
    state
        .broker
        .write_stdin(&run_id, &text)
        .map_err(err_to_string)
}

#[tauri::command]
fn resize_task_run(
    state: State<Arc<AppState>>,
    run_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    state
        .broker
        .resize(&run_id, cols, rows)
        .map_err(err_to_string)
}

#[tauri::command]
fn show_main_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.show().map_err(|err| err.to_string())?;
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    Ok(())
}

#[tauri::command]
fn stop_task(state: State<Arc<AppState>>, run_id: String) -> Result<TaskRun, String> {
    let core = state.core.lock().map_err(|err| err.to_string())?;
    let run = state
        .broker
        .stop(core.connection(), &run_id)
        .map_err(err_to_string)?;
    let finished = core
        .finish_task_run(&run_id, "cancelled", run.exit_code)
        .map_err(err_to_string)?;
    let _ = core.record_project_event(
        &finished.project_id,
        "task",
        "Stopped task",
        Some("cancelled"),
    );
    core.record_audit_event(
        "desktop",
        "stop_task",
        "task_run",
        Some(&finished.id),
        Some("cancelled"),
        "success",
    )
    .map_err(err_to_string)?;
    Ok(finished)
}

#[tauri::command]
fn open_in_explorer(path: String) -> Result<(), String> {
    launch::open_in_file_manager(&path).map_err(err_to_string)
}

#[tauri::command]
fn list_external_tools() -> ExternalTools {
    launch::list_external_tools()
}

#[tauri::command]
fn open_in_terminal(path: String, terminal: Option<String>) -> Result<(), String> {
    launch::open_in_terminal(&path, terminal.as_deref()).map_err(err_to_string)
}

#[tauri::command]
fn open_in_ide(app: AppHandle, path: String, ide: String) -> Result<(), String> {
    launch::open_in_ide(&path, &ide).map_err(err_to_string)?;
    if let Some(state) = app.try_state::<Arc<AppState>>() {
        if let Ok(core) = state.core.lock() {
            if let Ok(projects) = core.list_projects(ProjectQuery::default()) {
                if let Some(project) = projects
                    .into_iter()
                    .find(|project| project.canonical_path == path)
                {
                    let _ = core.record_project_event(&project.id, "ide", "Opened IDE", Some(&ide));
                    let _ = core.mark_opened(&project.id);
                }
            }
        }
    }
    Ok(())
}

#[tauri::command]
fn open_in_agent(app: AppHandle, path: String, agent: String) -> Result<(), String> {
    launch::open_in_agent(&path, &agent).map_err(err_to_string)?;
    if let Some(state) = app.try_state::<Arc<AppState>>() {
        if let Ok(core) = state.core.lock() {
            if let Ok(projects) = core.list_projects(ProjectQuery::default()) {
                if let Some(project) = projects
                    .into_iter()
                    .find(|project| project.canonical_path == path)
                {
                    let _ = core.record_project_event(
                        &project.id,
                        "agent",
                        "Opened agent",
                        Some(&agent),
                    );
                    let _ = core.mark_opened(&project.id);
                }
            }
        }
    }
    Ok(())
}

#[tauri::command]
fn resolve_pending_approval(
    app: AppHandle,
    state: State<Arc<AppState>>,
    approval_id: String,
    approved: bool,
    allow_port_conflicts: Option<bool>,
) -> Result<PendingApproval, String> {
    let (approval, spec) = {
        let core = state.core.lock().map_err(|err| err.to_string())?;
        core.resolve_task_approval(&approval_id, approved)
            .map_err(err_to_string)?
    };
    if let Some(spec) = spec {
        let run = match start_task_spec(
            &app,
            state.inner().as_ref(),
            spec,
            allow_port_conflicts.unwrap_or(false),
            None,
            None,
        ) {
            Ok(run) => run,
            Err(error) => {
                let failure = state
                    .core
                    .lock()
                    .map_err(|lock_error| lock_error.to_string())
                    .and_then(|core| {
                        core.mark_task_approval_failed(&approval_id, &error)
                            .map(|_| ())
                            .map_err(err_to_string)
                    });
                return match failure {
                    Ok(()) => Err(error),
                    Err(failure_error) => Err(format!(
                        "{error}; approval failure could not be persisted: {failure_error}"
                    )),
                };
            }
        };
        let core = state.core.lock().map_err(|err| err.to_string())?;
        return match core.mark_task_approval_started(&approval_id, &run.id) {
            Ok(updated) => Ok(updated),
            Err(error) => {
                // The process has already been spawned, so a failed
                // transition must actively reap it. Otherwise the database
                // would report a starting approval while the child continues
                // to run without an owned approval record.
                let transition_error = err_to_string(error);
                let cleanup_error = cleanup_failed_approval_run(&state.broker, &core, &run.id);
                let approval_error = cleanup_error.as_ref().map_or_else(
                    || transition_error.clone(),
                    |cleanup_error| format!("{transition_error}; {cleanup_error}"),
                );
                let failure_error = core
                    .mark_task_approval_failed(&approval_id, &approval_error)
                    .err()
                    .map(err_to_string);

                let mut errors = vec![transition_error];
                if let Some(cleanup_error) = cleanup_error {
                    errors.push(format!(
                        "started task cleanup could not be completed: {cleanup_error}"
                    ));
                }
                if let Some(failure_error) = failure_error {
                    errors.push(format!(
                        "approval failure could not be persisted: {failure_error}"
                    ));
                }
                Err(errors.join("; "))
            }
        };
    }
    Ok(approval)
}

/// Clean up the child that was started before the approval transition failed.
///
/// `Broker::stop` normally terminates and reaps the child in one operation. A
/// transient broker/database error can still make that operation return an
/// error after the process has been terminated, or before the broker has had
/// a chance to do so. Retry the bounded operation and verify the broker no
/// longer owns the run. Every cleanup failure is returned to the caller so a
/// failed approval can never look like a successful, fully-cleaned-up start.
fn cleanup_failed_approval_run(broker: &Broker, core: &Core, run_id: &str) -> Option<String> {
    const CLEANUP_ATTEMPTS: usize = 3;

    let mut errors = Vec::new();
    let mut stopped = false;
    for attempt in 1..=CLEANUP_ATTEMPTS {
        match broker.stop(core.connection(), run_id) {
            Ok(_) => {
                stopped = true;
                break;
            }
            Err(error) => {
                errors.push(format!("broker stop attempt {attempt} failed: {error}"));
                if attempt < CLEANUP_ATTEMPTS {
                    std::thread::yield_now();
                }
            }
        }
    }

    match broker.active() {
        Ok(active) if active.iter().any(|active_id| active_id == run_id) => {
            errors.push("task process is still owned by the broker after cleanup attempts".into());
        }
        Ok(_) => {}
        Err(error) => errors.push(format!("could not verify task cleanup: {error}")),
    }

    // Keep the persisted Task Run terminal even when the broker reports a
    // cleanup error. The cleanup error is returned below, while the late
    // broker exit callback remains harmless because finish_task_run is
    // idempotent for terminal runs.
    if let Err(error) = core.finish_task_run(run_id, "failed", None) {
        errors.push(format!("task failure could not be persisted: {error}"));
    }

    if stopped && errors.is_empty() {
        None
    } else if errors.is_empty() {
        Some("broker did not confirm task cleanup".into())
    } else {
        Some(errors.join("; "))
    }
}

#[tauri::command]
async fn inspect_project_environment(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    project_id: String,
) -> Result<repoatlas_core::EnvironmentInspection, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        // Snapshot the database-backed facts quickly, then release the Core
        // mutex while fixed allow-listed version probes run (each with its
        // own two-second deadline).
        let detail = {
            let core = state.core.lock().map_err(|err| err.to_string())?;
            core.get_project(&project_id).map_err(err_to_string)?
        };
        let inspection = repoatlas_core::environment::inspect_environment(&detail);
        {
            let core = state.core.lock().map_err(|err| err.to_string())?;
            core.record_environment_inspection(&inspection)
                .map_err(err_to_string)?;
        }
        Ok(inspection)
    })
    .await
    .map_err(|err| err.to_string())?
    .inspect(|_| {
        let _ = app.emit("attention://changed", ());
    })
}

#[tauri::command]
fn read_project_icons(
    state: State<Arc<AppState>>,
    project_ids: Vec<String>,
) -> Result<Vec<repoatlas_core::ProjectIcon>, String> {
    state
        .core
        .lock()
        .map_err(|err| err.to_string())?
        .read_project_icons(&project_ids)
        .map_err(err_to_string)
}

#[tauri::command]
fn set_project_icon(
    state: State<Arc<AppState>>,
    project_id: String,
    path: String,
) -> Result<repoatlas_core::ProjectIcon, String> {
    let core = state.core.lock().map_err(|err| err.to_string())?;
    core.set_project_icon_with_origin(&project_id, path, "desktop")
        .map_err(err_to_string)
}

#[tauri::command]
fn clear_project_icon(
    state: State<Arc<AppState>>,
    project_id: String,
) -> Result<repoatlas_core::ProjectIcon, String> {
    let core = state.core.lock().map_err(|err| err.to_string())?;
    core.clear_project_icon_with_origin(&project_id, "desktop")
        .map_err(err_to_string)
}

#[tauri::command]
fn read_project_file(
    state: State<Arc<AppState>>,
    project_id: String,
    path: String,
) -> Result<repoatlas_core::ReadmeDocument, String> {
    state
        .core
        .lock()
        .map_err(|err| err.to_string())?
        .read_project_file(&project_id, &path)
        .map_err(err_to_string)
}

async fn project_root_for_files(
    state: Arc<AppState>,
    project_id: String,
) -> Result<PathBuf, String> {
    run_blocking(state, move |core| core.project_path(&project_id)).await
}

#[tauri::command]
async fn list_project_directory(
    state: State<'_, Arc<AppState>>,
    project_id: String,
    path: String,
    show_generated: bool,
) -> Result<repoatlas_core::project_files::ProjectDirectoryListing, String> {
    let root = project_root_for_files(state.inner().clone(), project_id).await?;
    tauri::async_runtime::spawn_blocking(move || {
        project_files::list_directory(&root, &path, show_generated).map_err(err_to_string)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn read_project_file_preview(
    state: State<'_, Arc<AppState>>,
    project_id: String,
    path: String,
) -> Result<repoatlas_core::project_files::ProjectFilePreview, String> {
    let root = project_root_for_files(state.inner().clone(), project_id).await?;
    tauri::async_runtime::spawn_blocking(move || {
        project_files::read_preview(&root, &path).map_err(err_to_string)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn read_project_image(
    state: State<'_, Arc<AppState>>,
    project_id: String,
    path: String,
) -> Result<tauri::ipc::Response, String> {
    let root = project_root_for_files(state.inner().clone(), project_id).await?;
    let bytes = tauri::async_runtime::spawn_blocking(move || {
        project_files::read_image(&root, &path).map_err(err_to_string)
    })
    .await
    .map_err(|error| error.to_string())??;
    Ok(tauri::ipc::Response::new(bytes))
}

#[tauri::command]
async fn ensure_project_path_index(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    project_id: String,
    include_generated: bool,
) -> Result<ProjectPathIndexStatus, String> {
    let app_state = state.inner().clone();
    if let Some(existing) = app_state
        .file_indexes
        .lock()
        .map_err(|error| error.to_string())?
        .get(&project_id)
        .filter(|job| job.include_generated == include_generated)
        .cloned()
    {
        let status = existing.status();
        if status.state != "failed" && status.state != "canceled" {
            return Ok(status);
        }
    }

    let root = project_root_for_files(app_state.clone(), project_id.clone()).await?;
    let generation = app_state
        .next_file_index_generation
        .fetch_add(1, Ordering::Relaxed)
        + 1;
    let job = Arc::new(ProjectFileIndexJob {
        project_id: project_id.clone(),
        generation,
        include_generated,
        cancel: Arc::new(AtomicBool::new(false)),
        scanned: Arc::new(AtomicUsize::new(0)),
        indexed: Arc::new(AtomicUsize::new(0)),
        search_cancel: Mutex::new(None),
        state: Mutex::new(ProjectFileIndexJobState::Building),
    });
    {
        let mut indexes = app_state
            .file_indexes
            .lock()
            .map_err(|error| error.to_string())?;
        if let Some(previous) = indexes.insert(project_id, job.clone()) {
            previous.cancel.store(true, Ordering::Relaxed);
            if let Ok(mut search) = previous.search_cancel.lock() {
                if let Some(cancel) = search.take() {
                    cancel.store(true, Ordering::Relaxed);
                }
            }
        }
    }

    let monitor_job = job.clone();
    let monitor_app = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(250));
        let status = monitor_job.status();
        let building = status.state == "building";
        let _ = monitor_app.emit("files://index-progress", status);
        if !building {
            break;
        }
    });

    let build_job = job.clone();
    let build_lock = app_state.file_index_build_lock.clone();
    tauri::async_runtime::spawn(async move {
        let cancel = build_job.cancel.clone();
        let scanned = build_job.scanned.clone();
        let indexed = build_job.indexed.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            let _guard = build_lock
                .lock()
                .map_err(|_| repoatlas_core::Error::msg("file index build lock was poisoned"))?;
            project_files::build_path_index(&root, include_generated, cancel, scanned, indexed)
        })
        .await;
        let next_state = match result {
            Ok(Ok(index)) => ProjectFileIndexJobState::Ready(Arc::new(index)),
            Ok(Err(error)) => ProjectFileIndexJobState::Failed(error.to_string()),
            Err(error) => ProjectFileIndexJobState::Failed(error.to_string()),
        };
        if let Ok(mut current) = build_job.state.lock() {
            *current = next_state;
        }
        let _ = app.emit("files://index-progress", build_job.status());
    });

    Ok(job.status())
}

#[tauri::command]
async fn search_project_paths(
    state: State<'_, Arc<AppState>>,
    project_id: String,
    query: String,
) -> Result<ProjectPathSearchResponse, String> {
    let job = state
        .file_indexes
        .lock()
        .map_err(|error| error.to_string())?
        .get(&project_id)
        .cloned()
        .ok_or_else(|| "path index has not been started".to_string())?;
    let status = job.status();
    let index = {
        let current = job.state.lock().map_err(|error| error.to_string())?;
        match &*current {
            ProjectFileIndexJobState::Ready(index) => Some(index.clone()),
            _ => None,
        }
    };
    let results = if let Some(index) = index {
        let cancel = Arc::new(AtomicBool::new(false));
        if let Ok(mut active) = job.search_cancel.lock() {
            if let Some(previous) = active.replace(cancel.clone()) {
                previous.store(true, Ordering::Relaxed);
            }
        }
        let search_lock = state.file_search_lock.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let _guard = search_lock.lock().ok();
            project_files::search_path_index_cancellable(&index, &query, 200, &cancel)
        })
        .await
        .map_err(|error| error.to_string())?
    } else {
        Vec::new()
    };
    Ok(ProjectPathSearchResponse { status, results })
}

#[tauri::command]
fn cancel_project_path_search(
    state: State<Arc<AppState>>,
    project_id: String,
) -> Result<(), String> {
    if let Some(job) = state
        .file_indexes
        .lock()
        .map_err(|error| error.to_string())?
        .get(&project_id)
        .cloned()
    {
        if let Ok(mut search) = job.search_cancel.lock() {
            if let Some(cancel) = search.take() {
                cancel.store(true, Ordering::Relaxed);
            }
        }
    }
    Ok(())
}

#[tauri::command]
fn cancel_project_path_index(
    state: State<Arc<AppState>>,
    project_id: String,
) -> Result<(), String> {
    if let Some(job) = state
        .file_indexes
        .lock()
        .map_err(|error| error.to_string())?
        .remove(&project_id)
    {
        job.cancel.store(true, Ordering::Relaxed);
        if let Ok(mut search) = job.search_cancel.lock() {
            if let Some(cancel) = search.take() {
                cancel.store(true, Ordering::Relaxed);
            }
        }
    }
    Ok(())
}

#[tauri::command]
async fn reveal_project_path(
    state: State<'_, Arc<AppState>>,
    project_id: String,
    path: String,
) -> Result<(), String> {
    let root = project_root_for_files(state.inner().clone(), project_id).await?;
    let validated_root = root.clone();
    let reveal_target = tauri::async_runtime::spawn_blocking(move || {
        project_files::validate_existing_path(&validated_root, &path)
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(err_to_string)?;
    reveal_path(&reveal_target)
}

#[tauri::command]
fn reveal_project_file(
    state: State<Arc<AppState>>,
    project_id: String,
    path: String,
) -> Result<(), String> {
    let file = state
        .core
        .lock()
        .map_err(|err| err.to_string())?
        .resolve_project_file(&project_id, &path)
        .map_err(err_to_string)?;
    reveal_path(&file)
}

#[tauri::command]
fn open_project_file(
    state: State<Arc<AppState>>,
    project_id: String,
    path: String,
) -> Result<(), String> {
    let file = state
        .core
        .lock()
        .map_err(|err| err.to_string())?
        .resolve_project_file(&project_id, &path)
        .map_err(err_to_string)?;
    launch::open_in_file_manager(&file.to_string_lossy()).map_err(err_to_string)
}

fn reveal_path(path: &std::path::Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .args(["/select,", &path.to_string_lossy()])
            .spawn()
            .map_err(|err| err.to_string())?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .args(["-R", &path.to_string_lossy()])
            .spawn()
            .map_err(|err| err.to_string())?;
        Ok(())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = path;
        Err("unsupported platform".into())
    }
}

fn err_to_string(err: repoatlas_core::Error) -> String {
    err.to_string()
}

async fn run_blocking<T, F>(state: Arc<AppState>, fun: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&Core) -> repoatlas_core::Result<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let core = state.core.lock().map_err(|err| err.to_string())?;
        fun(&core).map_err(err_to_string)
    })
    .await
    .map_err(|err| err.to_string())?
}

fn walk_scan_root(
    app: &AppHandle,
    state: &AppState,
    root_id: &str,
    root_path: &str,
    cancel: &AtomicBool,
) -> Result<ScanResult, String> {
    let scan_id = uuid::Uuid::new_v4().to_string();
    let emit = |progress: ScanProgress| {
        let _ = app.emit("scan://progress", progress);
    };
    emit(ScanProgress {
        scan_id: scan_id.clone(),
        root_path: root_path.to_string(),
        phase: "started".into(),
        visited: 0,
        discovered: 0,
        current_path: Some(root_path.to_string()),
        message: None,
    });
    let engine = ScanEngine {
        scan_id: scan_id.clone(),
        root: PathBuf::from(root_path),
        cancel,
        on_progress: &emit,
    };
    let (discovered, visited, errors, walk_cancelled) = engine.walk().map_err(err_to_string)?;
    let mut found_ids = HashSet::new();
    let mut cancelled = walk_cancelled;
    for item in &discovered {
        if cancel.load(Ordering::Relaxed) {
            cancelled = true;
            break;
        }
        let core = state.core.lock().map_err(|err| err.to_string())?;
        found_ids.insert(
            core.ingest_scan_project(root_id, item)
                .map_err(err_to_string)?,
        );
    }
    let unavailable = state
        .core
        .lock()
        .map_err(|err| err.to_string())?
        .finalize_scan_ingest(root_id, &found_ids, cancelled)
        .map_err(err_to_string)?;
    emit(ScanProgress {
        scan_id: scan_id.clone(),
        root_path: root_path.to_string(),
        phase: if cancelled {
            "cancelled".into()
        } else {
            "completed".into()
        },
        visited,
        discovered: discovered.len() as u64,
        current_path: None,
        message: None,
    });
    Ok(ScanResult {
        scan_id,
        visited,
        discovered: discovered.len() as u64,
        unavailable,
        cancelled,
        errors,
    })
}

fn db_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    Ok(dir.join("repoatlas.sqlite"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let path = db_path(app.handle())?;
            let core = Core::open(&path).map_err(|err| err.to_string())?;
            let log_dir = path
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .join("task-logs");
            let broker = Broker::new(log_dir).map_err(|err| err.to_string())?;
            app.manage(Arc::new(AppState {
                core: Mutex::new(core),
                broker,
                cancel: Arc::new(AtomicBool::new(false)),
                runtime_configs: Mutex::new(HashMap::new()),
                runtime_sampler_started: AtomicBool::new(false),
                port_preflights: Mutex::new(HashMap::new()),
                file_indexes: Mutex::new(HashMap::new()),
                file_index_build_lock: Arc::new(Mutex::new(())),
                file_search_lock: Arc::new(Mutex::new(())),
                next_file_index_generation: AtomicU64::new(0),
            }));
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(2500));
                if let Some(window) = handle.get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        return;
                    }
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            get_settings,
            update_settings,
            list_scan_roots,
            add_scan_root,
            remove_scan_root,
            list_projects,
            list_collections,
            create_collection,
            update_collection,
            delete_collection,
            collection_member_ids,
            set_collection_members,
            save_collection,
            search_projects,
            get_project,
            get_project_brief,
            read_project_readme,
            read_project_document,
            inspect_project_environment,
            read_project_icons,
            set_project_icon,
            clear_project_icon,
            read_project_file,
            list_project_directory,
            read_project_file_preview,
            read_project_image,
            ensure_project_path_index,
            search_project_paths,
            cancel_project_path_search,
            cancel_project_path_index,
            reveal_project_path,
            reveal_project_file,
            open_project_file,
            mcp_setup_info,
            register_project,
            update_project,
            remove_project,
            remove_folder_records,
            refresh_project,
            mark_project_opened,
            relocate_project,
            atlas_report,
            export_atlas_report_to,
            list_pending_approvals,
            list_attention_items,
            get_attention_center,
            get_dashboard_snapshot,
            acknowledge_attention_item,
            resolve_pending_approval,
            export_json,
            export_json_to,
            import_json,
            import_json_from,
            backup_db,
            start_scan,
            cancel_scan,
            git_status,
            git_diff,
            git_execute,
            list_task_runs,
            list_active_task_runs,
            get_task_run,
            read_task_log,
            get_task_runtime_snapshot,
            get_task_runtime_snapshots,
            preflight_task_ports,
            open_dev_endpoint,
            start_task,
            write_task_stdin,
            stop_task,
            resize_task_run,
            show_main_window,
            open_in_explorer,
            list_external_tools,
            open_in_terminal,
            open_in_ide,
            open_in_agent
        ])
        .run(tauri::generate_context!())
        .expect("error while running RepoAtlas");
}
