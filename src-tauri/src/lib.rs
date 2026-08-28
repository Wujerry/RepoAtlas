use repoatlas_core::{
    scan::ScanEngine, AppSettings, AtlasReport, Broker, Core, GitDiff, GitOp, GitStatus, LogChunk,
    PendingApproval, ProjectPatch, ProjectQuery, ProjectRemoval, ProjectSummary, ScanProgress,
    ScanResult, ScanRoot, ScanRootRemoval, SearchHit, TaskRun, TaskSpec,
};
use serde::Serialize;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

pub struct AppState {
    pub core: Mutex<Core>,
    pub broker: Broker,
    pub cancel: Arc<AtomicBool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    pub settings: AppSettings,
    pub scan_roots: Vec<ScanRoot>,
    pub projects: Vec<ProjectSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpSetupInfo {
    pub db_path: String,
    pub platform: String,
    pub binary_name: String,
    pub binary_path: Option<String>,
    pub workspace_path: Option<String>,
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

fn resolve_api_key(profile: &repoatlas_core::ProviderProfile, explicit: &str) -> String {
    if !explicit.is_empty() {
        return explicit.to_string();
    }
    if profile.credential_ref.is_empty() {
        return String::new();
    }
    std::env::var(&profile.credential_ref).unwrap_or_default()
}

#[tauri::command]
fn list_provider_profiles(
    state: State<Arc<AppState>>,
) -> Result<Vec<repoatlas_core::ProviderProfile>, String> {
    state
        .core
        .lock()
        .map_err(|e| e.to_string())?
        .list_provider_profiles()
        .map_err(err_to_string)
}

#[tauri::command]
fn upsert_provider_profile(
    state: State<Arc<AppState>>,
    upsert: repoatlas_core::ProviderUpsert,
) -> Result<repoatlas_core::ProviderProfile, String> {
    state
        .core
        .lock()
        .map_err(|e| e.to_string())?
        .upsert_provider_profile(upsert)
        .map_err(err_to_string)
}

#[tauri::command]
fn delete_provider_profile(state: State<Arc<AppState>>, id: String) -> Result<(), String> {
    state
        .core
        .lock()
        .map_err(|e| e.to_string())?
        .delete_provider_profile(&id)
        .map_err(err_to_string)
}

#[tauri::command]
fn provider_presets(
    state: State<Arc<AppState>>,
) -> Result<Vec<repoatlas_core::ProviderPreset>, String> {
    Ok(state
        .core
        .lock()
        .map_err(|e| e.to_string())?
        .provider_presets())
}

#[tauri::command]
fn list_memory(
    state: State<Arc<AppState>>,
    project_id: String,
) -> Result<Vec<repoatlas_core::AiMemoryItem>, String> {
    state
        .core
        .lock()
        .map_err(|e| e.to_string())?
        .list_memory(&project_id)
        .map_err(err_to_string)
}

#[tauri::command]
fn add_memory(
    state: State<Arc<AppState>>,
    project_id: String,
    text: String,
) -> Result<repoatlas_core::AiMemoryItem, String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    core.add_memory_with_origin(&project_id, &text, "desktop")
        .map_err(err_to_string)
}

#[tauri::command]
fn delete_memory(state: State<Arc<AppState>>, id: String) -> Result<(), String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    core.delete_memory_with_origin(&id, "desktop")
        .map_err(err_to_string)
}

#[tauri::command]
fn latest_summary(
    state: State<Arc<AppState>>,
    project_id: String,
) -> Result<Option<repoatlas_core::AiSummary>, String> {
    state
        .core
        .lock()
        .map_err(|e| e.to_string())?
        .latest_summary(&project_id)
        .map_err(err_to_string)
}

#[tauri::command]
fn conversation_summary(
    state: State<Arc<AppState>>,
    project_id: String,
) -> Result<repoatlas_core::ConversationSummary, String> {
    state
        .core
        .lock()
        .map_err(|e| e.to_string())?
        .conversation_summary(&project_id)
        .map_err(err_to_string)
}

