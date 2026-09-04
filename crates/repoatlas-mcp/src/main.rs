use repoatlas_core::{
    Broker, CollectionUpsert, Core, Error as CoreError, ProjectPatch, ProjectQuery, TaskDefinition,
    TaskSpec,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
};

mod args;
mod schema;

use args::{
    optional_bool, optional_nullable_string, optional_string, require_confirmation,
    required_string, string_array,
};
use schema::tool_schemas;

const MCP_PROTOCOL_VERSION: &str = "2025-06-18";

#[derive(Debug, Deserialize)]
struct RpcRequest {
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug)]
struct ParsedRequest {
    request: RpcRequest,
    // `RpcRequest.id` retains its historical shape for direct unit tests. This
    // separate bit distinguishes a missing id (notification) from id: null.
    has_id: bool,
}

#[derive(Debug)]
struct EnvelopeError {
    code: i32,
    message: &'static str,
    id: Value,
    has_id: bool,
    always_respond: bool,
}

#[derive(Debug, Default)]
struct ProtocolState {
    initialized: bool,
    initialized_notification: bool,
}

trait ProtocolStateAccess {
    fn is_initialized(&self) -> bool;
    fn mark_initialized(&mut self);
    fn mark_initialized_notification(&mut self);
}

impl ProtocolStateAccess for ProtocolState {
    fn is_initialized(&self) -> bool {
        self.initialized
    }

    fn mark_initialized(&mut self) {
        self.initialized = true;
    }

    fn mark_initialized_notification(&mut self) {
        self.initialized_notification = true;
    }
}

// A bool keeps the helper convenient for focused tests and callers that only
// need the request-handshake state. The full stdin loop uses ProtocolState so
// the initialized notification is tracked independently as required by MCP.
impl ProtocolStateAccess for bool {
    fn is_initialized(&self) -> bool {
        *self
    }

    fn mark_initialized(&mut self) {
        *self = true;
    }

    fn mark_initialized_notification(&mut self) {
        // A lifecycle notification does not complete the initialize request.
    }
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
    struct WorkItem {
        line: String,
        request_key: Option<String>,
        cancel: Arc<AtomicBool>,
    }

    let active = Arc::new(Mutex::new(HashMap::<String, Arc<AtomicBool>>::new()));
    let (work_tx, work_rx) = mpsc::channel::<WorkItem>();
    let (response_tx, response_rx) = mpsc::channel::<String>();
    let worker_active = active.clone();
    let worker = std::thread::spawn(move || {
        let mut protocol_state = ProtocolState::default();
        for work in work_rx {
            if let Some(response) = handle_line_with_cancel(
                &core,
                &broker,
                finish_db.as_ref(),
                &work.line,
                &mut protocol_state,
                work.cancel.as_ref(),
            ) {
                let _ = response_tx.send(response);
            }
            if let Some(key) = work.request_key {
                if let Ok(mut active) = worker_active.lock() {
                    active.remove(&key);
                }
            }
        }
    });
    let writer = std::thread::spawn(move || {
        let stdout = io::stdout();
        let mut out = stdout.lock();
        for response in response_rx {
            let _ = writeln!(out, "{response}");
            let _ = out.flush();
        }
    });

    // Keep stdin responsive while the single database worker performs a long
    // operation. Requests remain ordered, but cancellation notifications can
    // interrupt the active scan instead of waiting behind it.
    let stdin = io::stdin();
    let mut reader = stdin.lock();

    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let parsed = serde_json::from_str::<Value>(line.trim()).ok();
        if parsed
            .as_ref()
            .and_then(|value| value.get("method"))
            .and_then(Value::as_str)
            == Some("notifications/cancelled")
        {
            if let Some(key) = parsed
                .as_ref()
                .and_then(|value| value.get("params"))
                .and_then(|params| params.get("requestId"))
                .map(Value::to_string)
            {
                if let Some(cancel) = active
                    .lock()
                    .ok()
                    .and_then(|items| items.get(&key).cloned())
                {
                    cancel.store(true, Ordering::Relaxed);
                }
            }
            continue;
        }
        let request_key = parsed
            .as_ref()
            .and_then(|value| value.get("id"))
            .map(Value::to_string);
        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(key) = request_key.as_ref() {
            if let Ok(mut active) = active.lock() {
                active.insert(key.clone(), cancel.clone());
            }
        }
        if work_tx
            .send(WorkItem {
                line,
                request_key,
                cancel,
            })
            .is_err()
        {
            break;
        }
    }
    drop(work_tx);
    let _ = worker.join();
    let _ = writer.join();
    Ok(())
}

