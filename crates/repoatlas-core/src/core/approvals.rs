use super::{from_json, now, to_json, Core};
use crate::error::{Error, Result};
use crate::models::{PendingApproval, TaskDefinition, TaskSpec};
use rusqlite::{params, OptionalExtension};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const APPROVAL_TTL_MINUTES: i64 = 15;
const EXPIRED_ERROR: &str = "approval expired; request a new approval";

fn approval_expired(approval: &PendingApproval) -> bool {
    chrono::DateTime::parse_from_rfc3339(&approval.created_at)
        .map(|created| {
            chrono::Utc::now() >= created + chrono::Duration::minutes(APPROVAL_TTL_MINUTES)
        })
        .unwrap_or(true)
}

impl Core {
    pub fn request_task_approval(&self, spec: TaskSpec) -> Result<PendingApproval> {
        self.request_task_approval_from("desktop", spec)
    }

    pub fn request_task_approval_from(
        &self,
        origin: &str,
        mut spec: TaskSpec,
    ) -> Result<PendingApproval> {
        let detail = self.get_project(&spec.project_id)?;
        let task = spec
            .task_id
            .as_ref()
            .and_then(|id| detail.tasks.iter().find(|task| &task.id == id));
        if spec.task_id.is_some() && task.is_none() {
            return Err(Error::NotFound(spec.task_id.clone().unwrap_or_default()));
        }
        // A task id is a capability reference, not permission to override the
        // saved executable or arguments. Always resolve it from RepoAtlas data.
        if let Some(task) = task {
            spec.kind = task.kind.clone();
            spec.executable = task.executable.clone();
            spec.argv = task.argv.clone();
            spec.cwd = task.cwd.clone();
            spec.shell_mode = task.shell_mode;
        }
        let title = task
            .map(|task| task.name.clone())
            .unwrap_or_else(|| spec.kind.clone());
        let argv = if spec.argv.is_empty() {
            task.map(|task| task.argv.clone()).unwrap_or_default()
        } else {
            spec.argv.clone()
        };
        let executable = if spec.executable.trim().is_empty() {
            task.map(|task| task.executable.clone())
        } else {
            Some(spec.executable.clone())
        };
        let configured_cwd = spec
            .cwd
            .clone()
            .or_else(|| task.and_then(|task| task.cwd.clone()))
            .or(Some(detail.project.canonical_path.clone()));
        let project_path = Path::new(&detail.project.canonical_path);
        let cwd = configured_cwd.map(|value| {
            let configured = PathBuf::from(&value);
            if configured.is_absolute() {
                value
            } else {
                project_path.join(configured).to_string_lossy().into_owned()
            }
        });
        let detail_text = format!(
            "{} {}\ncwd {}",
            executable.clone().unwrap_or_default(),
            argv.join(" "),
            cwd.clone().unwrap_or_default()
        );
        let approval = PendingApproval {
            id: Uuid::new_v4().to_string(),
            project_id: spec.project_id,
            kind: spec.kind,
            title,
            detail: detail_text,
            executable,
            argv,
            cwd,
            task_id: spec.task_id,
            shell_mode: spec.shell_mode,
            expected_ports: task
                .map(|task| task.expected_ports.clone())
                .unwrap_or_default(),
            dev_url_path: task.and_then(|task| task.dev_url_path.clone()),
            dev_url_scheme: task.and_then(|task| task.dev_url_scheme.clone()),
            status: "pending".into(),
            origin: if origin.trim().is_empty() {
                "desktop".into()
            } else {
                origin.trim().into()
            },
            run_id: None,
            error: None,
            created_at: now(),
            resolved_at: None,
        };
        let argv_json = to_json(&approval.argv)?;
        let expected_ports_json = to_json(&approval.expected_ports)?;
        self.with_immediate_transaction(|core| {
            core.conn.execute(
                "INSERT INTO pending_approvals (id, project_id, kind, title, detail, executable, argv_json, cwd, task_id, shell_mode, status, origin, run_id, error, created_at, resolved_at, expected_ports_json, dev_url_path, dev_url_scheme) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
                params![
                    approval.id,
                    approval.project_id,
                    approval.kind,
                    approval.title,
                    approval.detail,
                    approval.executable,
                    argv_json,
                    approval.cwd,
                    approval.task_id,
                    approval.shell_mode,
                    approval.status,
                    approval.origin,
                    approval.run_id,
                    approval.error,
                    approval.created_at,
                    approval.resolved_at
                    ,expected_ports_json
                    ,approval.dev_url_path
                    ,approval.dev_url_scheme
                ],
            )?;
            core.record_audit_event(
                &approval.origin,
                "request_task_approval",
                "task",
                approval.task_id.as_deref().or(Some(&approval.project_id)),
                Some(&approval.title),
                "pending",
            )?;
            Ok(())
        })?;
        Ok(approval)
    }