#[tauri::command]
fn list_conversation(
    state: State<Arc<AppState>>,
    project_id: String,
) -> Result<Vec<repoatlas_core::ChatMessage>, String> {
    state
        .core
        .lock()
        .map_err(|e| e.to_string())?
        .list_conversation(&project_id)
        .map_err(err_to_string)
}

#[tauri::command]
fn clear_conversation(state: State<Arc<AppState>>, project_id: String) -> Result<(), String> {
    let core = state.core.lock().map_err(|e| e.to_string())?;
    core.clear_conversation_with_origin(&project_id, "desktop")
        .map_err(err_to_string)
}

#[tauri::command]
fn analysis_plan(
    state: State<Arc<AppState>>,
    project_id: String,
    provider_id: String,
) -> Result<repoatlas_core::AnalysisPlan, String> {
    state
        .core
        .lock()
        .map_err(|e| e.to_string())?
        .analysis_plan(&project_id, &provider_id)
        .map_err(err_to_string)
}

#[tauri::command]
async fn summarize_project(
    state: State<'_, Arc<AppState>>,
    project_id: String,
    provider_id: String,
    api_key: String,
) -> Result<repoatlas_core::AiSummary, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        // Read the small, immutable inputs under the Core mutex, then release
        // it before making the potentially slow external HTTP request.
        let (profile, evidence) = {
            let core = state.core.lock().map_err(|err| err.to_string())?;
            let profile = core
                .get_provider_profile(&provider_id)
                .map_err(err_to_string)?;
            let detail = core.get_project(&project_id).map_err(err_to_string)?;
            let evidence = repoatlas_core::ai::evidence_for(
                &PathBuf::from(detail.project.canonical_path),
                8_000,
            )
            .map_err(err_to_string)?;
            (profile, evidence)
        };
        let key = resolve_api_key(&profile, &api_key);
        let caller = repoatlas_core::ai::AiCaller {
            profile: profile.clone(),
            api_key: key,
        };
        let text = caller
            .chat(
                "You maintain concise developer notes. Given the evidence below, produce a short project summary covering: what the project is, main languages/stack, how to run, how to build/package, and where key entry points are. Answer in the language of the evidence, plain text, no markdown headers.",
                vec![repoatlas_core::ChatMessage {
                    role: "user".into(),
                    content: evidence.text.clone(),
                }],
                900,
            )
            .map_err(err_to_string)?;
        let core = state.core.lock().map_err(|err| err.to_string())?;
        core.save_summary_with_origin(
            &project_id,
            Some(&provider_id),
            Some(&caller.profile.model),
            &text,
            &evidence.text,
            "desktop",
            Some(&caller.profile.name),
        )
        .map_err(err_to_string)
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
async fn ask_project(
    state: State<'_, Arc<AppState>>,
    project_id: String,
    provider_id: String,
    api_key: String,
    question: String,
) -> Result<String, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let question = question.trim().to_string();
        if question.is_empty() {
            return Err("question is empty".into());
        }
        let (profile, evidence, mut history) = {
            let core = state.core.lock().map_err(|err| err.to_string())?;
            let profile = core
                .get_provider_profile(&provider_id)
                .map_err(err_to_string)?;
            let detail = core.get_project(&project_id).map_err(err_to_string)?;
            let evidence = repoatlas_core::ai::evidence_for(
                &PathBuf::from(detail.project.canonical_path),
                8_000,
            )
            .map_err(err_to_string)?;
            let history = repoatlas_core::ai::conversation(core.connection(), &project_id)
                .map_err(err_to_string)?;
            (profile, evidence, history)
        };
        let key = resolve_api_key(&profile, &api_key);
        let caller = repoatlas_core::ai::AiCaller {
            profile: profile.clone(),
            api_key: key,
        };
        history.push(repoatlas_core::ChatMessage {
            role: "user".into(),
            content: question.clone(),
        });
        let system = format!(
            "You are RepoAtlas, a local code-asset assistant. Answer using the provided project evidence. Evidence:\n\n{}\n\nDo not invent files that are not shown.",
            evidence.text
        );
        let answer = caller
            .chat(&system, history, 1_500)
            .map_err(err_to_string)?;
        let core = state.core.lock().map_err(|err| err.to_string())?;
        core.append_conversation_with_origin(
            &project_id,
            vec![
                repoatlas_core::ChatMessage {
                    role: "user".into(),
                    content: question.clone(),
                },
                repoatlas_core::ChatMessage {
                    role: "assistant".into(),
                    content: answer.clone(),
                },
            ],
            "desktop",
            Some(&question),
        )
        .map_err(err_to_string)?;
        Ok(answer)
    })
    .await
    .map_err(|err| err.to_string())?
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
    Ok(McpSetupInfo {
        db_path: db_path(&app)?.to_string_lossy().into_owned(),
        platform: std::env::consts::OS.to_string(),
        binary_name,
        binary_path: binary_path.map(|path| path.to_string_lossy().into_owned()),
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
fn list_summaries(
    state: State<Arc<AppState>>,
    project_id: String,
) -> Result<Vec<repoatlas_core::AiSummary>, String> {
    state
        .core
        .lock()
        .map_err(|err| err.to_string())?
        .list_summaries(&project_id)
        .map_err(err_to_string)
}

#[tauri::command]
fn get_summary(
    state: State<Arc<AppState>>,
    summary_id: String,
) -> Result<repoatlas_core::AiSummary, String> {
    state
        .core
        .lock()
        .map_err(|err| err.to_string())?
        .get_summary(&summary_id)
        .map_err(err_to_string)
}

#[tauri::command]
fn accept_summary_memory(
    state: State<Arc<AppState>>,
    summary_id: String,
) -> Result<repoatlas_core::AiMemoryItem, String> {
    let core = state.core.lock().map_err(|err| err.to_string())?;
    core.accept_summary_as_memory_with_origin(&summary_id, "desktop")
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
    run_blocking(state.inner().clone(), move |core| {
        core.git_status(&project_id)
    })
    .await
}

#[tauri::command]
async fn git_diff(
    state: State<'_, Arc<AppState>>,
    project_id: String,
    path: Option<String>,
    staged: bool,
) -> Result<GitDiff, String> {
    run_blocking(state.inner().clone(), move |core| {
        core.git_diff(&project_id, path.as_deref(), staged)
    })
    .await
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
) -> Result<TaskRun, String> {
    let spec = {
        let core = state.core.lock().map_err(|err| err.to_string())?;
        let detail = core.get_project(&project_id).map_err(err_to_string)?;
        let task = detail
            .tasks
            .iter()
            .find(|task| task.id == task_id)
            .ok_or_else(|| format!("task not found: {task_id}"))?;
        let project_path = PathBuf::from(&detail.project.canonical_path);
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
        TaskSpec {
            project_id,
            task_id: Some(task.id.clone()),
            kind: task.kind.clone(),
            executable: task.executable.clone(),
            argv: task.argv.clone(),
            cwd: Some(cwd.to_string_lossy().into_owned()),
            shell_mode: task.shell_mode,
        }
    };
    start_task_spec(&app, state.inner().as_ref(), spec)
}

fn start_task_spec(app: &AppHandle, state: &AppState, spec: TaskSpec) -> Result<TaskRun, String> {
    let core = state.core.lock().map_err(|err| err.to_string())?;
    let app_handle = app.clone();
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
                        if let Some(state) = app_handle.try_state::<Arc<AppState>>() {
                            let core = match state.core.lock() {
                                Ok(core) => core,
                                Err(_) => return,
                            };
                            let status = if code == Some(0) {
                                "succeeded"
                            } else {
                                "failed"
                            };
                            match core.finish_task_run(&run_id, status, code) {
                                Ok(run) => {
                                    let _ = core.record_project_event(
                                        &run.project_id,
                                        "task",
                                        "Finished task",
                                        Some(status),
                                    );
                                    let _ = core.record_audit_event(
                                        "desktop",
                                        "finish_task_run",
                                        "task_run",
                                        Some(&run.id),
                                        Some(status),
                                        status,
                                    );
                                    let _ = app_handle.emit("task://exited", run);
                                }
                                Err(error) => {
                                    let _ = core.record_audit_event(
                                        "desktop",
                                        "finish_task_run",
                                        "task_run",
                                        Some(&run_id),
                                        Some(&error.to_string()),
                                        "failed",
                                    );
                                }
                            }
                        }
                    });
                }
            },
        )
        .map_err(err_to_string)?;
    let _ = core.record_project_event(&run.project_id, "task", "Started task", Some(&run.kind));
    if let Err(error) = core.record_audit_event(
        "desktop",
        "start_task",
        "task",
        run.task_id.as_deref().or(Some(&run.project_id)),
        Some(&run.kind),
        "started",
    ) {
        // Do not leave a live process behind when the required audit write
        // fails. Stop/reap the child and close its run record before returning
        // the persistence error to the UI.
        let _ = state.broker.stop(core.connection(), &run.id);
        let _ = core.finish_task_run(&run.id, "failed", None);
        return Err(err_to_string(error));
    }
    Ok(run)
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
    open_path(&path)
}