#[cfg(test)]
fn handle_line<S: ProtocolStateAccess>(
    core: &Core,
    broker: &Broker,
    db_path: Option<&PathBuf>,
    line: &str,
    initialized_state: &mut S,
) -> Option<String> {
    let cancel = AtomicBool::new(false);
    handle_line_with_cancel(core, broker, db_path, line, initialized_state, &cancel)
}

fn handle_line_with_cancel<S: ProtocolStateAccess>(
    core: &Core,
    broker: &Broker,
    db_path: Option<&PathBuf>,
    line: &str,
    initialized_state: &mut S,
    cancel: &AtomicBool,
) -> Option<String> {
    if line.trim().is_empty() {
        return None;
    }

    let parsed = match parse_request(line.trim()) {
        Ok(parsed) => parsed,
        Err(error) => {
            // A malformed object without an id is a notification-shaped
            // invalid request and must stay silent. Parse errors are the one
            // exception permitted to use a null id response.
            if error.always_respond || error.has_id {
                return Some(error_response(error.id, error.code, error.message));
            }
            return None;
        }
    };

    let ParsedRequest { request, has_id } = parsed;
    if !has_id {
        // MCP notifications are lifecycle signals. In particular, do not
        // dispatch arbitrary notification methods as metadata mutations.
        match request.method.as_str() {
            "notifications/initialized" => initialized_state.mark_initialized_notification(),
            "notifications/cancelled" => {}
            _ => {}
        }
        return None;
    }

    let id = request.id.clone().unwrap_or(Value::Null);
    if request.method == "initialize" {
        let result = initialize_result(&request.params);
        initialized_state.mark_initialized();
        return Some(result_response(id, result));
    }

    if !initialized_state.is_initialized() {
        return Some(error_response(id, -32600, "Server not initialized"));
    }

    Some(dispatch_with_cancel(
        core, broker, db_path, &request, cancel,
    ))
}

fn parse_request(line: &str) -> Result<ParsedRequest, EnvelopeError> {
    let value: Value = serde_json::from_str(line).map_err(|_| EnvelopeError {
        code: -32700,
        message: "Parse error",
        id: Value::Null,
        // Parse failures cannot be classified as notifications.
        has_id: true,
        always_respond: true,
    })?;

    let Some(object) = value.as_object() else {
        return Err(EnvelopeError {
            code: -32600,
            message: "Invalid Request",
            id: Value::Null,
            // A primitive/array has no notification envelope, so answer it.
            has_id: true,
            always_respond: false,
        });
    };

    let id = object.get("id").cloned();
    let has_id = id.is_some();
    let response_id = id.clone().unwrap_or(Value::Null);

    if object.get("jsonrpc") != Some(&Value::String("2.0".into())) {
        return Err(EnvelopeError {
            code: -32600,
            message: "Invalid Request",
            id: response_id,
            has_id,
            always_respond: false,
        });
    }

    let Some(method) = object.get("method").and_then(Value::as_str) else {
        return Err(EnvelopeError {
            code: -32600,
            message: "Invalid Request",
            id: response_id,
            has_id,
            always_respond: false,
        });
    };

    Ok(ParsedRequest {
        request: RpcRequest {
            id,
            method: method.to_string(),
            params: object.get("params").cloned().unwrap_or(Value::Null),
        },
        has_id,
    })
}

#[cfg(test)]
fn dispatch(
    core: &Core,
    broker: &Broker,
    db_path: Option<&PathBuf>,
    request: &RpcRequest,
) -> String {
    let cancel = AtomicBool::new(false);
    dispatch_with_cancel(core, broker, db_path, request, &cancel)
}

