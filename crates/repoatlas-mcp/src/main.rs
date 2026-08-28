use repoatlas_core::{
    Broker, Core, Error as CoreError, ProjectPatch, ProjectQuery, TaskDefinition, TaskSpec,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

mod args;
mod schema;

use args::{
    optional_bool, optional_nullable_string, optional_string, require_confirmation,
    required_string, string_array,
};
use schema::tool_schemas;

#[derive(Debug, Deserialize)]
struct RpcRequest {
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

fn task_log_dir(db_path: Option<&PathBuf>) -> PathBuf {
    db_path
        .and_then(|path| path.parent().map(|parent| parent.join("task-logs")))
        .unwrap_or_else(|| std::env::temp_dir().join("repoatlas-task-logs"))
}

fn main() -> io::Result<()> {
    let db_path = std::env::var("REPOATLAS_DB").ok().map(PathBuf::from);
    let log_dir = task_log_dir(db_path.as_ref());
    let finish_db = db_path.clone();
    let core = match db_path {
        // MCP may run alongside the desktop app. Do not mark the desktop
        // Broker's live task rows as failed just because this read/write
        // control plane opened the shared database.
        Some(path) => Core::open_without_recovery(path).expect("open core"),
        None => Core::open_in_memory().expect("open in-memory core"),
    };
    let broker = Broker::new(log_dir).expect("open broker");
    // Wrap in a reader over stdin (UTF-8, newline-delimited JSON).
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut reader = stdin.lock();

    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let request: RpcRequest = match serde_json::from_str(line) {
            Ok(req) => req,
            Err(_) => continue,
        };
        // JSON-RPC notifications deliberately have no response, including
        // error responses. Still dispatch the request so a notification can
        // perform a permitted metadata mutation.
        if request.id.is_none() {
            let _ = dispatch(&core, &broker, finish_db.as_ref(), &request);
            continue;
        }
        let response = dispatch(&core, &broker, finish_db.as_ref(), &request);
        let _ = writeln!(out, "{response}");
        let _ = out.flush();
    }
    Ok(())
}

fn dispatch(
    core: &Core,
    broker: &Broker,
    db_path: Option<&PathBuf>,
    request: &RpcRequest,
) -> String {
    let id = request.id.clone();
    if request.method == "initialize" {
        let result = json!({
            "protocolVersion": "2025-06-18",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "repoatlas-mcp", "version": env!("CARGO_PKG_VERSION") }
        });
        return respond(id, Ok(result));
    }
    if request.method == "tools/list" {
        let result = json!({ "tools": tool_schemas() });
        return respond(id, Ok(result));
    }
    // Keep the complete tools/call envelope here. `route` needs both the tool name and its
    // arguments; passing only params.arguments would silently turn every call into an unknown
    // tool request.
    let result = route(core, broker, db_path, &request.method, &request.params);
    respond(id, result)
}

fn route(
    core: &Core,
    broker: &Broker,
    db_path: Option<&PathBuf>,
    tool: &str,
    p: &Value,
) -> Result<Value, String> {
    match tool {
        "tools/call" => {
            let name = p.get("name").and_then(Value::as_str).unwrap_or("");
            let args = p
                .get("arguments")
                .cloned()
                .unwrap_or(Value::Object(Default::default()));
            call(core, broker, db_path, name, &args)
                .map(|result| json!({ "content": [ { "type": "text", "text": result } ] }))
        }
        _ => Err(format!("unknown method: {tool}")),
    }
}