#[tauri::command]
fn open_in_terminal(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let system_root = std::env::var_os("SystemRoot")
            .or_else(|| std::env::var_os("WINDIR"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        Command::new(system_root.join("System32").join("cmd.exe"))
            .args(["/D", "/K"])
            .current_dir(&path)
            .spawn()
            .map_err(|err| err.to_string())?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .args(["-a", "Terminal", &path])
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

#[tauri::command]
fn open_in_ide(app: AppHandle, path: String, ide: String) -> Result<(), String> {
    let command = match ide.as_str() {
        "vscode" => Some(("code", vec![path.clone()])),
        "cursor" => Some(("cursor", vec![path.clone()])),
        "zed" => Some(("zed", vec![path.clone()])),
        "visualstudio" => Some(("devenv", vec![path.clone()])),
        "jetbrains" => Some(("idea", vec![path.clone()])),
        "xcode" => Some(("open", vec!["-a".into(), "Xcode".into(), path.clone()])),
        _ => None,
    };
    let opened = if let Some((program, args)) = command {
        Command::new(program).args(args).spawn().is_ok()
    } else {
        false
    };
    if !opened {
        open_path(&path)?;
    }
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
fn resolve_pending_approval(
    app: AppHandle,
    state: State<Arc<AppState>>,
    approval_id: String,
    approved: bool,
) -> Result<PendingApproval, String> {
    let (approval, spec) = {
        let core = state.core.lock().map_err(|err| err.to_string())?;
        core.resolve_task_approval(&approval_id, approved)
            .map_err(err_to_string)?
    };
    if let Some(spec) = spec {
        let run = match start_task_spec(&app, state.inner().as_ref(), spec) {
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
        Ok(repoatlas_core::environment::inspect_environment(&detail))
    })
    .await
    .map_err(|err| err.to_string())?
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
    open_path(&file.to_string_lossy())
}

fn open_path(path: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(path)
            .spawn()
            .map_err(|err| err.to_string())?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(path)
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
    let (discovered, visited, errors, cancelled) = engine.walk().map_err(err_to_string)?;
    let unavailable = {
        let core = state.core.lock().map_err(|err| err.to_string())?;
        core.ingest_scan(root_id, &discovered, cancelled)
            .map_err(err_to_string)?
    };
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
            }));
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
            search_projects,
            get_project,
            read_project_readme,
            read_project_document,
            inspect_project_environment,
            read_project_icons,
            set_project_icon,
            clear_project_icon,
            read_project_file,
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
            list_summaries,
            get_summary,
            accept_summary_memory,
            list_pending_approvals,
            resolve_pending_approval,
            export_json,
            export_json_to,
            import_json,
            import_json_from,
            backup_db,
            list_provider_profiles,
            upsert_provider_profile,
            delete_provider_profile,
            provider_presets,
            list_memory,
            add_memory,
            delete_memory,
            latest_summary,
            conversation_summary,
            list_conversation,
            clear_conversation,
            analysis_plan,
            summarize_project,
            ask_project,
            start_scan,
            cancel_scan,
            git_status,
            git_diff,
            git_execute,
            list_task_runs,
            read_task_log,
            start_task,
            write_task_stdin,
            stop_task,
            open_in_explorer,
            open_in_terminal,
            open_in_ide
        ])
        .run(tauri::generate_context!())
        .expect("error while running RepoAtlas");
}