fn dispatch_with_cancel(
    core: &Core,
    broker: &Broker,
    db_path: Option<&PathBuf>,
    request: &RpcRequest,
    cancel: &AtomicBool,
) -> String {
    dispatch_request(core, broker, db_path, request, cancel)
}

fn dispatch_request(
    core: &Core,
    broker: &Broker,
    db_path: Option<&PathBuf>,
    request: &RpcRequest,
    cancel: &AtomicBool,
) -> String {
    let id = request.id.clone().unwrap_or(Value::Null);
    match request.method.as_str() {
        "initialize" => result_response(id, initialize_result(&request.params)),
        "tools/list" => result_response(id, json!({ "tools": tool_schemas() })),
        "tools/call" => match route(core, broker, db_path, &request.params, cancel) {
            Ok(result) => result_response(id, result),
            Err(RouteError::InvalidParams) => error_response(id, -32602, "Invalid params"),
            Err(RouteError::Tool(error)) => result_response(id, tool_error_result(error)),
        },
        _ => error_response(id, -32601, "Method not found"),
    }
}

fn initialize_result(params: &Value) -> Value {
    // MCP servers select a supported version. The current server remains
    // compatible with the known versions and chooses its current version for
    // unknown future client requests as allowed by the protocol.
    let requested_version = params
        .as_object()
        .and_then(|params| params.get("protocolVersion"))
        .and_then(Value::as_str);
    let protocol_version = match requested_version {
        Some(MCP_PROTOCOL_VERSION) | None => MCP_PROTOCOL_VERSION,
        Some(_) => MCP_PROTOCOL_VERSION,
    };
    json!({
        "protocolVersion": protocol_version,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "repoatlas-mcp", "version": env!("CARGO_PKG_VERSION") }
    })
}

enum RouteError {
    InvalidParams,
    Tool(String),
}

fn route(
    core: &Core,
    broker: &Broker,
    db_path: Option<&PathBuf>,
    p: &Value,
    cancel: &AtomicBool,
) -> Result<Value, RouteError> {
    let Some(params) = p.as_object() else {
        return Err(RouteError::InvalidParams);
    };
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return Err(RouteError::InvalidParams);
    };
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| Value::Object(Default::default()));
    if !args.is_object() {
        return Err(RouteError::InvalidParams);
    }
    call_with_cancel(core, broker, db_path, name, &args, cancel)
        .map(|result| json!({ "content": [ { "type": "text", "text": result } ] }))
        .map_err(RouteError::Tool)
}

#[cfg(test)]
fn call(
    core: &Core,
    broker: &Broker,
    db_path: Option<&PathBuf>,
    name: &str,
    args: &Value,
) -> Result<String, String> {
    let cancel = AtomicBool::new(false);
    call_with_cancel(core, broker, db_path, name, args, &cancel)
}

