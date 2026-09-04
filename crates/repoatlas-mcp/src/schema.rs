use serde_json::{json, Value};

pub(crate) fn tool_schemas() -> Vec<Value> {
    let task_properties = json!({
        "projectId": { "type": "string", "minLength": 1 },
        "id": { "type": "string" },
        "kind": { "type": "string" },
        "name": { "type": "string", "minLength": 1 },
        "description": { "type": ["string", "null"] },
        "executable": { "type": "string", "minLength": 1 },
        "argv": { "type": "array", "items": { "type": "string" } },
        "cwd": { "type": ["string", "null"] },
        "inferred": { "type": "boolean" },
        "shellMode": { "type": "boolean" },
        "expectedPorts": { "type": "array", "items": { "type": "integer", "minimum": 1, "maximum": 65535 }, "uniqueItems": true },
        "devUrlPath": { "type": ["string", "null"] },
        "devUrlScheme": { "type": ["string", "null"], "enum": ["http", "https", null] }
    });
    vec![
        tool_schema(
            "list_projects",
            "List managed projects with optional scope and search filters.",
            object_schema(json!({
                "section": { "type": "string", "enum": ["projects", "favorites", "recent", "archived"] },
                "search": { "type": "string" },
                "collectionId": { "type": "string", "minLength": 1 }
            }), &[]),
        ),
        tool_schema(
            "list_collections",
            "List user-defined project collections and their project counts.",
            object_schema(json!({}), &[]),
        ),
        tool_schema(
            "create_collection",
            "Create a project collection. This only changes RepoAtlas metadata.",
            object_schema(json!({
                "name": { "type": "string", "minLength": 1, "maxLength": 80 },
                "description": { "type": ["string", "null"], "maxLength": 500 }
            }), &["name"]),
        ),
        tool_schema(
            "update_collection",
            "Rename or describe a project collection.",
            object_schema(json!({
                "id": { "type": "string", "minLength": 1 },
                "name": { "type": "string", "minLength": 1, "maxLength": 80 },
                "description": { "type": ["string", "null"], "maxLength": 500 }
            }), &["id", "name"]),
        ),
        tool_schema(
            "delete_collection",
            "Delete a collection record. Projects and their directories are unchanged.",
            object_schema(json!({ "id": { "type": "string", "minLength": 1 } }), &["id"]),
        ),
        tool_schema(
            "set_collection_members",
            "Replace a collection's project membership. Project records and directories are unchanged.",
            object_schema(json!({
                "id": { "type": "string", "minLength": 1 },
                "projectIds": { "type": "array", "items": { "type": "string", "minLength": 1 }, "uniqueItems": true }
            }), &["id", "projectIds"]),
        ),
        tool_schema(
            "search_projects",
            "Search projects by name, path, stack, description, tags, and notes.",
            object_schema(json!({ "query": { "type": "string", "minLength": 1 } }), &["query"]),
        ),
        tool_schema(
            "get_project",
            "Get project detail including path, detected technology, README excerpt, tasks, environment, lineage, events, and Git snapshot.",
            object_schema(json!({ "id": { "type": "string", "minLength": 1 } }), &["id"]),
        ),
        tool_schema(
            "get_project_brief",
            "Get the structured local project brief, including detected facts, environment, tasks, recent runs, and activity.",
            object_schema(json!({ "projectId": { "type": "string", "minLength": 1 } }), &["projectId"]),
        ),
        tool_schema(
            "register_project",
            "Register one local directory as a managed project without changing files in that directory.",
            object_schema(json!({ "path": { "type": "string", "minLength": 1 } }), &["path"]),
        ),
        tool_schema(
            "update_project",
            "Update user-maintained project metadata. Null description clears it; tasks replaces the complete saved task list.",
            object_schema(
                json!({
                    "id": { "type": "string", "minLength": 1 },
                    "displayName": { "type": "string", "minLength": 1 },
                    "description": { "type": ["string", "null"] },
                    "notes": { "type": ["string", "null"] },
                    "favorite": { "type": "boolean" },
                    "archived": { "type": "boolean" },
                    "tags": { "type": "array", "items": { "type": "string" } },
                    "tasks": { "type": "array", "items": { "type": "object" } }
                }),
                &["id"],
            ),
        ),
        tool_schema(
            "remove_project",
            "Remove one managed project and its RepoAtlas records. Requires confirm=true. Never deletes, moves, or edits the project directory.",
            object_schema(
                json!({
                    "projectId": { "type": "string", "minLength": 1 },
                    "confirm": { "const": true }
                }),
                &["projectId", "confirm"],
            ),
        ),
        tool_schema(
            "relocate_project",
            "Relocate a managed project record to an existing directory. Never moves or deletes either directory.",
            object_schema(
                json!({
                    "projectId": { "type": "string", "minLength": 1 },
                    "path": { "type": "string", "minLength": 1 }
                }),
                &["projectId", "path"],
            ),
        ),
        tool_schema(
            "refresh_project",
            "Refresh one managed project from its current directory.",
            object_schema(json!({ "projectId": { "type": "string", "minLength": 1 } }), &["projectId"]),
        ),
        tool_schema(
            "set_project_icon",
            "Set a custom project icon from one explicit local PNG, JPEG, WebP, or ICO file. RepoAtlas validates and copies it without modifying the project directory.",
            object_schema(
                json!({
                    "projectId": { "type": "string", "minLength": 1 },
                    "path": { "type": "string", "minLength": 1 }
                }),
                &["projectId", "path"],
            ),
        ),
        tool_schema(
            "clear_project_icon",
            "Clear a custom project icon and return to automatic project or language icon detection.",
            object_schema(json!({ "projectId": { "type": "string", "minLength": 1 } }), &["projectId"]),
        ),
        tool_schema(
            "read_project_icons",
            "Read metadata for a batch of project icons. MCP returns no image payload; use the desktop app for rendering.",
            object_schema(
                json!({
                    "projectIds": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": 48,
                        "items": { "type": "string", "minLength": 1 }
                    }
                }),
                &["projectIds"],
            ),
        ),
        tool_schema(
            "add_task",
            "Add or replace one runnable task in a project. argv is always an array of complete argument strings.",
            object_schema(
                task_properties.clone(),
                &["projectId", "name", "executable"],
            ),
        ),
        tool_schema(
            "update_task",
            "Update one saved task by id while preserving fields omitted from the request. argv is an array of complete argument strings.",
            object_schema(
                json!({
                    "projectId": { "type": "string", "minLength": 1 },
                    "taskId": { "type": "string", "minLength": 1 },
                    "kind": { "type": "string" },
                    "name": { "type": "string", "minLength": 1 },
                    "description": { "type": ["string", "null"] },
                    "executable": { "type": "string", "minLength": 1 },
                    "argv": { "type": "array", "items": { "type": "string" } },
                    "cwd": { "type": ["string", "null"] },
                    "shellMode": { "type": "boolean" },
                    "expectedPorts": { "type": "array", "items": { "type": "integer", "minimum": 1, "maximum": 65535 }, "uniqueItems": true },
                    "devUrlPath": { "type": ["string", "null"] },
                    "devUrlScheme": { "type": ["string", "null"], "enum": ["http", "https", null] }
                }),
                &["projectId", "taskId"],
            ),
        ),
        tool_schema(
            "remove_task",
            "Remove one saved task from a project. This only changes RepoAtlas task metadata.",
            object_schema(
                json!({
                    "projectId": { "type": "string", "minLength": 1 },
                    "taskId": { "type": "string", "minLength": 1 }
                }),
                &["projectId", "taskId"],
            ),
        ),
        tool_schema(
            "list_task_runs",
            "List recent task runs for a project.",
            object_schema(json!({ "projectId": { "type": "string", "minLength": 1 } }), &["projectId"]),
        ),
        tool_schema(
            "run_task",
            "Request desktop approval to start one existing project task. MCP never starts the task directly.",
            object_schema(
                json!({
                    "projectId": { "type": "string", "minLength": 1 },
                    "taskId": { "type": "string" },
                    "name": { "type": "string" }
                }),
                &["projectId"],
            ),
        ),
        tool_schema(
            "get_task_run",
            "Get one task run by id.",
            object_schema(json!({ "runId": { "type": "string", "minLength": 1 } }), &["runId"]),
        ),
        tool_schema(
            "read_task_log",
            "Read the captured log for one task run.",
            object_schema(json!({ "runId": { "type": "string", "minLength": 1 } }), &["runId"]),
        ),
        tool_schema(
            "scan_root",
            "Scan one already authorized scan root. Scanning remains a manual refresh.",
            object_schema(json!({ "rootId": { "type": "string", "minLength": 1 } }), &["rootId"]),
        ),
        tool_schema(
            "add_scan_root",
            "Authorize one local directory as a scan root. This changes RepoAtlas authorization records only.",
            object_schema(json!({ "path": { "type": "string", "minLength": 1 } }), &["path"]),
        ),
        tool_schema(
            "remove_scan_root",
            "Remove a scan-root authorization. orphan preserves project records; removeRecords also removes managed project records. Never deletes the directory.",
            object_schema(
                json!({
                    "rootId": { "type": "string", "minLength": 1 },
                    "recordPolicy": { "type": "string", "enum": ["orphan", "removeRecords"] },
                    "confirm": { "const": true }
                }),
                &["rootId", "recordPolicy", "confirm"],
            ),
        ),
        tool_schema(
            "list_scan_roots",
            "List explicitly authorized scan roots.",
            object_schema(json!({}), &[]),
        ),
        tool_schema(
            "inspect_project_environment",
            "Inspect project runtime requirements, local versions, match states, and detected evidence files.",
            object_schema(json!({ "projectId": { "type": "string", "minLength": 1 } }), &["projectId"]),
        ),
        tool_schema(
            "list_project_events",
            "List system-generated activity events for a project. Events are read-only.",
            object_schema(
                json!({
                    "projectId": { "type": "string", "minLength": 1 },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 100 }
                }),
                &["projectId"],
            ),
        ),
        tool_schema(
            "atlas_report",
            "Build a local Markdown Atlas Report from existing project facts. Does not make an AI request.",
            object_schema(json!({ "projectId": { "type": "string", "minLength": 1 } }), &["projectId"]),
        ),
        tool_schema(
            "list_audit_events",
            "List RepoAtlas audit events. Audit records are system-generated and read-only.",
            object_schema(
                json!({ "limit": { "type": "integer", "minimum": 1, "maximum": 500 } }),
                &[],
            ),
        ),
        tool_schema(
            "list_attention_items",
            "List unresolved approvals, failed runs, unavailable projects, environment mismatches, and system failures.",
            object_schema(json!({}), &[]),
        ),
        tool_schema(
            "git_status",
            "Read the current Git status for a managed project. Read-only.",
            object_schema(json!({ "projectId": { "type": "string", "minLength": 1 } }), &["projectId"]),
        ),
        tool_schema(
            "git_diff",
            "Read a Git diff for a managed project. Read-only.",
            object_schema(
                json!({
                    "projectId": { "type": "string", "minLength": 1 },
                    "path": { "type": ["string", "null"] },
                    "staged": { "type": "boolean" }
                }),
                &["projectId"],
            ),
        ),
    ]
}

fn tool_schema(name: &str, description: &str, input_schema: Value) -> Value {
    json!({ "name": name, "description": description, "inputSchema": input_schema })
}

fn object_schema(properties: Value, required: &[&str]) -> Value {
    let mut schema = json!({
        "type": "object",
        "additionalProperties": false,
        "properties": properties
    });
    if !required.is_empty() {
        schema["required"] = json!(required);
    }
    schema
}