fn call(
    core: &Core,
    _broker: &Broker,
    _db_path: Option<&PathBuf>,
    name: &str,
    args: &Value,
) -> Result<String, String> {
    match name {
        "list_projects" => {
            let section = args
                .get("section")
                .and_then(Value::as_str)
                .map(str::to_string);
            let search = args
                .get("search")
                .and_then(Value::as_str)
                .map(str::to_string);
            let projects = core
                .list_projects(ProjectQuery {
                    section: section.clone(),
                    search,
                    include_archived: Some(section.as_deref() == Some("archived")),
                })
                .map_err(|e| e.to_string())?;
            Ok(serde_json::to_string_pretty(&projects).unwrap())
        }
        "search_projects" => {
            let query = required_string(args, "query")?;
            let hits = core.search_projects(query, 50).map_err(|e| e.to_string())?;
            Ok(serde_json::to_string_pretty(&hits).unwrap())
        }
        "get_project" => {
            let id = required_string(args, "id")?;
            let detail = core.get_project(id).map_err(|e| e.to_string())?;
            Ok(serde_json::to_string_pretty(&detail).unwrap())
        }
        "register_project" => {
            let path = required_string(args, "path")?;
            let project = core
                .register_project_with_origin(path, "mcp")
                .map_err(|e| e.to_string())?;
            Ok(serde_json::to_string(&project).unwrap())
        }
        "update_project" => {
            let id = required_string(args, "id")?.to_string();
            let display_name = optional_string(args, "displayName")?;
            // ProjectPatch uses `None` to mean "leave unchanged". Encode an explicit JSON null
            // as an empty note so Core can normalize it to NULL and clear a previous value.
            let notes = match args.get("notes") {
                None => None,
                Some(Value::Null) => Some(String::new()),
                Some(Value::String(value)) => Some(value.clone()),
                Some(_) => return Err("notes must be a string or null".to_string()),
            };
            let description = match args.get("description") {
                Some(Value::String(value)) => Some(Some(value.clone())),
                Some(Value::Null) => Some(None),
                Some(_) => return Err("description must be a string or null".to_string()),
                None => None,
            };
            let patch = ProjectPatch {
                display_name,
                notes,
                description,
                favorite: optional_bool(args, "favorite")?,
                archived: optional_bool(args, "archived")?,
                tags: string_array(args, "tags")?,
                tasks: match args.get("tasks") {
                    Some(value) => Some(
                        serde_json::from_value::<Vec<TaskDefinition>>(value.clone())
                            .map_err(|err| err.to_string())?,
                    ),
                    None => None,
                },
            };
            let project = core
                .update_project_with_origin(&id, patch, "mcp")
                .map_err(|e| e.to_string())?;
            Ok(serde_json::to_string(&project).unwrap())
        }
        "remove_project" => {
            let project_id = required_string(args, "projectId")?;
            require_confirmation(args)?;
            let removal = match core.remove_project_with_origin(project_id, "mcp") {
                Ok(removal) => removal,
                Err(CoreError::NotFound(_)) => {
                    record_mcp_audit(
                        core,
                        "remove_project",
                        "project",
                        Some(project_id),
                        None,
                        "not_found",
                    )?;
                    return Ok(json!({
                        "status": "not_found",
                        "projectId": project_id,
                        "filesystemDeleted": false
                    })
                    .to_string());
                }
                Err(error) => return Err(error.to_string()),
            };
            Ok(serde_json::to_string_pretty(&removal).unwrap())
        }
        "relocate_project" => {
            let project_id = required_string(args, "projectId")?;
            let path = required_string(args, "path")?;
            let project = core
                .relocate_project_with_origin(project_id, path, "mcp")
                .map_err(|e| e.to_string())?;
            Ok(serde_json::to_string(&project).unwrap())
        }
        "refresh_project" => {
            let project_id = required_string(args, "projectId")?;
            let project = core
                .refresh_project_with_origin(project_id, "mcp")
                .map_err(|e| e.to_string())?;
            Ok(serde_json::to_string(&project).unwrap())
        }
        "set_project_icon" => {
            let project_id = required_string(args, "projectId")?;
            let path = required_string(args, "path")?;
            let icon = core
                .set_project_icon_with_origin(project_id, path, "mcp")
                .map_err(|e| e.to_string())?;
            Ok(json!({
                "projectId": icon.project_id,
                "kind": icon.kind,
                "source": icon.source,
                "mimeType": icon.mime_type,
            })
            .to_string())
        }
        "clear_project_icon" => {
            let project_id = required_string(args, "projectId")?;
            let icon = core
                .clear_project_icon_with_origin(project_id, "mcp")
                .map_err(|e| e.to_string())?;
            Ok(json!({
                "projectId": icon.project_id,
                "kind": icon.kind,
                "source": icon.source,
                "mimeType": icon.mime_type,
            })
            .to_string())
        }
        "read_project_icons" => {
            let project_ids = string_array(args, "projectIds")?
                .ok_or_else(|| "missing required array argument: projectIds".to_string())?;
            if project_ids.is_empty() || project_ids.len() > 48 {
                return Err("projectIds must contain between 1 and 48 items".into());
            }
            let icons = core
                .read_project_icons(&project_ids)
                .map_err(|e| e.to_string())?;
            // Keep image bytes in the local desktop surface. An MCP client only needs the
            // selected source and kind to manage metadata without receiving a large data URL.
            let metadata = icons
                .into_iter()
                .map(|icon| {
                    json!({
                        "projectId": icon.project_id,
                        "kind": icon.kind,
                        "source": icon.source,
                        "mimeType": icon.mime_type
                    })
                })
                .collect::<Vec<_>>();
            Ok(serde_json::to_string_pretty(&metadata).unwrap())
        }
        "add_task" => {
            let project_id = required_string(args, "projectId")?.to_string();
            let name = required_string(args, "name")?.to_string();
            let executable = required_string(args, "executable")?.to_string();
            let argv = string_array(args, "argv")?.unwrap_or_default();
            let shell_mode = optional_bool(args, "shellMode")?.unwrap_or(false);
            if shell_mode {
                return Err("shell-mode tasks cannot be managed from MCP".into());
            }
            let task = TaskDefinition {
                id: optional_string(args, "id")?.unwrap_or_default(),
                kind: optional_string(args, "kind")?.unwrap_or_else(|| "run".into()),
                name,
                description: optional_nullable_string(args, "description")?,
                executable,
                argv,
                cwd: optional_nullable_string(args, "cwd")?,
                inferred: false,
                shell_mode,
            };
            let detail = core
                .add_task_with_origin(&project_id, task, "mcp")
                .map_err(|e| e.to_string())?;
            Ok(serde_json::to_string_pretty(&detail).unwrap())
        }
        "update_task" => {
            let project_id = required_string(args, "projectId")?;
            let task_id = required_string(args, "taskId")?;
            let detail = core.get_project(project_id).map_err(|e| e.to_string())?;
            let Some(existing) = detail.tasks.iter().find(|task| task.id == task_id) else {
                return Err(format!("task not found: {task_id}"));
            };
            let requested_shell_mode = optional_bool(args, "shellMode")?;
            if requested_shell_mode == Some(true) {
                return Err("shell-mode tasks cannot be managed from MCP".into());
            }
            let shell_mode = requested_shell_mode.unwrap_or(existing.shell_mode);
            let task = TaskDefinition {
                id: existing.id.clone(),
                kind: optional_string(args, "kind")?.unwrap_or_else(|| existing.kind.clone()),
                name: optional_string(args, "name")?.unwrap_or_else(|| existing.name.clone()),
                description: if args.get("description").is_some() {
                    optional_nullable_string(args, "description")?
                } else {
                    existing.description.clone()
                },
                executable: optional_string(args, "executable")?
                    .unwrap_or_else(|| existing.executable.clone()),
                argv: string_array(args, "argv")?.unwrap_or_else(|| existing.argv.clone()),
                cwd: if args.get("cwd").is_some() {
                    optional_nullable_string(args, "cwd")?
                } else {
                    existing.cwd.clone()
                },
                inferred: false,
                shell_mode,
            };
            let updated = core
                .update_task_with_origin(project_id, task_id, task, "mcp")
                .map_err(|e| e.to_string())?;
            Ok(serde_json::to_string_pretty(&updated).unwrap())
        }
        "remove_task" => {
            let project_id = required_string(args, "projectId")?;
            let task_id = required_string(args, "taskId")?;
            let updated = core
                .remove_task_with_origin(project_id, task_id, "mcp")
                .map_err(|e| e.to_string())?;
            Ok(serde_json::to_string_pretty(&updated).unwrap())
        }
        "list_task_runs" => {
            let project_id = required_string(args, "projectId")?;
            let runs = core.list_task_runs(project_id).map_err(|e| e.to_string())?;
            Ok(serde_json::to_string_pretty(&runs).unwrap())
        }
        "run_task" => {
            let project_id = required_string(args, "projectId")?.to_string();
            let detail = core.get_project(&project_id).map_err(|e| e.to_string())?;
            let task = if let Some(task_id) = args.get("taskId").and_then(Value::as_str) {
                detail.tasks.iter().find(|item| item.id == task_id).cloned()
            } else if let Some(name) = args.get("name").and_then(Value::as_str) {
                let matches: Vec<_> = detail
                    .tasks
                    .iter()
                    .filter(|item| item.name == name)
                    .cloned()
                    .collect();
                if matches.len() > 1 {
                    return Err(format!("multiple tasks named {name}; pass taskId"));
                }
                matches.into_iter().next()
            } else if detail.tasks.len() == 1 {
                detail.tasks.first().cloned()
            } else {
                None
            };
            let Some(task) = task else {
                return Err("task not found; pass taskId or a unique name".into());
            };
            if task.shell_mode {
                return Err("shell-mode tasks cannot be started from MCP".into());
            }
            let spec = TaskSpec {
                project_id: project_id.clone(),
                task_id: Some(task.id.clone()),
                kind: task.kind.clone(),
                executable: task.executable.clone(),
                argv: task.argv.clone(),
                cwd: task
                    .cwd
                    .clone()
                    .or_else(|| Some(detail.project.canonical_path.clone())),
                shell_mode: false,
            };
            let approval = core
                .request_task_approval_from("mcp", spec)
                .map_err(|e| e.to_string())?;
            Ok(json!({
                "status": approval.status,
                "approvalId": approval.id,
                "projectId": approval.project_id,
                "taskId": approval.task_id,
                "title": approval.title,
                "detail": approval.detail,
                "origin": approval.origin,
                "runId": approval.run_id,
                "error": approval.error,
                "message": "Task is waiting for desktop approval and was not started."
            })
            .to_string())
        }
        "get_task_run" => {
            let run_id = required_string(args, "runId")?;
            let run = core.get_task_run(run_id).map_err(|e| e.to_string())?;
            Ok(serde_json::to_string_pretty(&run).unwrap())
        }
        "read_task_log" => {
            let run_id = required_string(args, "runId")?;
            let log = core.read_task_log(run_id).map_err(|e| e.to_string())?;
            Ok(json!({ "runId": run_id, "log": log }).to_string())
        }
        "inspect_project_environment" => {
            let project_id = required_string(args, "projectId")?;
            let environment = core
                .inspect_project_environment(project_id)
                .map_err(|e| e.to_string())?;
            Ok(serde_json::to_string_pretty(&environment).unwrap())
        }
        "read_project_file" => {
            let project_id = required_string(args, "projectId")?;
            let path = args
                .get("path")
                .or_else(|| args.get("relativePath"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "missing required string argument: path".to_string())?;
            let document = core
                .read_project_file(project_id, path)
                .map_err(|e| e.to_string())?;
            Ok(serde_json::to_string_pretty(&document).unwrap())
        }
        "list_project_events" => {
            let project_id = required_string(args, "projectId")?;
            // Keep MCP responses bounded even when an agent sends an excessively large limit.
            let limit = args
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(20)
                .clamp(1, 100) as usize;
            core.get_project(project_id).map_err(|e| e.to_string())?;
            let events = core
                .list_project_events(project_id, limit)
                .map_err(|e| e.to_string())?;
            Ok(serde_json::to_string_pretty(&events).unwrap())
        }
        "atlas_report" => {
            let project_id = required_string(args, "projectId")?;
            let report = core.atlas_report(project_id).map_err(|e| e.to_string())?;
            Ok(serde_json::to_string_pretty(&report).unwrap())
        }
        "scan_root" => {
            let root_id = required_string(args, "rootId")?.to_string();
            let result = core
                .scan_root_with_origin(
                    &root_id,
                    &std::sync::atomic::AtomicBool::new(false),
                    &|_| {},
                    "mcp",
                )
                .map_err(|e| e.to_string())?;
            Ok(serde_json::to_string(&result).unwrap())
        }
        "add_scan_root" => {
            let path = required_string(args, "path")?;
            let root = core
                .add_scan_root_with_origin(path, "mcp")
                .map_err(|e| e.to_string())?;
            Ok(serde_json::to_string(&root).unwrap())
        }
        "remove_scan_root" => {
            let root_id = required_string(args, "rootId")?;
            require_confirmation(args)?;
            let policy = required_string(args, "recordPolicy")?;
            let remove_records = match policy {
                "orphan" => false,
                "removeRecords" => true,
                _ => return Err("recordPolicy must be orphan or removeRecords".into()),
            };
            let removal = match core.remove_scan_root_with_origin(root_id, remove_records, "mcp") {
                Ok(removal) => removal,
                Err(CoreError::NotFound(_)) => {
                    record_mcp_audit(
                        core,
                        "remove_scan_root",
                        "scan_root",
                        Some(root_id),
                        Some(policy),
                        "not_found",
                    )?;
                    return Ok(json!({
                        "status": "not_found",
                        "rootId": root_id,
                        "recordPolicy": policy,
                        "filesystemDeleted": false
                    })
                    .to_string());
                }
                Err(error) => return Err(error.to_string()),
            };
            Ok(json!({
                "status": "removed",
                "rootId": root_id,
                "recordPolicy": policy,
                "root": removal.root,
                "projectsRemoved": removal.removed_projects,
                "orphanedProjects": removal.orphaned_projects,
                "pendingLogCleanup": removal.pending_log_cleanup,
                "filesystemDeleted": removal.filesystem_deleted
            })
            .to_string())
        }
        "list_scan_roots" => {
            let roots = core.list_scan_roots().map_err(|e| e.to_string())?;
            Ok(serde_json::to_string(&roots).unwrap())
        }
        "list_memory" => {
            let project_id = required_string(args, "projectId")?;
            core.get_project(project_id).map_err(|e| e.to_string())?;
            let items = core.list_memory(project_id).map_err(|e| e.to_string())?;
            Ok(serde_json::to_string_pretty(&items).unwrap())
        }
        "add_memory" => {
            let project_id = required_string(args, "projectId")?;
            let text = required_string(args, "text")?;
            core.get_project(project_id).map_err(|e| e.to_string())?;
            let item = core
                .add_memory_with_origin(project_id, text, "mcp")
                .map_err(|e| e.to_string())?;
            Ok(serde_json::to_string_pretty(&item).unwrap())
        }
        "delete_memory" => {
            let memory_id = required_string(args, "memoryId")?;
            core.delete_memory_with_origin(memory_id, "mcp")
                .map_err(|e| e.to_string())?;
            Ok(json!({ "status": "deleted", "memoryId": memory_id }).to_string())
        }
        "list_summaries" => {
            let project_id = required_string(args, "projectId")?;
            core.get_project(project_id).map_err(|e| e.to_string())?;
            let summaries = core.list_summaries(project_id).map_err(|e| e.to_string())?;
            Ok(serde_json::to_string_pretty(&summaries).unwrap())
        }
        "get_summary" => {
            let summary_id = required_string(args, "summaryId")?;
            let summary = core.get_summary(summary_id).map_err(|e| e.to_string())?;
            Ok(serde_json::to_string_pretty(&summary).unwrap())
        }
        "delete_summary" => {
            let summary_id = required_string(args, "summaryId")?;
            core.delete_summary_with_origin(summary_id, "mcp")
                .map_err(|e| e.to_string())?;
            Ok(json!({ "status": "deleted", "summaryId": summary_id }).to_string())
        }
        "accept_summary_memory" => {
            let summary_id = required_string(args, "summaryId")?;
            let memory = core
                .accept_summary_as_memory_with_origin(summary_id, "mcp")
                .map_err(|e| e.to_string())?;
            Ok(serde_json::to_string_pretty(&memory).unwrap())
        }
        "conversation_summary" => {
            let project_id = required_string(args, "projectId")?;
            core.get_project(project_id).map_err(|e| e.to_string())?;
            let summary = core
                .conversation_summary(project_id)
                .map_err(|e| e.to_string())?;
            Ok(serde_json::to_string_pretty(&summary).unwrap())
        }
        "list_conversation" => {
            let project_id = required_string(args, "projectId")?;
            core.get_project(project_id).map_err(|e| e.to_string())?;
            let messages = core
                .list_conversation(project_id)
                .map_err(|e| e.to_string())?;
            Ok(serde_json::to_string_pretty(&messages).unwrap())
        }
        "clear_conversation" => {
            let project_id = required_string(args, "projectId")?;
            core.get_project(project_id).map_err(|e| e.to_string())?;
            core.clear_conversation_with_origin(project_id, "mcp")
                .map_err(|e| e.to_string())?;
            Ok(json!({ "status": "cleared", "projectId": project_id }).to_string())
        }
        "list_audit_events" => {
            let limit = args
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(100)
                .clamp(1, 500) as usize;
            let events = core.list_audit_events(limit).map_err(|e| e.to_string())?;
            Ok(serde_json::to_string_pretty(&events).unwrap())
        }
        "git_status" => {
            let project_id = required_string(args, "projectId")?;
            let status = core.git_status(project_id).map_err(|e| e.to_string())?;
            Ok(serde_json::to_string_pretty(&status).unwrap())
        }
        "git_diff" => {
            let project_id = required_string(args, "projectId")?;
            let path = optional_nullable_string(args, "path")?;
            let staged = optional_bool(args, "staged")?.unwrap_or(false);
            let diff = core
                .git_diff(project_id, path.as_deref(), staged)
                .map_err(|e| e.to_string())?;
            Ok(serde_json::to_string_pretty(&diff).unwrap())
        }
        _ => Err(format!("unknown tool: {name}")),
    }
}

fn record_mcp_audit(
    core: &Core,
    action: &str,
    target_type: &str,
    target_id: Option<&str>,
    detail: Option<&str>,
    outcome: &str,
) -> Result<(), String> {
    // MCP mutations are only acknowledged after their audit record is durable.
    // Core owns the table and migration; this adapter supplies the external
    // origin label and keeps sensitive path/credential values out of detail.
    core.record_audit_event("mcp", action, target_type, target_id, detail, outcome)
        .map(|_| ())
        .map_err(|error| format!("operation completed but audit could not be persisted: {error}"))
}

fn respond(id: Option<Value>, result: Result<Value, String>) -> String {
    match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string(),
        Err(error) => {
            json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32000, "message": error } })
                .to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_list_declares_real_parameter_schemas() {
        let core = Core::open_in_memory().expect("core");
        let request = RpcRequest {
            id: Some(json!(1)),
            method: "tools/list".into(),
            params: json!({}),
        };
        let broker =
            Broker::new(std::env::temp_dir().join("repoatlas-mcp-test-logs")).expect("broker");
        let response: Value =
            serde_json::from_str(&dispatch(&core, &broker, None, &request)).expect("response");
        let tools = response["result"]["tools"].as_array().expect("tools");
        for tool in tools {
            let schema = &tool["inputSchema"];
            assert_eq!(schema["type"], "object", "{}", tool["name"]);
            assert!(schema["properties"].is_object(), "{}", tool["name"]);
        }

        let update = tools
            .iter()
            .find(|tool| tool["name"] == "update_project")
            .expect("update_project");
        assert_eq!(update["inputSchema"]["required"], json!(["id"]));
        assert_eq!(
            update["inputSchema"]["properties"]["description"]["type"],
            json!(["string", "null"])
        );
        let set_icon = tools
            .iter()
            .find(|tool| tool["name"] == "set_project_icon")
            .expect("set_project_icon");
        assert_eq!(
            set_icon["inputSchema"]["required"],
            json!(["projectId", "path"])
        );
        assert_eq!(
            set_icon["inputSchema"]["additionalProperties"],
            json!(false)
        );
        let clear_icon = tools
            .iter()
            .find(|tool| tool["name"] == "clear_project_icon")
            .expect("clear_project_icon");
        assert_eq!(clear_icon["inputSchema"]["required"], json!(["projectId"]));

        let remove = tools
            .iter()
            .find(|tool| tool["name"] == "remove_project")
            .expect("remove_project");
        assert_eq!(
            remove["inputSchema"]["required"],
            json!(["projectId", "confirm"])
        );
        assert_eq!(
            remove["inputSchema"]["properties"]["confirm"]["const"],
            true
        );
        let remove_root = tools
            .iter()
            .find(|tool| tool["name"] == "remove_scan_root")
            .expect("remove_scan_root");
        assert_eq!(
            remove_root["inputSchema"]["required"],
            json!(["rootId", "recordPolicy", "confirm"])
        );

        for name in [
            "search_projects",
            "get_project",
            "register_project",
            "add_scan_root",
            "add_task",
            "list_task_runs",
            "run_task",
            "get_task_run",
            "read_task_log",
            "scan_root",
            "set_project_icon",
            "clear_project_icon",
            "remove_project",
            "relocate_project",
            "refresh_project",
            "read_project_icons",
            "update_task",
            "remove_task",
            "inspect_project_environment",
            "read_project_file",
            "list_project_events",
            "atlas_report",
            "remove_scan_root",
            "list_memory",
            "add_memory",
            "delete_memory",
            "list_summaries",
            "get_summary",
            "delete_summary",
            "accept_summary_memory",
            "conversation_summary",
            "list_conversation",
            "clear_conversation",
            "git_status",
            "git_diff",
        ] {
            let tool = tools.iter().find(|tool| tool["name"] == name).expect(name);
            assert!(
                tool["inputSchema"]["required"].as_array().is_some(),
                "{name}"
            );
        }
    }

    #[test]
    fn missing_required_arguments_are_rejected() {
        let core = Core::open_in_memory().expect("core");
        let broker =
            Broker::new(std::env::temp_dir().join("repoatlas-mcp-test-logs")).expect("broker");
        assert!(call(&core, &broker, None, "search_projects", &json!({})).is_err());
        assert!(call(&core, &broker, None, "get_project", &json!({"id":""})).is_err());
        assert!(call(
            &core,
            &broker,
            None,
            "remove_project",
            &json!({"projectId":"p"})
        )
        .unwrap_err()
        .contains("confirm=true"));
    }

    #[test]
    fn json_rpc_tools_call_routes_the_declared_tool_and_arguments() {
        let core = Core::open_in_memory().expect("core");
        let broker =
            Broker::new(std::env::temp_dir().join("repoatlas-mcp-dispatch-logs")).expect("broker");
        let request = RpcRequest {
            id: Some(json!(7)),
            method: "tools/call".into(),
            params: json!({ "name": "list_scan_roots", "arguments": {} }),
        };
        let response: Value =
            serde_json::from_str(&dispatch(&core, &broker, None, &request)).expect("response");
        assert_eq!(response["id"], json!(7));
        assert!(response["result"]["content"][0]["text"]
            .as_str()
            .expect("text")
            .contains('['));
        assert!(response.get("error").is_none());
    }

    #[test]
    fn remove_project_only_removes_repoatlas_records() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let project_path = std::env::temp_dir().join(format!("repoatlas-mcp-remove-{nonce}"));
        std::fs::create_dir_all(&project_path).expect("project dir");
        std::fs::write(project_path.join("README.md"), "# managed").expect("readme");

        let core = Core::open_in_memory().expect("core");
        let project = core.register_project(&project_path).expect("project");
        let broker = Broker::new(std::env::temp_dir().join(format!("repoatlas-mcp-logs-{nonce}")))
            .expect("broker");

        call(
            &core,
            &broker,
            None,
            "update_project",
            &json!({"id": project.id, "notes": "temporary note"}),
        )
        .expect("set notes");
        call(
            &core,
            &broker,
            None,
            "update_project",
            &json!({"id": project.id, "notes": null}),
        )
        .expect("clear notes");
        assert!(core
            .get_project(&project.id)
            .expect("project after notes")
            .project
            .notes
            .is_none());

        let response = call(
            &core,
            &broker,
            None,
            "remove_project",
            &json!({"projectId": project.id, "confirm": true}),
        )
        .expect("remove project");
        let response: Value = serde_json::from_str(&response).expect("json");
        assert_eq!(response["filesystemDeleted"], false);
        assert_eq!(response["project"]["id"], project.id);
        assert!(project_path.exists(), "MCP must never delete project files");
        assert!(core.get_project(&project.id).is_err());

        let response = call(
            &core,
            &broker,
            None,
            "remove_project",
            &json!({"projectId": project.id, "confirm": true}),
        )
        .expect("repeat remove project");
        let response: Value = serde_json::from_str(&response).expect("json");
        assert_eq!(response["status"], "not_found");

        let _ = std::fs::remove_dir_all(project_path);
    }

    #[test]
    fn task_management_preserves_argument_boundaries() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let project_path = std::env::temp_dir().join(format!("repoatlas-mcp-task-{nonce}"));
        std::fs::create_dir_all(&project_path).expect("project dir");

        let core = Core::open_in_memory().expect("core");
        let project = core.register_project(&project_path).expect("project");
        let broker = Broker::new(std::env::temp_dir().join(format!("repoatlas-mcp-logs-{nonce}")))
            .expect("broker");
        let response = call(
            &core,
            &broker,
            None,
            "add_task",
            &json!({
                "projectId": project.id,
                "id": "test",
                "name": "Test",
                "executable": "node",
                "argv": ["--title", "hello world"]
            }),
        )
        .expect("add task");
        let detail: Value = serde_json::from_str(&response).expect("detail json");
        assert_eq!(
            detail["tasks"][0]["argv"],
            json!(["--title", "hello world"])
        );

        let response = call(
            &core,
            &broker,
            None,
            "update_task",
            &json!({
                "projectId": project.id,
                "taskId": "test",
                "argv": ["--title", "updated value"]
            }),
        )
        .expect("update task");
        let detail: Value = serde_json::from_str(&response).expect("detail json");
        assert_eq!(
            detail["tasks"][0]["argv"],
            json!(["--title", "updated value"])
        );

        let response = call(
            &core,
            &broker,
            None,
            "remove_task",
            &json!({"projectId": project.id, "taskId": "test"}),
        )
        .expect("remove task");
        let detail: Value = serde_json::from_str(&response).expect("detail json");
        assert!(detail["tasks"].as_array().expect("tasks").is_empty());

        let _ = std::fs::remove_dir_all(project_path);
    }

    #[test]
    fn run_task_creates_an_approval_without_starting_a_process() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let project_path = std::env::temp_dir().join(format!("repoatlas-mcp-approval-{nonce}"));
        std::fs::create_dir_all(&project_path).expect("project dir");
        std::fs::write(
            project_path.join("package.json"),
            r#"{"name":"mcp-approval"}"#,
        )
        .expect("manifest");

        let core = Core::open_in_memory().expect("core");
        let project = core.register_project(&project_path).expect("project");
        let detail = core
            .add_task(
                &project.id,
                TaskDefinition {
                    id: "dev".into(),
                    kind: "dev".into(),
                    name: "Development server".into(),
                    description: None,
                    executable: "pnpm".into(),
                    argv: vec!["dev".into()],
                    cwd: None,
                    inferred: false,
                    shell_mode: false,
                },
            )
            .expect("task");
        let task_id = &detail.tasks[0].id;
        let broker = Broker::new(std::env::temp_dir().join(format!("repoatlas-mcp-logs-{nonce}")))
            .expect("broker");

        let response = call(
            &core,
            &broker,
            None,
            "run_task",
            &json!({"projectId": project.id, "taskId": task_id}),
        )
        .expect("approval response");
        let response: Value = serde_json::from_str(&response).expect("json");
        assert_eq!(response["status"], "pending");
        assert_eq!(core.list_pending_approvals().expect("approvals").len(), 1);
        assert!(core.list_task_runs(&project.id).expect("runs").is_empty());

        let _ = std::fs::remove_dir_all(project_path);
    }
}
