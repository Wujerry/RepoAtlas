use super::*;
use repoatlas_core::agent_sessions::{
    adapters::{self, LocalAdapter, SessionAdapter},
    *,
};
use std::sync::LazyLock;

static JOB: LazyLock<Mutex<SessionRefreshJob>> =
    LazyLock::new(|| Mutex::new(SessionRefreshJob::default()));
static CANCEL: AtomicBool = AtomicBool::new(false);
static FORCE_AGAIN: AtomicBool = AtomicBool::new(false);

async fn read<T: Send + 'static>(
    state: &State<'_, Arc<AppState>>,
    f: impl FnOnce(&Core) -> repoatlas_core::Result<T> + Send + 'static,
) -> Result<T, String> {
    let core = state.core.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let guard = core.lock().map_err(|_| "core_lock_failed")?;
        f(&guard).map_err(err_to_string)
    })
    .await
    .map_err(err_to_string)?
}
#[tauri::command]
pub async fn session_sources(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<SessionSource>, String> {
    read(&state, |c| c.session_source_candidates()).await
}
#[tauri::command]
pub async fn set_session_source(
    state: State<'_, Arc<AppState>>,
    adapter: String,
    path: String,
    enabled: bool,
) -> Result<SessionSource, String> {
    if !enabled {
        CANCEL.store(true, Ordering::Relaxed);
    }
    read(&state, move |c| {
        c.set_session_source(&adapter, &path, enabled)
    })
    .await
}
#[tauri::command]
pub async fn search_agent_sessions(
    state: State<'_, Arc<AppState>>,
    query: SessionQuery,
) -> Result<SessionSearchResult, String> {
    read(&state, move |c| c.search_agent_sessions(query)).await
}
#[tauri::command]
pub async fn get_agent_session(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<AgentSession, String> {
    read(&state, move |c| c.get_agent_session(&id)).await
}
#[tauri::command]
pub async fn continue_agent_sessions(
    state: State<'_, Arc<AppState>>,
    project_id: Option<String>,
) -> Result<Vec<AgentSession>, String> {
    read(&state, move |c| {
        c.continue_agent_sessions(project_id.as_deref())
    })
    .await
}
#[tauri::command]
pub async fn agent_session_messages(
    state: State<'_, Arc<AppState>>,
    id: String,
    offset: usize,
    limit: usize,
) -> Result<SessionMessagePage, String> {
    read(&state, move |c| {
        c.agent_session_messages(&id, offset, limit)
    })
    .await
}
#[tauri::command]
pub async fn link_agent_session(
    state: State<'_, Arc<AppState>>,
    id: String,
    project_id: Option<String>,
    cwd: Option<String>,
) -> Result<(), String> {
    read(&state, move |c| {
        c.link_agent_session(&id, project_id.as_deref(), cwd.as_deref())
    })
    .await
}
#[tauri::command]
pub async fn agent_session_resume_spec(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<SessionResumeSpec, String> {
    read(&state, move |c| c.agent_session_resume_spec(&id)).await
}
#[tauri::command]
pub async fn resume_agent_session(
    state: State<'_, Arc<AppState>>,
    id: String,
    target: Option<String>,
) -> Result<(), String> {
    read(&state, move |c| {
        c.resume_agent_session_target(&id, target.as_deref().unwrap_or("cli"))
    })
    .await
}
#[tauri::command]
pub fn session_refresh_status() -> SessionRefreshJob {
    JOB.lock().unwrap_or_else(|e| e.into_inner()).clone()
}
#[tauri::command]
pub fn cancel_session_refresh() {
    CANCEL.store(true, Ordering::Relaxed);
}
#[tauri::command]
pub async fn rebuild_session_index(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    {
        let mut status = JOB.lock().map_err(err_to_string)?;
        if status.running {
            return Err("session_refresh_running".into());
        }
        status.running = true;
    }
    let result = read(&state, |c| c.rebuild_session_index()).await;
    JOB.lock().map_err(err_to_string)?.running = false;
    result
}
#[tauri::command]
pub fn refresh_agent_sessions(
    state: State<'_, Arc<AppState>>,
    force: bool,
) -> Result<SessionRefreshJob, String> {
    let mut status = JOB.lock().map_err(err_to_string)?;
    if status.running {
        if force {
            FORCE_AGAIN.store(true, Ordering::Relaxed);
        }
        return Ok(status.clone());
    }
    CANCEL.store(false, Ordering::Relaxed);
    *status = SessionRefreshJob {
        running: true,
        ..Default::default()
    };
    let initial = status.clone();
    drop(status);
    let core = state.core.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut force = force;
        loop {
            let run =
                || -> Result<(), String> {
                    let worker = core
                        .lock()
                        .map_err(err_to_string)?
                        .session_worker()
                        .map_err(err_to_string)?;
                    let sources = worker.session_sources().map_err(err_to_string)?;
                    for source in sources.into_iter().filter(|s| s.enabled) {
                        if CANCEL.load(Ordering::Relaxed) {
                            break;
                        }
                        if !force && source.is_fresh() {
                            continue;
                        }
                        JOB.lock().map_err(err_to_string)?.source_id = Some(source.id.clone());
                        let mut seen = HashSet::new();
                        let mut error = None;
                        match adapters::enumerate(&source, &CANCEL) {
                            Err(e) => {
                                error = Some(e.to_string());
                                JOB.lock().map_err(err_to_string)?.errors += 1;
                            }
                            Ok(files) => {
                                for path in files {
                                    if CANCEL.load(Ordering::Relaxed) {
                                        break;
                                    }
                                    let locator = repoatlas_core::paths::path_to_string(&path);
                                    seen.insert(locator.clone());
                                    let result =
                                        || -> repoatlas_core::Result<()> {
                                            let fingerprint = adapters::fingerprint(&path)?;
                                            let changed = worker.session_file_changed(
                                                &source.id,
                                                &locator,
                                                &fingerprint,
                                            )?;
                                            if !changed {
                                                return Ok(());
                                            }
                                            let sessions = LocalAdapter(source.adapter.clone())
                                                .read(Path::new(&source.path), &path, &CANCEL)?;
                                            if CANCEL.load(Ordering::Relaxed) {
                                                return Ok(());
                                            }
                                            let mut ids = HashSet::new();
                                            for (session, messages) in sessions {
                                                if CANCEL.load(Ordering::Relaxed) {
                                                    break;
                                                }
                                                ids.insert(session.external_id.clone());
                                                worker.ingest_session_cancelable(
                                                    &source,
                                                    session,
                                                    &messages,
                                                    &fingerprint,
                                                    &CANCEL,
                                                )?;
                                            }
                                            if !CANCEL.load(Ordering::Relaxed) {
                                                worker.finish_session_file(
                                                    &source.id,
                                                    &locator,
                                                    &fingerprint,
                                                    &ids,
                                                )?;
                                            }
                                            Ok(())
                                        }();
                                    if let Err(e) = result {
                                        error = Some(e.to_string());
                                        JOB.lock().map_err(err_to_string)?.errors += 1;
                                    }
                                    JOB.lock().map_err(err_to_string)?.processed += 1;
                                }
                            }
                        }
                        if CANCEL.load(Ordering::Relaxed) {
                            error = Some("canceled".into());
                        }
                        worker
                            .finish_session_source(&source.id, &seen, error.as_deref())
                            .map_err(err_to_string)?;
                    }
                    Ok(())
                };
            let result = run();
            let mut status = JOB.lock().unwrap_or_else(|e| e.into_inner());
            if result.is_err() {
                status.errors += 1;
            }
            if FORCE_AGAIN.swap(false, Ordering::Relaxed) && !CANCEL.load(Ordering::Relaxed) {
                force = true;
                drop(status);
                continue;
            }
            status.running = false;
            status.canceled = CANCEL.load(Ordering::Relaxed);
            status.source_id = None;
            break;
        }
    });
    Ok(initial)
}