    pub fn list_pending_approvals(&self) -> Result<Vec<PendingApproval>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project_id, kind, title, detail, executable, argv_json, cwd, task_id, shell_mode, status, origin, run_id, error, created_at, resolved_at, expected_ports_json, dev_url_path, dev_url_scheme FROM pending_approvals WHERE status = 'pending' ORDER BY datetime(created_at) DESC",
        )?;
        let rows = stmt.query_map([], approval_from_row)?;
        Ok(rows
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .filter(|approval| !approval_expired(approval))
            .collect())
    }

    pub fn get_pending_approval(&self, id: &str) -> Result<PendingApproval> {
        let mut approval = self.load_pending_approval(id)?;
        if approval.status == "pending" && approval_expired(&approval) {
            approval.status = "expired".into();
            approval.error = Some(EXPIRED_ERROR.into());
        }
        Ok(approval)
    }

    fn load_pending_approval(&self, id: &str) -> Result<PendingApproval> {
        self.conn
            .query_row(
                "SELECT id, project_id, kind, title, detail, executable, argv_json, cwd, task_id, shell_mode, status, origin, run_id, error, created_at, resolved_at, expected_ports_json, dev_url_path, dev_url_scheme FROM pending_approvals WHERE id = ?1",
                params![id],
                approval_from_row,
            )
            .map_err(|_| Error::NotFound(id.into()))
    }

    pub fn resolve_task_approval(
        &self,
        id: &str,
        approved: bool,
    ) -> Result<(PendingApproval, Option<TaskSpec>)> {
        const CHANGED_ERROR: &str = "task definition changed; approval denied";
        let (approval, spec, rejection) = self.with_immediate_transaction(|core| {
            let mut approval = core.load_pending_approval(id)?;
            if approval.status != "pending" {
                return Err(Error::msg("approval is no longer pending"));
            }

            if approval_expired(&approval) {
                let resolved_at = now();
                core.conn.execute(
                    "UPDATE pending_approvals SET status = 'expired', error = ?1, resolved_at = ?2 WHERE id = ?3 AND status = 'pending'",
                    params![EXPIRED_ERROR, resolved_at, id],
                )?;
                core.record_audit_event(&approval.origin, "expire_task_approval", "task",
                    approval.task_id.as_deref().or(Some(&approval.project_id)), Some(EXPIRED_ERROR), "expired")?;
                approval.status = "expired".into();
                approval.resolved_at = Some(resolved_at);
                approval.error = Some(EXPIRED_ERROR.into());
                return Ok((approval, None, Some(EXPIRED_ERROR)));
            }

            // BEGIN IMMEDIATE covers this comparison, so a concurrent
            // update_task cannot sneak in after the definition check.
            if approved && core.approval_task_changed(&approval)? {
                let resolved_at = now();
                let changed = core.conn.execute(
                    "UPDATE pending_approvals SET status = 'denied', error = ?1, resolved_at = ?2 WHERE id = ?3 AND status = 'pending'",
                    params![CHANGED_ERROR, resolved_at, id],
                )?;
                if changed != 1 {
                    return Err(Error::msg("approval is no longer pending"));
                }
                core.record_audit_event(
                    &approval.origin,
                    "deny_task_approval",
                    "task",
                    approval.task_id.as_deref().or(Some(&approval.project_id)),
                    Some(CHANGED_ERROR),
                    "denied",
                )?;
                approval.status = "denied".into();
                approval.resolved_at = Some(resolved_at);
                approval.error = Some(CHANGED_ERROR.into());
                return Ok((approval, None, Some(CHANGED_ERROR)));
            }

            let next_status = if approved { "starting" } else { "denied" };
            let resolved_at = if approved { None } else { Some(now()) };
            let task_target = approval
                .task_id
                .clone()
                .unwrap_or_else(|| approval.project_id.clone());
            let audit_action = if approved {
                "start_task_approval"
            } else {
                "deny_task_approval"
            };
            let changed = core.conn.execute(
                "UPDATE pending_approvals SET status = ?1, resolved_at = ?2, error = NULL WHERE id = ?3 AND status = 'pending'",
                params![next_status, resolved_at, id],
            )?;
            if changed != 1 {
                return Err(Error::msg("approval is no longer pending"));
            }
            core.record_audit_event(
                &approval.origin,
                audit_action,
                "task",
                Some(&task_target),
                Some(&approval.title),
                next_status,
            )?;
            approval.status = next_status.into();
            approval.resolved_at = resolved_at;
            approval.error = None;
            let spec = if approved {
                Some(TaskSpec {
                    project_id: approval.project_id.clone(),
                    task_id: approval.task_id.clone(),
                    kind: approval.kind.clone(),
                    executable: approval.executable.clone().unwrap_or_default(),
                    argv: approval.argv.clone(),
                    cwd: approval.cwd.clone(),
                    shell_mode: approval.shell_mode,
                })
            } else {
                None
            };
            Ok((approval, spec, None))
        })?;
        if let Some(reason) = rejection {
            return Err(Error::msg(reason));
        }
        Ok((approval, spec))
    }

    fn approval_task_changed(&self, approval: &PendingApproval) -> Result<bool> {
        let Some(task_id) = approval.task_id.as_deref() else {
            return Ok(false);
        };
        let (tasks_json, project_path): (String, String) = self
            .conn
            .query_row(
                "SELECT tasks_json, canonical_path FROM projects WHERE id = ?1",
                params![approval.project_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or_else(|| Error::NotFound(approval.project_id.clone()))?;
        let tasks: Vec<TaskDefinition> = from_json(tasks_json)?;
        let Some(task) = tasks.iter().find(|task| task.id == task_id) else {
            return Ok(true);
        };
        let approved_cwd =
            crate::paths::normalize(Path::new(approval.cwd.as_deref().unwrap_or(&project_path)));
        let current_cwd = task
            .cwd
            .as_deref()
            .map(PathBuf::from)
            .map(|path| {
                if path.is_absolute() {
                    path
                } else {
                    PathBuf::from(&project_path).join(path)
                }
            })
            .unwrap_or_else(|| PathBuf::from(&project_path));
        let current_cwd = crate::paths::normalize(&current_cwd);
        Ok(task.kind != approval.kind
            || task.name != approval.title
            || task.executable != approval.executable.clone().unwrap_or_default()
            || task.argv != approval.argv
            || current_cwd != approved_cwd
            || task.shell_mode != approval.shell_mode
            || task.expected_ports != approval.expected_ports
            || task.dev_url_path != approval.dev_url_path
            || task.dev_url_scheme != approval.dev_url_scheme)
    }

    pub fn mark_task_approval_started(&self, id: &str, run_id: &str) -> Result<PendingApproval> {
        let approval = self.get_pending_approval(id)?;
        let resolved_at = now();
        let origin = approval.origin.clone();
        let task_target = approval
            .task_id
            .clone()
            .unwrap_or_else(|| approval.project_id.clone());
        let run_id_owned = run_id.to_string();
        let changed = self.with_immediate_transaction(|core| {
            let changed = core.conn.execute(
                "UPDATE pending_approvals SET status = 'started', run_id = ?1, error = NULL, resolved_at = ?2 WHERE id = ?3 AND status = 'starting'",
                params![run_id, resolved_at, id],
            )?;
            if changed == 1 {
                core.record_audit_event(
                    &origin,
                    "task_started",
                    "task",
                    Some(&task_target),
                    Some(&run_id_owned),
                    "started",
                )?;
            }
            Ok(changed)
        })?;
        if changed != 1 {
            return Err(Error::msg("approval is no longer starting"));
        }
        let mut updated = approval;
        updated.status = "started".into();
        updated.run_id = Some(run_id.into());
        updated.error = None;
        updated.resolved_at = Some(resolved_at);
        Ok(updated)
    }

    pub fn mark_task_approval_failed(&self, id: &str, error: &str) -> Result<PendingApproval> {
        let approval = self.get_pending_approval(id)?;
        let resolved_at = now();
        let safe_error = sanitize_error(error);
        let origin = approval.origin.clone();
        let task_target = approval
            .task_id
            .clone()
            .unwrap_or_else(|| approval.project_id.clone());
        let changed = self.with_immediate_transaction(|core| {
            let changed = core.conn.execute(
                "UPDATE pending_approvals SET status = 'failed', error = ?1, resolved_at = ?2 WHERE id = ?3 AND status = 'starting'",
                params![safe_error, resolved_at, id],
            )?;
            if changed == 1 {
                core.record_audit_event(
                    &origin,
                    "task_start_failed",
                    "task",
                    Some(&task_target),
                    Some(&safe_error),
                    "failed",
                )?;
            }
            Ok(changed)
        })?;
        if changed != 1 {
            return Err(Error::msg("approval is no longer starting"));
        }
        let mut updated = approval;
        updated.status = "failed".into();
        updated.error = Some(safe_error.clone());
        updated.resolved_at = Some(resolved_at);
        Ok(updated)
    }
}

fn approval_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PendingApproval> {
    let argv_json: String = row.get(6)?;
    let expected_ports_json: String = row.get(16)?;
    Ok(PendingApproval {
        id: row.get(0)?,
        project_id: row.get(1)?,
        kind: row.get(2)?,
        title: row.get(3)?,
        detail: row.get(4)?,
        executable: row.get(5)?,
        argv: serde_json::from_str(&argv_json).unwrap_or_default(),
        cwd: row.get(7)?,
        task_id: row.get(8)?,
        shell_mode: row.get::<_, i64>(9)? != 0,
        status: row.get(10)?,
        origin: row.get(11)?,
        run_id: row.get(12)?,
        error: row.get(13)?,
        created_at: row.get(14)?,
        resolved_at: row.get(15)?,
        expected_ports: serde_json::from_str(&expected_ports_json).unwrap_or_default(),
        dev_url_path: row.get(17)?,
        dev_url_scheme: row.get(18)?,
    })
}

fn sanitize_error(error: &str) -> String {
    error
        .chars()
        .filter(|character| *character != '\0' && *character != '\r' && *character != '\n')
        .take(2_000)
        .collect()
}
