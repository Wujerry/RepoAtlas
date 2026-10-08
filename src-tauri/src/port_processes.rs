use crate::{err_to_string, AppState};
use repoatlas_core::{
    port_processes::{ProcessIdentity, ProcessInventory, StopMode, StopPreview},
    ProjectQuery, ProjectSummary,
};
use std::sync::Arc;
use tauri::State;

fn projects(state: &AppState) -> Result<Vec<ProjectSummary>, String> {
    state
        .core
        .lock()
        .map_err(|e| e.to_string())?
        .list_projects(ProjectQuery {
            include_archived: Some(true),
            ..Default::default()
        })
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn list_port_processes(
    state: State<'_, Arc<AppState>>,
) -> Result<ProcessInventory, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let projects = projects(&state)?;
        state
            .port_processes
            .lock()
            .map_err(|e| e.to_string())?
            .inventory(&state.broker, &projects)
            .map_err(err_to_string)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn associate_port_process(
    state: State<'_, Arc<AppState>>,
    identity: ProcessIdentity,
    project_id: Option<String>,
) -> Result<(), String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let projects = projects(&state)?;
        state
            .port_processes
            .lock()
            .map_err(|e| e.to_string())?
            .associate(identity.clone(), project_id, &projects)
            .map_err(err_to_string)?;
        state
            .core
            .lock()
            .map_err(|e| e.to_string())?
            .record_audit_event(
                "desktop",
                "associate_port_process",
                "process",
                Some(&identity.pid.to_string()),
                None,
                "success",
            )
            .map_err(err_to_string)?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn preview_process_stop(
    state: State<'_, Arc<AppState>>,
    identity: ProcessIdentity,
    mode: StopMode,
) -> Result<StopPreview, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let projects = projects(&state)?;
        state
            .port_processes
            .lock()
            .map_err(|e| e.to_string())?
            .preview(&state.broker, &projects, &identity, mode)
            .map_err(err_to_string)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn confirm_process_stop(
    state: State<'_, Arc<AppState>>,
    token: String,
) -> Result<String, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let projects = projects(&state)?;
        let preview = state
            .port_processes
            .lock()
            .map_err(|e| e.to_string())?
            .consume(&state.broker, &projects, &token)
            .map_err(err_to_string)?;
        let target = preview.target;
        // Write intent before touching a process. Keep identity paths and command lines out of audit.
        state
            .core
            .lock()
            .map_err(|e| e.to_string())?
            .record_audit_event(
                "desktop",
                "stop_port_process",
                "process",
                Some(&target.pid.to_string()),
                Some(if preview.mode == StopMode::Force {
                    "force"
                } else {
                    "graceful"
                }),
                "requested",
            )
            .map_err(err_to_string)?;
        let result: Result<String, String> = (|| {
            if let Some(run) = target.run_id {
                if preview.mode == StopMode::Graceful {
                    state
                        .broker
                        .request_normal_stop(&run)
                        .map_err(err_to_string)?;
                    // A cooperative signal is a request, never proof of process exit.
                    Ok("signal_sent".into())
                } else {
                    crate::stop_task_inner(&state, &run)?;
                    Ok("stopped".into())
                }
            } else {
                if preview.mode != StopMode::Force {
                    return Err("graceful_unavailable".into());
                }
                repoatlas_core::port_processes::force_external(
                    target.identity.as_ref().ok_or("identity_unavailable")?,
                )
                .map_err(err_to_string)?;
                Ok("stopped".into())
            }
        })();
        state
            .core
            .lock()
            .map_err(|e| e.to_string())?
            .record_audit_event(
                "desktop",
                "stop_port_process",
                "process",
                Some(&target.pid.to_string()),
                None,
                if result.is_ok() { "success" } else { "failed" },
            )
            .map_err(err_to_string)?;
        result
    })
    .await
    .map_err(|e| e.to_string())?
}
