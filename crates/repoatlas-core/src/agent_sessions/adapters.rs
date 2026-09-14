use super::*;
use crate::{paths, Error, Result};
use serde_json::Value;
use std::{
    fs,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

pub const ADAPTERS: &[&str] = &[
    "claude",
    "codex",
    "opencode",
    "cursor-cli",
    "gemini",
    "copilot",
    "kimi",
    "qwen",
];
pub trait SessionAdapter {
    fn id(&self) -> &str;
    fn accepts(&self, path: &Path) -> bool;
    fn read(
        &self,
        root: &Path,
        path: &Path,
        cancel: &AtomicBool,
    ) -> Result<Vec<(AgentSession, Vec<SessionMessage>)>>;
}
pub struct LocalAdapter(pub String);
pub fn candidate_sources() -> Vec<SessionSource> {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_default();
    let base = |env: &str, fallback: &str| {
        std::env::var_os(env)
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(fallback))
    };
    let codex = base("CODEX_HOME", ".codex");
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"));
    [
        (
            "claude",
            base("CLAUDE_CONFIG_DIR", ".claude").join("projects"),
        ),
        ("codex", codex.join("sessions")),
        ("codex", codex.join("archived_sessions")),
        ("opencode", data.join("opencode")),
        (
            "cursor-cli",
            base("CURSOR_CONFIG_DIR", ".cursor").join("chats"),
        ),
        ("gemini", base("GEMINI_CLI_HOME", "").join(".gemini/tmp")),
        (
            "copilot",
            base("COPILOT_HOME", ".copilot").join("session-state"),
        ),
        ("kimi", base("KIMI_CODE_HOME", ".kimi-code")),
        ("kimi", home.join(".kimi")),
        ("qwen", base("QWEN_HOME", ".qwen").join("projects")),
        ("qwen", base("QWEN_HOME", ".qwen").join("tmp")),
    ]
    .into_iter()
    .filter(|(_, p)| p.is_dir())
    .map(|(a, p)| SessionSource {
        id: String::new(),
        adapter: a.into(),
        path: paths::path_to_string(&p),
        enabled: false,
        last_scanned_at: None,
        last_error: None,
    })
    .collect()
}

/// Preserve the Agent data home when resuming from an explicitly selected source.
/// Unknown copied layouts remain searchable but must not accidentally resume an
/// unrelated ID from the Agent's default home.
pub fn resume_environment(
    source: &SessionSource,
) -> Result<std::collections::BTreeMap<String, String>> {
    let path = Path::new(&source.path);
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    let parent = path
        .parent()
        .ok_or_else(|| Error::msg("session_source_layout_unknown"))?;
    let (key, root) = match source.adapter.as_str() {
        "claude" if name == "projects" => ("CLAUDE_CONFIG_DIR", parent),
        "codex" if ["sessions", "archived_sessions"].contains(&name) => ("CODEX_HOME", parent),
        "qwen" if ["projects", "tmp"].contains(&name) => ("QWEN_HOME", parent),
        "cursor-cli" if name == "chats" => ("CURSOR_CONFIG_DIR", parent),
        "copilot" if name == "session-state" => ("COPILOT_HOME", parent),
        "opencode" if name == "opencode" => ("XDG_DATA_HOME", parent),
        "gemini" if name == "tmp" && parent.file_name().is_some_and(|p| p == ".gemini") => (
            "GEMINI_CLI_HOME",
            parent
                .parent()
                .ok_or_else(|| Error::msg("session_source_layout_unknown"))?,
        ),
        "kimi" => ("KIMI_CODE_HOME", path),
        _ => return Err(Error::msg("session_source_layout_unknown")),
    };
    let mut env = std::collections::BTreeMap::new();
    env.insert(key.into(), paths::path_to_string(root));
    if source.adapter == "kimi" {
        env.insert("KIMI_SHARE_DIR".into(), paths::path_to_string(root));
    }
    Ok(env)
}