fn call_with_cancel(
    core: &Core,
    _broker: &Broker,
    _db_path: Option<&PathBuf>,
    name: &str,
    args: &Value,
    cancel: &AtomicBool,
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
                    collection_id: optional_string(args, "collectionId")?,
                    limit: Some(500),
                })
                .map_err(|e| e.to_string())?;
            Ok(serde_json::to_string_pretty(&projects).unwrap())
        }
        "list_collections" => Ok(serde_json::to_string_pretty(
            &core.list_collections().map_err(|error| error.to_string())?,
        )
        .unwrap()),
        "create_collection" => {
            let collection = core
                .create_collection_with_origin(
                    CollectionUpsert {
                        name: required_string(args, "name")?.to_string(),
                        description: optional_nullable_string(args, "description")?,
                    },
                    "mcp",
                )
                .map_err(|error| error.to_string())?;
            Ok(serde_json::to_string_pretty(&collection).unwrap())
        }
        "update_collection" => {
            let id = required_string(args, "id")?;
            let collection = core
                .update_collection_with_origin(
                    id,
                    CollectionUpsert {
                        name: required_string(args, "name")?.to_string(),
                        description: optional_nullable_string(args, "description")?,
                    },
                    "mcp",
                )
                .map_err(|error| error.to_string())?;
            Ok(serde_json::to_string_pretty(&collection).unwrap())
        }
        "delete_collection" => {
            let id = required_string(args, "id")?;
            core.delete_collection_with_origin(id, "mcp")
                .map_err(|error| error.to_string())?;
            Ok(json!({ "deleted": true, "id": id, "filesystemDeleted": false }).to_string())
        }
        "set_collection_members" => {
            let id = required_string(args, "id")?;
            let project_ids = string_array(args, "projectIds")?
                .ok_or_else(|| "missing required array argument: projectIds".to_string())?;
            let collection = core
                .set_collection_members_with_origin(id, &project_ids, "mcp")
                .map_err(|error| error.to_string())?;
            Ok(serde_json::to_string_pretty(&collection).unwrap())
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
        "get_project_brief" => {
            let project_id = required_string(args, "projectId")?;
            let brief = core
                .get_project_brief(project_id)
                .map_err(|error| error.to_string())?;
            Ok(serde_json::to_string_pretty(&brief).unwrap())
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
                expected_ports: port_array(args, "expectedPorts")?.unwrap_or_default(),
                dev_url_path: optional_nullable_string(args, "devUrlPath")?,
                dev_url_scheme: optional_nullable_string(args, "devUrlScheme")?,
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
                expected_ports: port_array(args, "expectedPorts")?
                    .unwrap_or_else(|| existing.expected_ports.clone()),
                dev_url_path: if args.get("devUrlPath").is_some() {
                    optional_nullable_string(args, "devUrlPath")?
                } else {
                    existing.dev_url_path.clone()
                },
                dev_url_scheme: if args.get("devUrlScheme").is_some() {
                    optional_nullable_string(args, "devUrlScheme")?
                } else {
                    existing.dev_url_scheme.clone()
                },
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
                .scan_root_with_origin(&root_id, cancel, &|_| {}, "mcp")
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
        "list_audit_events" => {
            let limit = args
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(100)
                .clamp(1, 500) as usize;
            let events = core.list_audit_events(limit).map_err(|e| e.to_string())?;
            Ok(serde_json::to_string_pretty(&events).unwrap())
        }
        "list_attention_items" => {
            let items = core
                .list_attention_items()
                .map_err(|error| error.to_string())?;
            Ok(serde_json::to_string_pretty(&items).unwrap())
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

fn port_array(args: &Value, key: &str) -> Result<Option<Vec<u16>>, String> {
    let Some(value) = args.get(key) else {
        return Ok(None);
    };
    let values = value
        .as_array()
        .ok_or_else(|| format!("{key} must be an array of ports"))?;
    let mut ports = Vec::with_capacity(values.len());
    for value in values {
        let port = value
            .as_u64()
            .filter(|port| (1..=u16::MAX as u64).contains(port))
            .ok_or_else(|| format!("{key} must contain ports between 1 and 65535"))?;
        ports.push(port as u16);
    }
    ports.sort_unstable();
    ports.dedup();
    Ok(Some(ports))
}

fn result_response(id: Value, result: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
}

fn error_response(id: Value, code: i32, message: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
    .to_string()
}

fn tool_error_result(error: String) -> Value {
    json!({
        "content": [{ "type": "text", "text": error }],
        "isError": true
    })
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
        for unavailable in [
            "acknowledge_attention_item",
            "list_provider_profiles",
            "upsert_provider_profile",
            "delete_provider_profile",
            "list_memory",
            "add_memory",
            "delete_memory",
            "list_summaries",
            "get_summary",
            "accept_summary_memory",
            "conversation_summary",
            "list_conversation",
            "clear_conversation",
        ] {
            assert!(
                tools.iter().all(|tool| tool["name"] != unavailable),
                "retired tool {unavailable} must stay unavailable"
            );
        }
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
            "list_project_events",
            "atlas_report",
            "remove_scan_root",
            "git_status",
            "git_diff",
        ] {
            let tool = tools.iter().find(|tool| tool["name"] == name).expect(name);
            assert!(
                tool["inputSchema"]["required"].as_array().is_some(),
                "{name}"
            );
        }
        assert!(
            !tools.iter().any(|tool| tool["name"] == "read_project_file"),
            "desktop-only project file reading must not be exposed through MCP"
        );
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
    fn non_json_line_returns_parse_error() {
        let core = Core::open_in_memory().expect("core");
        let broker =
            Broker::new(std::env::temp_dir().join("repoatlas-mcp-protocol-logs")).expect("broker");
        let mut state = ProtocolState::default();
        let response = handle_line(&core, &broker, None, "this is not json\n", &mut state)
            .expect("parse error response");
        let response: Value = serde_json::from_str(&response).expect("response json");
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], Value::Null);
        assert_eq!(response["error"]["code"], -32700);
        assert_eq!(response["error"]["message"], "Parse error");
    }

    #[test]
    fn invalid_jsonrpc_version_returns_invalid_request() {
        let core = Core::open_in_memory().expect("core");
        let broker =
            Broker::new(std::env::temp_dir().join("repoatlas-mcp-protocol-logs")).expect("broker");
        let mut state = ProtocolState::default();
        let response = handle_line(
            &core,
            &broker,
            None,
            r#"{"jsonrpc":"1.0","id":4,"method":"tools/list"}"#,
            &mut state,
        )
        .expect("invalid request response");
        let response: Value = serde_json::from_str(&response).expect("response json");
        assert_eq!(response["id"], 4);
        assert_eq!(response["error"]["code"], -32600);
        assert_eq!(response["error"]["message"], "Invalid Request");
    }

    #[test]
    fn non_object_json_returns_invalid_request() {
        let core = Core::open_in_memory().expect("core");
        let broker =
            Broker::new(std::env::temp_dir().join("repoatlas-mcp-protocol-logs")).expect("broker");
        let mut state = ProtocolState::default();
        let response =
            handle_line(&core, &broker, None, "[]", &mut state).expect("invalid request response");
        let response: Value = serde_json::from_str(&response).expect("response json");
        assert_eq!(response["id"], Value::Null);
        assert_eq!(response["error"]["code"], -32600);
    }

    #[test]
    fn null_id_is_a_request_not_a_notification() {
        let core = Core::open_in_memory().expect("core");
        let broker =
            Broker::new(std::env::temp_dir().join("repoatlas-mcp-protocol-logs")).expect("broker");
        let mut state = ProtocolState::default();
        let response = handle_line(
            &core,
            &broker,
            None,
            r#"{"jsonrpc":"2.0","id":null,"method":"initialize"}"#,
            &mut state,
        )
        .expect("initialize response");
        let response: Value = serde_json::from_str(&response).expect("response json");
        assert_eq!(response["id"], Value::Null);
        assert_eq!(response["result"]["protocolVersion"], MCP_PROTOCOL_VERSION);
        assert!(state.initialized);
    }

    #[test]
    fn tools_call_without_name_is_invalid_params() {
        let core = Core::open_in_memory().expect("core");
        let broker =
            Broker::new(std::env::temp_dir().join("repoatlas-mcp-protocol-logs")).expect("broker");
        let mut state = ProtocolState {
            initialized: true,
            ..ProtocolState::default()
        };
        let response = handle_line(
            &core,
            &broker,
            None,
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{}}"#,
            &mut state,
        )
        .expect("invalid params response");
        let response: Value = serde_json::from_str(&response).expect("response json");
        assert_eq!(response["error"]["code"], -32602);
        assert_eq!(response["error"]["message"], "Invalid params");
        assert!(response.get("result").is_none());
    }

    #[test]
    fn unknown_method_returns_method_not_found_after_initialize() {
        let core = Core::open_in_memory().expect("core");
        let broker =
            Broker::new(std::env::temp_dir().join("repoatlas-mcp-protocol-logs")).expect("broker");
        let mut state = ProtocolState::default();
        let initialize = handle_line(
            &core,
            &broker,
            None,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#,
            &mut state,
        )
        .expect("initialize response");
        let initialize: Value = serde_json::from_str(&initialize).expect("initialize json");
        assert_eq!(
            initialize["result"]["protocolVersion"],
            MCP_PROTOCOL_VERSION
        );
        assert!(state.initialized);

        let response = handle_line(
            &core,
            &broker,
            None,
            r#"{"jsonrpc":"2.0","id":2,"method":"does/not-exist"}"#,
            &mut state,
        )
        .expect("method error response");
        let response: Value = serde_json::from_str(&response).expect("response json");
        assert_eq!(response["error"]["code"], -32601);
        assert_eq!(response["error"]["message"], "Method not found");
    }

    #[test]
    fn unknown_tool_is_a_call_tool_error_result() {
        let core = Core::open_in_memory().expect("core");
        let broker =
            Broker::new(std::env::temp_dir().join("repoatlas-mcp-protocol-logs")).expect("broker");
        let mut state = ProtocolState {
            initialized: true,
            ..ProtocolState::default()
        };
        let response = handle_line(
            &core,
            &broker,
            None,
            r#"{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"not_a_tool","arguments":{}}}"#,
            &mut state,
        )
        .expect("tool response");
        let response: Value = serde_json::from_str(&response).expect("response json");
        assert_eq!(response["result"]["isError"], true);
        assert!(response["result"]["content"][0]["text"]
            .as_str()
            .expect("tool error text")
            .contains("unknown tool"));
        assert!(response.get("error").is_none());
    }

    #[test]
    fn requests_before_initialize_are_rejected_but_initialize_succeeds() {
        let core = Core::open_in_memory().expect("core");
        let broker =
            Broker::new(std::env::temp_dir().join("repoatlas-mcp-protocol-logs")).expect("broker");
        let mut state = ProtocolState::default();
        let response = handle_line(
            &core,
            &broker,
            None,
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
            &mut state,
        )
        .expect("not initialized response");
        let response: Value = serde_json::from_str(&response).expect("response json");
        assert_eq!(response["error"]["code"], -32600);
        assert_eq!(response["error"]["message"], "Server not initialized");

        let response = handle_line(
            &core,
            &broker,
            None,
            r#"{"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"future"}}"#,
            &mut state,
        )
        .expect("initialize response");
        let response: Value = serde_json::from_str(&response).expect("response json");
        assert_eq!(response["result"]["protocolVersion"], MCP_PROTOCOL_VERSION);
        assert!(state.initialized);
    }

    #[test]
    fn missing_id_notification_has_no_response_and_initialized_is_recorded() {
        let core = Core::open_in_memory().expect("core");
        let broker =
            Broker::new(std::env::temp_dir().join("repoatlas-mcp-protocol-logs")).expect("broker");
        let mut state = ProtocolState::default();
        assert!(handle_line(
            &core,
            &broker,
            None,
            r#"{"jsonrpc":"2.0","method":"tools/list"}"#,
            &mut state,
        )
        .is_none());
        assert!(!state.initialized);

        assert!(handle_line(
            &core,
            &broker,
            None,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            &mut state,
        )
        .is_none());
        assert!(state.initialized_notification);
        assert!(!state.initialized);
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
                    expected_ports: Vec::new(),
                    dev_url_path: None,
                    dev_url_scheme: None,
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

    #[test]
    fn canceled_scan_request_stops_cooperatively() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root_path = std::env::temp_dir().join(format!("repoatlas-mcp-scan-{nonce}"));
        std::fs::create_dir_all(&root_path).expect("scan root");
        let core = Core::open_in_memory().expect("core");
        let root = core.add_scan_root(&root_path).expect("add root");
        let broker = Broker::new(std::env::temp_dir().join(format!("repoatlas-mcp-logs-{nonce}")))
            .expect("broker");
        let cancel = AtomicBool::new(true);

        let response = call_with_cancel(
            &core,
            &broker,
            None,
            "scan_root",
            &json!({"rootId": root.id}),
            &cancel,
        )
        .expect("scan response");
        let response: Value = serde_json::from_str(&response).expect("json");
        assert_eq!(response["cancelled"], true);
        let _ = std::fs::remove_dir_all(root_path);
    }
}