/// Revalidate the selected ID against the authorized provider file immediately
/// before dispatch. A cached row is not proof that the provider can still resume.
pub fn validate_resume_source(source: &SessionSource, session: &AgentSession) -> Result<()> {
    if !source.enabled || source.id != session.source_id || source.adapter != session.adapter {
        return Err(Error::msg("source_disabled"));
    }
    let root = paths::canonicalize(Path::new(&source.path))?;
    let file = paths::canonicalize(Path::new(&session.source_locator))
        .map_err(|_| Error::msg("session_source_missing"))?;
    if !paths::is_within(&file, &root) {
        return Err(Error::msg("source_path_escape"));
    }
    let found = LocalAdapter(source.adapter.clone())
        .read(&root, &file, &AtomicBool::new(false))?
        .into_iter()
        .any(|(current, _)| current.external_id == session.external_id);
    if !found {
        return Err(Error::msg("session_source_missing"));
    }
    Ok(())
}

/// Only traverse approved roots, never links or credential/generated directories.
pub fn enumerate(source: &SessionSource, cancel: &AtomicBool) -> Result<Vec<PathBuf>> {
    let adapter = LocalAdapter(source.adapter.clone());
    let root = paths::canonicalize(Path::new(&source.path))?;
    let mut pending = vec![(root.clone(), 0)];
    let mut files = Vec::new();
    let mut visited = 0;
    while let Some((dir, depth)) = pending.pop() {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::msg("canceled"));
        }
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            visited += 1;
            if visited > 200_000 {
                return Err(Error::msg("source_entry_limit"));
            }
            let ty = entry.file_type()?;
            let path = entry.path();
            if ty.is_symlink() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if ty.is_dir() {
                if depth >= 12 {
                    return Err(Error::msg("source_depth_limit"));
                }
                if [
                    "credentials",
                    "logs",
                    "node_modules",
                    ".git",
                    "skills",
                    "plugins",
                    "tasks",
                    "plans",
                    "bin",
                    "cache",
                    "updates",
                ]
                .contains(&name.as_str())
                {
                    continue;
                }
                if source.adapter == "kimi" && depth == 0 && name != "sessions" {
                    continue;
                }
                if name == "subagents" || (source.adapter == "kimi" && name.starts_with("agent-")) {
                    continue;
                }
                if paths::is_within(&paths::canonicalize(&path)?, &root) {
                    pending.push((path, depth + 1));
                }
            } else if ty.is_file() && adapter.accepts(&path) {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

pub fn fingerprint(path: &Path) -> Result<String> {
    let m = fs::metadata(path)?;
    let mut h: u64 = 14695981039346656037;
    let mut f = fs::File::open(path)?;
    let mut buffer = [0; 4096];
    let n = f.read(&mut buffer)?;
    for b in &buffer[..n] {
        h = (h ^ *b as u64).wrapping_mul(1099511628211);
    }
    let mut result = format!("parser3:{}:{:?}:{h}", m.len(), m.modified()?);
    let wal = PathBuf::from(format!("{}-wal", path.display()));
    if let Ok(w) = fs::metadata(wal) {
        result.push_str(&format!(":{}:{:?}", w.len(), w.modified()?));
    }
    if let Some(parent) = path.parent() {
        for name in ["meta.json", "state.json", "metadata.json"] {
            if let Ok(m) = fs::metadata(parent.join(name)) {
                result.push_str(&format!(":{}:{:?}", m.len(), m.modified()?));
            }
        }
    }
    Ok(result)
}
fn text(v: &Value) -> String {
    if let Some(s) = v.as_str() {
        return s.to_owned();
    }
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|p| {
                    let kind = p.get("type").and_then(Value::as_str).unwrap_or("text");
                    if p.get("thought").and_then(Value::as_bool) != Some(true)
                        && ["text", "input_text", "output_text"].contains(&kind)
                    {
                        p.get("text").and_then(Value::as_str)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}
fn s(v: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|k| v.get(k).and_then(Value::as_str))
        .unwrap_or_default()
        .to_owned()
}
fn timestamp(v: &Value, keys: &[&str]) -> String {
    for key in keys {
        if let Some(x) = v.get(key) {
            if let Some(t) = x.as_str() {
                if let Ok(d) = chrono::DateTime::parse_from_rfc3339(t) {
                    return d.to_utc().to_rfc3339();
                }
            }
            if let Some(n) = x.as_i64() {
                if let Some(d) = chrono::DateTime::from_timestamp_millis(if n < 10_000_000_000 {
                    n * 1000
                } else {
                    n
                }) {
                    return d.to_rfc3339();
                }
            }
        }
    }
    String::new()
}
fn metadata(session: &mut AgentSession, v: &Value) {
    let id = s(v, &["sessionId", "session_id"]);
    if !id.is_empty() {
        session.external_id = id;
    }
    let cwd = s(v, &["cwd", "directory", "workDir", "workingDirectory"]);
    if !cwd.is_empty() {
        session.cwd = cwd;
    }
    let title = s(v, &["customTitle", "title", "summary", "name"]);
    if !title.is_empty() {
        session.title = title.chars().take(240).collect();
    }
    let started = timestamp(v, &["startTime", "createdAt", "created_at", "timestamp"]);
    let updated = timestamp(v, &["lastUpdated", "updatedAt", "updated_at", "timestamp"]);
    if session.started_at.is_empty() && !started.is_empty() {
        session.started_at = started;
    }
    if !updated.is_empty() && updated > session.updated_at {
        session.updated_at = updated;
    }
    if v.get("archived").and_then(Value::as_bool) == Some(true) {
        session.archived = true;
    }
}
fn push(messages: &mut Vec<SessionMessage>, role: &str, content: String, time: String) {
    if !["user", "assistant"].contains(&role) || content.trim().is_empty() {
        return;
    }
    messages.push(SessionMessage {
        index: messages.len(),
        role: role.into(),
        content,
        timestamp: time,
    });
}

impl SessionAdapter for LocalAdapter {
    fn id(&self) -> &str {
        &self.0
    }
    fn accepts(&self, path: &Path) -> bool {
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        match self.id() {
            "claude" => name.ends_with(".jsonl") && !name.starts_with("agent-"),
            "codex" => name.starts_with("rollout-") && name.ends_with(".jsonl"),
            "qwen" => {
                name.ends_with(".jsonl") && path.components().any(|c| c.as_os_str() == "chats")
            }
            "gemini" => {
                name.starts_with("session-")
                    && (name.ends_with(".json") || name.ends_with(".jsonl"))
            }
            "copilot" => name == "events.jsonl",
            "kimi" => {
                name == "context.jsonl"
                    || (name == "wire.jsonl"
                        && path
                            .parent()
                            .and_then(Path::file_name)
                            .is_some_and(|n| n == "main"))
            }
            "opencode" => name == "opencode.db",
            "cursor-cli" => name == "store.db",
            _ => false,
        }
    }
    fn read(
        &self,
        root: &Path,
        path: &Path,
        cancel: &AtomicBool,
    ) -> Result<Vec<(AgentSession, Vec<SessionMessage>)>> {
        let path = paths::canonicalize(path)?;
        if !paths::is_within(&path, &paths::canonicalize(root)?) {
            return Err(Error::msg("source_path_escape"));
        }
        if self.id() == "opencode" {
            return read_opencode(&path, cancel);
        }
        if self.id() == "cursor-cli" {
            return read_cursor(&path, cancel);
        }
        let mut session = AgentSession {
            adapter: self.id().into(),
            source_locator: paths::path_to_string(&path),
            ..Default::default()
        };
        session.archived = path.components().any(|c| {
            matches!(
                c.as_os_str().to_str(),
                Some("archive" | "archived_sessions")
            )
        });
        let mut messages = Vec::new();
        let mut legacy_messages = Vec::new();
        if self.id() == "gemini" {
            let records = read_gemini(&path, cancel)?;
            metadata(&mut session, &records.0);
            for m in &records.1 {
                let role = s(m, &["type", "role"]);
                push(
                    &mut messages,
                    if role == "gemini" { "assistant" } else { &role },
                    text(&m["content"]),
                    timestamp(m, &["timestamp"]),
                );
            }
            if session.cwd.is_empty() {
                if let Some(parent) = path.parent().and_then(Path::parent) {
                    let p = parent.join(".project_root");
                    if p.is_file() && paths::is_within(&paths::canonicalize(&p)?, root) {
                        session.cwd = fs::read_to_string(p)?.trim().to_owned();
                    }
                }
            }
        } else {
            let mut reader = BufReader::new(fs::File::open(&path)?);
            let mut line = Vec::new();
            let mut bad = 0;
            loop {
                if cancel.load(Ordering::Relaxed) {
                    return Err(Error::msg("canceled"));
                }
                line.clear();
                let n = reader
                    .by_ref()
                    .take(8 * 1024 * 1024 + 1)
                    .read_until(b'\n', &mut line)?;
                if n == 0 {
                    break;
                }
                if n > 8 * 1024 * 1024 {
                    return Err(Error::msg("session_line_limit"));
                }
                let v: Value = match serde_json::from_slice(&line) {
                    Ok(v) => v,
                    Err(_) if !line.ends_with(b"\n") => break,
                    Err(_) => {
                        bad += 1;
                        continue;
                    }
                };
                let kind = s(&v, &["type"]);
                metadata(&mut session, &v);
                if v["isSidechain"] == true || v["isMeta"] == true {
                    continue;
                }
                match self.id() {
                    "codex" => {
                        let p = &v["payload"];
                        if kind == "session_meta" {
                            metadata(&mut session, p);
                            session.external_id = s(p, &["id"]);
                        }
                        if kind == "turn_context" {
                            // `summary` here is a reasoning configuration (e.g. "auto"), not a title.
                            let cwd = s(p, &["cwd"]);
                            if !cwd.is_empty() {
                                session.cwd = cwd;
                            }
                        }
                        if kind == "event_msg" {
                            if p["type"] == "user_message" {
                                push(
                                    &mut legacy_messages,
                                    "user",
                                    text(&p["message"]),
                                    timestamp(&v, &["timestamp"]),
                                );
                            }
                            if p["type"] == "agent_message" {
                                push(
                                    &mut legacy_messages,
                                    "assistant",
                                    text(&p["message"]),
                                    timestamp(&v, &["timestamp"]),
                                );
                            }
                        }
                        if kind == "response_item" && p["type"] == "message" {
                            push(
                                &mut messages,
                                &s(p, &["role"]),
                                text(&p["content"]),
                                timestamp(&v, &["timestamp"]),
                            );
                        }
                        // event_msg mirrors response_item in modern rollouts; avoid duplicate messages.
                    }
                    "copilot" => {
                        let p = &v["data"];
                        if kind == "session.start" {
                            metadata(&mut session, p);
                            metadata(&mut session, &p["context"]);
                        }
                        if kind == "session.info" {
                            metadata(&mut session, p);
                        }
                        if kind == "user.message" || kind == "assistant.message" {
                            push(
                                &mut messages,
                                if kind == "user.message" {
                                    "user"
                                } else {
                                    "assistant"
                                },
                                text(&p["content"]),
                                timestamp(&v, &["timestamp"]),
                            );
                        }
                    }
                    "kimi" => {
                        let p = v.get("message").unwrap_or(&v);
                        let ty = s(p, &["type"]);
                        if ty == "TurnBegin" {
                            push(
                                &mut messages,
                                "user",
                                text(&p["payload"]["user_input"]),
                                timestamp(&v, &["timestamp"]),
                            );
                        } else if ty == "ContentPart" && p["payload"]["type"] == "text" {
                            let chunk = text(&p["payload"]["text"]);
                            if let Some(last) =
                                messages.last_mut().filter(|m| m.role == "assistant")
                            {
                                last.content.push_str(&chunk);
                            } else {
                                push(
                                    &mut messages,
                                    "assistant",
                                    chunk,
                                    timestamp(&v, &["timestamp"]),
                                );
                            }
                        } else {
                            push(
                                &mut messages,
                                &s(p, &["role"]),
                                text(&p["content"]),
                                timestamp(&v, &["timestamp"]),
                            );
                        }
                    }
                    _ => {
                        if self.id() == "qwen" && v["subtype"] == "custom_title" {
                            metadata(&mut session, &v["systemPayload"]);
                        }
                        let p = v.get("message").unwrap_or(&v);
                        let role = s(p, &["role"]);
                        let role = if role.is_empty() {
                            kind.as_str()
                        } else {
                            role.as_str()
                        };
                        push(
                            &mut messages,
                            if role == "model" { "assistant" } else { role },
                            text(
                                p.get("content")
                                    .or_else(|| p.get("parts"))
                                    .unwrap_or(&Value::Null),
                            ),
                            timestamp(&v, &["timestamp"]),
                        );
                    }
                }
                if messages.len() > 100_000 || legacy_messages.len() > 100_000 {
                    return Err(Error::msg("session_message_limit"));
                }
            }
            if bad > 0 {
                return Err(Error::msg("session_malformed_records"));
            }
        }
        if messages.is_empty() && self.id() == "codex" {
            messages = legacy_messages;
        }
        if self.id() == "kimi" {
            let dir = if path.file_name().is_some_and(|n| n == "wire.jsonl") {
                path.parent().and_then(Path::parent).and_then(Path::parent)
            } else {
                path.parent()
            };
            if let Some(dir) = dir {
                session.external_id = dir
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                for name in ["state.json", "metadata.json"] {
                    let p = dir.join(name);
                    if p.is_file() && paths::is_within(&paths::canonicalize(&p)?, root) {
                        metadata(&mut session, &read_json(&p)?);
                    }
                }
                let index = root.join("session_index.jsonl");
                if index.is_file() && paths::is_within(&paths::canonicalize(&index)?, root) {
                    for line in BufReader::new(fs::File::open(index)?).lines().take(100_000) {
                        if let Ok(v) = serde_json::from_str::<Value>(&line?) {
                            if s(&v, &["sessionId"]) == session.external_id {
                                metadata(&mut session, &v);
                            }
                        }
                    }
                }
            }
        }
        if session.external_id.is_empty() && self.id() == "copilot" {
            session.external_id = path
                .parent()
                .and_then(Path::file_name)
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
        }
        finish(&mut session, &messages, &path)?;
        Ok(vec![(session, messages)])
    }
}
fn read_json(p: &Path) -> Result<Value> {
    if fs::metadata(p)?.len() > 64 * 1024 * 1024 {
        return Err(Error::msg("session_file_limit"));
    }
    serde_json::from_reader(fs::File::open(p)?)
        .map_err(|_| Error::msg("unsupported_session_format"))
}
fn read_gemini(path: &Path, cancel: &AtomicBool) -> Result<(Value, Vec<Value>)> {
    // Current Gemini appends metadata, message replacements and rewind records.
    // Legacy pretty-printed JSON is still accepted without treating thoughts as messages.
    let mut meta = serde_json::json!({});
    let mut records: Vec<Value> = Vec::new();
    let mut reader = BufReader::new(fs::File::open(path)?);
    let mut line = Vec::new();
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::msg("canceled"));
        }
        line.clear();
        let n = reader
            .by_ref()
            .take(8 * 1024 * 1024 + 1)
            .read_until(b'\n', &mut line)?;
        if n == 0 {
            break;
        }
        if n > 8 * 1024 * 1024 {
            return Err(Error::msg("session_line_limit"));
        }
        let v: Value = match serde_json::from_slice(&line) {
            Ok(v) => v,
            Err(_) => {
                if meta.as_object().is_some_and(|m| m.is_empty()) {
                    let old = read_json(path)?;
                    return Ok((
                        old.clone(),
                        old["messages"].as_array().cloned().unwrap_or_default(),
                    ));
                }
                if !line.ends_with(b"\n") {
                    break;
                }
                return Err(Error::msg("session_malformed_records"));
            }
        };
        if let Some(id) = v["$rewindTo"].as_str() {
            let pos = records.iter().position(|m| m["id"] == id).unwrap_or(0);
            records.truncate(pos);
        } else if v.get("$set").is_some() || v.get("sessionId").is_some() {
            let m = v.get("$set").unwrap_or(&v);
            if let Some(items) = m["messages"].as_array() {
                records = items.clone();
            }
            if let Some(map) = m.as_object() {
                for (k, v) in map {
                    if k != "messages" {
                        meta[k] = v.clone();
                    }
                }
            }
        } else if v.get("id").is_some() {
            if let Some(i) = records.iter().position(|m| m["id"] == v["id"]) {
                records[i] = v;
            } else {
                records.push(v);
            }
        }
        if records.len() > 100_000 {
            return Err(Error::msg("session_message_limit"));
        }
    }
    Ok((meta, records))
}
// Client-injected setup remains in the transcript but is not a user's intent.
fn user_intent(content: &str) -> Option<&str> {
    let value = content
        .split_once("## My request:")
        .map(|(_, request)| request)
        .unwrap_or(content)
        .trim();
    if value.is_empty()
        || [
            "<recommended_plugins>",
            "<environment_context>",
            "<skills_instructions>",
            "# AGENTS.md instructions",
            "<permissions instructions>",
        ]
        .iter()
        .any(|prefix| value.starts_with(prefix))
    {
        return None;
    }
    Some(value)
}
fn finish(session: &mut AgentSession, messages: &[SessionMessage], path: &Path) -> Result<()> {
    if session.external_id.is_empty() {
        return Err(Error::msg("missing_session_identity"));
    }
    session.message_count = messages.len();
    if session.title.is_empty() {
        session.title = messages
            .iter()
            .filter(|m| m.role == "user")
            .find_map(|m| user_intent(&m.content))
            .map(|content| content.chars().take(120).collect())
            .unwrap_or_else(|| {
                format!(
                    "{} · {}",
                    session.adapter,
                    session.external_id.chars().take(12).collect::<String>()
                )
            });
    }
    session.last_user_excerpt = messages
        .iter()
        .rev()
        .filter(|m| m.role == "user")
        .find_map(|m| user_intent(&m.content))
        .map(|content| content.chars().take(280).collect())
        .unwrap_or_default();
    if session.updated_at.is_empty() {
        session.updated_at =
            chrono::DateTime::<chrono::Utc>::from(fs::metadata(path)?.modified()?).to_rfc3339();
    }
    if session.started_at.is_empty() {
        session.started_at = session.updated_at.clone();
    }
    session.capabilities = SessionCapabilities {
        search: true,
        transcript: true,
        direct_resume: true,
    };
    Ok(())
}
fn readonly(path: &Path) -> Result<rusqlite::Connection> {
    let db = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    db.busy_timeout(std::time::Duration::from_millis(500))?;
    db.pragma_update(None, "query_only", true)?;
    db.execute_batch("BEGIN")?;
    Ok(db)
}
fn read_opencode(
    path: &Path,
    cancel: &AtomicBool,
) -> Result<Vec<(AgentSession, Vec<SessionMessage>)>> {
    let db = readonly(path)?;
    let mut stmt = db.prepare("SELECT id,directory,title,time_created,time_updated FROM session ORDER BY time_updated DESC")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, i64>(4)?,
        ))
    })?;
    let mut sessions = Vec::new();
    for row in rows {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::msg("canceled"));
        }
        let (id, cwd, title, created, updated) = row?;
        let mut session = AgentSession {
            external_id: id.clone(),
            adapter: "opencode".into(),
            cwd,
            title,
            source_locator: paths::path_to_string(path),
            started_at: chrono::DateTime::from_timestamp_millis(created)
                .map(|t| t.to_rfc3339())
                .unwrap_or_default(),
            updated_at: chrono::DateTime::from_timestamp_millis(updated)
                .map(|t| t.to_rfc3339())
                .unwrap_or_default(),
            ..Default::default()
        };
        let mut ms = db.prepare("SELECT m.data,p.data FROM message m JOIN part p ON p.message_id=m.id WHERE m.session_id=?1 ORDER BY m.time_created,m.id,p.id")?;
        let mut messages = Vec::new();
        for row in ms.query_map([&id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })? {
            let (m, p) = row?;
            let m: Value =
                serde_json::from_str(&m).map_err(|_| Error::msg("unsupported_session_format"))?;
            let p: Value =
                serde_json::from_str(&p).map_err(|_| Error::msg("unsupported_session_format"))?;
            if p["type"] == "text" && p["synthetic"] != true && p["ignored"] != true {
                push(
                    &mut messages,
                    &s(&m, &["role"]),
                    text(&p["text"]),
                    String::new(),
                );
            }
        }
        finish(&mut session, &messages, path)?;
        sessions.push((session, messages));
    }
    Ok(sessions)
}
fn read_cursor(
    path: &Path,
    cancel: &AtomicBool,
) -> Result<Vec<(AgentSession, Vec<SessionMessage>)>> {
    let db = readonly(path)?;
    let mut session = AgentSession {
        adapter: "cursor-cli".into(),
        external_id: path
            .parent()
            .and_then(Path::file_name)
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        source_locator: paths::path_to_string(path),
        ..Default::default()
    };
    let mut messages = Vec::new();
    let mut root_id = String::new();
    let mut stmt = db.prepare("SELECT value FROM meta")?;
    for row in stmt.query_map([], |r| Ok(r.get_ref(0)?.as_bytes()?.to_vec()))? {
        let raw = row?;
        // Cursor meta is JSON, sometimes hex encoded by the Store layer.
        let decoded = decode_hex(&raw).unwrap_or(raw);
        if let Ok(v) = serde_json::from_slice::<Value>(&decoded) {
            metadata(&mut session, &v);
            root_id = s(&v, &["latestRootBlobId"]);
        }
    }
    let sidecar = path.with_file_name("meta.json");
    if sidecar.is_file() && !fs::symlink_metadata(&sidecar)?.file_type().is_symlink() {
        metadata(&mut session, &read_json(&sidecar)?);
    }
    if root_id.is_empty() {
        return Err(Error::msg("unsupported_cursor_store_format"));
    }
    let blob = |id: &str| -> Result<Vec<u8>> {
        let bytes: Vec<u8> =
            db.query_row("SELECT data FROM blobs WHERE id=?1", [id], |r| r.get(0))?;
        if bytes.len() > 8 * 1024 * 1024 {
            return Err(Error::msg("session_line_limit"));
        }
        Ok(bytes)
    };
    let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let root = blob(&root_id)?;
    for (_, turn_id) in wire_fields(&root)?.into_iter().filter(|(n, _)| *n == 8) {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::msg("canceled"));
        }
        if turn_id.len() != 32 {
            return Err(Error::msg("unsupported_cursor_store_format"));
        }
        let turn = blob(&hex(turn_id))?;
        for (_, agent_turn) in wire_fields(&turn)?.into_iter().filter(|(n, _)| *n == 1) {
            for (field, id) in wire_fields(agent_turn)? {
                if ![1, 2].contains(&field) {
                    continue;
                }
                if id.len() != 32 {
                    return Err(Error::msg("unsupported_cursor_store_format"));
                }
                let data = blob(&hex(id))?;
                if field == 1 {
                    for (_, visible) in wire_fields(&data)?.into_iter().filter(|(n, _)| *n == 1) {
                        push(
                            &mut messages,
                            "user",
                            String::from_utf8_lossy(visible).into_owned(),
                            String::new(),
                        );
                    }
                } else {
                    for (_, assistant) in wire_fields(&data)?.into_iter().filter(|(n, _)| *n == 1) {
                        for (_, visible) in
                            wire_fields(assistant)?.into_iter().filter(|(n, _)| *n == 1)
                        {
                            push(
                                &mut messages,
                                "assistant",
                                String::from_utf8_lossy(visible).into_owned(),
                                String::new(),
                            );
                        }
                    }
                }
            }
        }
    }
    finish(&mut session, &messages, path)?;
    Ok(vec![(session, messages)])
}
/// Bounded protobuf wire reader for Cursor's observed graph. Only known visible
/// message fields above are projected; arbitrary blobs, system and thinking are not.
fn wire_fields(bytes: &[u8]) -> Result<Vec<(u64, &[u8])>> {
    fn varint(bytes: &[u8], pos: &mut usize) -> Result<u64> {
        let mut n = 0u64;
        for shift in (0..=63).step_by(7) {
            let b = *bytes
                .get(*pos)
                .ok_or_else(|| Error::msg("invalid_cursor_wire"))?;
            *pos += 1;
            if shift == 63 && b > 1 {
                return Err(Error::msg("invalid_cursor_wire"));
            }
            n |= ((b & 127) as u64) << shift;
            if b < 128 {
                return Ok(n);
            }
        }
        Err(Error::msg("invalid_cursor_wire"))
    }
    let mut pos = 0;
    let mut fields = Vec::new();
    while pos < bytes.len() {
        let tag = varint(bytes, &mut pos)?;
        let n = tag >> 3;
        if n == 0 {
            return Err(Error::msg("invalid_cursor_wire"));
        }
        match tag & 7 {
            0 => {
                varint(bytes, &mut pos)?;
            }
            1 => {
                pos = pos
                    .checked_add(8)
                    .ok_or_else(|| Error::msg("invalid_cursor_wire"))?;
            }
            5 => {
                pos = pos
                    .checked_add(4)
                    .ok_or_else(|| Error::msg("invalid_cursor_wire"))?;
            }
            2 => {
                let len = varint(bytes, &mut pos)? as usize;
                let end = pos
                    .checked_add(len)
                    .filter(|e| *e <= bytes.len())
                    .ok_or_else(|| Error::msg("invalid_cursor_wire"))?;
                fields.push((n, &bytes[pos..end]));
                pos = end;
            }
            _ => return Err(Error::msg("invalid_cursor_wire")),
        }
        if pos > bytes.len() || fields.len() > 100_000 {
            return Err(Error::msg("invalid_cursor_wire"));
        }
    }
    Ok(fields)
}
fn decode_hex(bytes: &[u8]) -> Option<Vec<u8>> {
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    bytes
        .chunks(2)
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).ok()?, 16).ok())
        .collect()
}
