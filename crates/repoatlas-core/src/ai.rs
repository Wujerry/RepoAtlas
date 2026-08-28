use crate::error::{Error, Result};
use crate::models::{
    AiMemoryItem, AiSummary, ChatMessage, ConversationSummary, ProviderProfile, ProviderUpsert,
};
use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;
use uuid::Uuid;

pub mod presets {
    use crate::models::ProviderPreset;

    pub fn all() -> Vec<ProviderPreset> {
        vec![
            preset(
                "OpenAI",
                "openai-chat",
                "https://api.openai.com/v1",
                "gpt-4o",
                "OPENAI_API_KEY",
            ),
            preset(
                "DeepSeek",
                "openai-compatible",
                "https://api.deepseek.com/v1",
                "deepseek-chat",
                "DEEPSEEK_API_KEY",
            ),
            preset(
                "OpenRouter",
                "openai-compatible",
                "https://openrouter.ai/api/v1",
                "openrouter/auto",
                "OPENROUTER_API_KEY",
            ),
            preset(
                "通义 Qwen",
                "openai-compatible",
                "https://dashscope.aliyuncs.com/compatible-mode/v1",
                "qwen-plus",
                "DASHSCOPE_API_KEY",
            ),
            preset(
                "Anthropic",
                "anthropic",
                "https://api.anthropic.com",
                "claude-3-5-sonnet",
                "ANTHROPIC_API_KEY",
            ),
            preset(
                "Google Gemini",
                "gemini",
                "https://generativelanguage.googleapis.com/v1beta",
                "gemini-1.5-pro",
                "GEMINI_API_KEY",
            ),
            preset("Ollama", "ollama", "http://localhost:11434", "llama3.2", ""),
            preset(
                "LM Studio",
                "openai-compatible",
                "http://localhost:1234/v1",
                "local-model",
                "",
            ),
            preset(
                "OpenCode Go",
                "openai-compatible",
                "https://opencode.ai/zen/go/v1",
                "deepseek-v4-pro",
                "OPENCODE_API_KEY",
            ),
            preset(
                "Groq",
                "openai-compatible",
                "https://api.groq.com/openai/v1",
                "llama-3.3-70b-versatile",
                "GROQ_API_KEY",
            ),
            preset(
                "Together",
                "openai-compatible",
                "https://api.together.xyz/v1",
                "meta-llama/Llama-3.3-70B-Instruct-Turbo",
                "TOGETHER_API_KEY",
            ),
            preset(
                "Fireworks",
                "openai-compatible",
                "https://api.fireworks.ai/inference/v1",
                "accounts/fireworks/models/llama-v3p3-70b-instruct",
                "FIREWORKS_API_KEY",
            ),
            preset(
                "Moonshot Kimi",
                "openai-compatible",
                "https://api.moonshot.cn/v1",
                "kimi-k2-turbo-preview",
                "MOONSHOT_API_KEY",
            ),
            preset(
                "MiniMax",
                "openai-compatible",
                "https://api.minimax.chat/v1",
                "MiniMax-Text-01",
                "MINIMAX_API_KEY",
            ),
            preset(
                "SiliconFlow",
                "openai-compatible",
                "https://api.siliconflow.cn/v1",
                "deepseek-ai/DeepSeek-V3",
                "SILICONFLOW_API_KEY",
            ),
            preset(
                "智谱 GLM",
                "openai-compatible",
                "https://open.bigmodel.cn/api/paas/v4",
                "glm-4-plus",
                "ZHIPUAI_API_KEY",
            ),
            preset(
                "Mistral",
                "openai-compatible",
                "https://api.mistral.ai/v1",
                "mistral-large-latest",
                "MISTRAL_API_KEY",
            ),
            preset(
                "xAI",
                "openai-compatible",
                "https://api.x.ai/v1",
                "grok-3",
                "XAI_API_KEY",
            ),
        ]
    }

    fn preset(
        name: &str,
        protocol: &str,
        base_url: &str,
        model: &str,
        credential_ref: &str,
    ) -> ProviderPreset {
        ProviderPreset {
            name: name.into(),
            protocol: protocol.into(),
            base_url: Some(base_url.into()),
            default_model: model.into(),
            credential_ref: credential_ref.into(),
        }
    }
}

pub fn list_profiles(conn: &Connection) -> Result<Vec<ProviderProfile>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, protocol, base_url, model, credential_ref, created_at, updated_at FROM provider_profiles ORDER BY name",
    )?;
    let rows = stmt
        .query_map([], profile_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn upsert_profile(conn: &Connection, upsert: ProviderUpsert) -> Result<ProviderProfile> {
    if upsert.name.trim().is_empty() {
        return Err(Error::msg("provider name is required"));
    }
    let id = match upsert.id.as_deref() {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => Uuid::new_v4().to_string(),
    };
    let existing: Option<(String, String, String)> = conn
        .query_row(
            "SELECT protocol, base_url, model FROM provider_profiles WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let protocol = if upsert.protocol.trim().is_empty() {
        existing
            .as_ref()
            .map(|entry| entry.0.clone())
            .unwrap_or_default()
    } else {
        upsert.protocol.trim().to_ascii_lowercase()
    };
    if protocol.trim().is_empty() {
        return Err(Error::msg("provider protocol is required"));
    }
    if !matches!(
        protocol.as_str(),
        "openai-compatible" | "openai-chat" | "anthropic" | "gemini" | "ollama"
    ) {
        return Err(Error::msg("unsupported provider protocol"));
    }
    let model = if upsert.model.trim().is_empty() {
        existing
            .as_ref()
            .map(|entry| entry.2.clone())
            .unwrap_or_default()
    } else {
        upsert.model.trim().to_string()
    };
    if model.is_empty() {
        return Err(Error::msg("provider model is required"));
    }
    if let Some(base_url) = upsert.base_url.as_deref() {
        let base_url = base_url.trim();
        if base_url.is_empty()
            || base_url
                .chars()
                .any(|character| character.is_control() || character.is_whitespace())
            || !(base_url.starts_with("http://") || base_url.starts_with("https://"))
        {
            return Err(Error::msg(
                "provider endpoint must be an http(s) URL without whitespace",
            ));
        }
    }
    let credential_ref = upsert.credential_ref.trim();
    if !credential_ref.is_empty() {
        let mut chars = credential_ref.chars();
        let valid = chars
            .next()
            .is_some_and(|character| character.is_ascii_alphabetic() || character == '_')
            && chars.all(|character| character.is_ascii_alphanumeric() || character == '_');
        if !valid {
            return Err(Error::msg(
                "credential reference must be an environment variable name",
            ));
        }
    }
    let now = now();
    // Empty means "keep/entry"; empty credential_ref means none. We store ref only, never the key.
    let base_url = upsert
        .base_url
        .map(|value| value.trim().to_string())
        .or_else(|| existing.as_ref().map(|entry| entry.1.clone()));
    conn.execute(
        r#"
        INSERT INTO provider_profiles (id, name, protocol, base_url, model, credential_ref, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
        ON CONFLICT(id) DO UPDATE SET
            name=excluded.name, protocol=excluded.protocol, base_url=excluded.base_url,
            model=excluded.model, credential_ref=excluded.credential_ref, updated_at=excluded.updated_at
        "#,
        params![id, upsert.name.trim(), protocol, base_url, model, credential_ref, now],
    )?;
    get_profile(conn, &id)
}

pub fn delete_profile(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM provider_profiles WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn get_profile(conn: &Connection, id: &str) -> Result<ProviderProfile> {
    conn.query_row(
        "SELECT id, name, protocol, base_url, model, credential_ref, created_at, updated_at FROM provider_profiles WHERE id = ?1",
        params![id],
        profile_row,
    )
    .map_err(|_| Error::NotFound(id.into()))
}

pub fn list_memory(conn: &Connection, project_id: &str) -> Result<Vec<AiMemoryItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, text, created_at FROM ai_memory WHERE project_id = ?1 ORDER BY datetime(created_at)",
    )?;
    let rows = stmt
        .query_map(params![project_id], memory_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn add_memory(conn: &Connection, project_id: &str, text: &str) -> Result<AiMemoryItem> {
    let text = text.trim();
    if text.is_empty() {
        return Err(Error::msg("memory text is empty"));
    }
    let item = AiMemoryItem {
        id: Uuid::new_v4().to_string(),
        project_id: project_id.into(),
        text: text.into(),
        created_at: now(),
    };
    conn.execute(
        "INSERT INTO ai_memory (id, project_id, text, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![item.id, item.project_id, item.text, item.created_at],
    )?;
    Ok(item)
}

pub fn delete_memory(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM ai_memory WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn latest_summary(conn: &Connection, project_id: &str) -> Result<Option<AiSummary>> {
    conn.query_row(
        "SELECT id, project_id, provider_id, model, evidence_snapshot, text, created_at FROM ai_summaries WHERE project_id = ?1 ORDER BY datetime(created_at) DESC LIMIT 1",
        params![project_id],
        summary_row,
    )
    .optional()
    .map_err(Into::into)
}

pub fn list_summaries(conn: &Connection, project_id: &str) -> Result<Vec<AiSummary>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, provider_id, model, evidence_snapshot, text, created_at FROM ai_summaries WHERE project_id = ?1 ORDER BY datetime(created_at) DESC LIMIT 20",
    )?;
    let rows = stmt.query_map(params![project_id], summary_row)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub fn get_summary(conn: &Connection, summary_id: &str) -> Result<AiSummary> {
    conn.query_row(
        "SELECT id, project_id, provider_id, model, evidence_snapshot, text, created_at FROM ai_summaries WHERE id = ?1",
        params![summary_id],
        summary_row,
    )
    .map_err(|_| Error::NotFound(summary_id.into()))
}

pub fn delete_summary(conn: &Connection, summary_id: &str) -> Result<()> {
    let deleted = conn.execute(
        "DELETE FROM ai_summaries WHERE id = ?1",
        params![summary_id],
    )?;
    if deleted == 0 {
        return Err(Error::NotFound(summary_id.into()));
    }
    Ok(())
}

pub fn save_summary(
    conn: &Connection,
    project_id: &str,
    provider_id: Option<&str>,
    model: Option<&str>,
    text: &str,
    evidence: &str,
) -> Result<AiSummary> {
    let summary = AiSummary {
        id: Uuid::new_v4().to_string(),
        project_id: project_id.into(),
        provider_id: provider_id.map(str::to_string),
        model: model.map(str::to_string),
        evidence_snapshot: Some(evidence.into()),
        text: text.into(),
        created_at: now(),
    };
    conn.execute(
        "INSERT INTO ai_summaries (id, project_id, provider_id, model, evidence_snapshot, text, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![summary.id, summary.project_id, summary.provider_id, summary.model, evidence, summary.text, summary.created_at],
    )?;
    Ok(summary)
}

pub fn conversation(conn: &Connection, project_id: &str) -> Result<Vec<ChatMessage>> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT messages FROM project_conversations WHERE project_id = ?1",
            params![project_id],
            |row| row.get(0),
        )
        .optional()?;
    match raw {
        Some(raw) => Ok(serde_json::from_str(&raw).unwrap_or_default()),
        None => Ok(Vec::new()),
    }
}

pub fn conversation_summary(conn: &Connection, project_id: &str) -> Result<ConversationSummary> {
    let messages = conversation(conn, project_id)?;
    let updated_at: Option<String> = conn
        .query_row(
            "SELECT updated_at FROM project_conversations WHERE project_id = ?1",
            params![project_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(ConversationSummary {
        project_id: project_id.into(),
        message_count: messages.len(),
        updated_at: updated_at.unwrap_or_else(now),
    })
}

pub fn append_conversation(
    conn: &Connection,
    project_id: &str,
    messages: Vec<ChatMessage>,
) -> Result<()> {
    let existing = conversation(conn, project_id)?;
    let mut merged = existing;
    merged.extend(messages);
    if merged.len() > 100 {
        let keep = merged.split_off(merged.len() - 100);
        merged = keep;
    }
    let now = now();
    conn.execute(
        "INSERT INTO project_conversations (project_id, messages, updated_at) VALUES (?1, ?2, ?3) ON CONFLICT(project_id) DO UPDATE SET messages=excluded.messages, updated_at=excluded.updated_at",
        params![project_id, serde_json::to_string(&merged).unwrap_or_else(|_| "[]".into()), now],
    )?;
    Ok(())
}

pub fn clear_conversation(conn: &Connection, project_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM project_conversations WHERE project_id = ?1",
        params![project_id],
    )?;
    Ok(())
}

pub struct AiCaller {
    pub profile: ProviderProfile,
    pub api_key: String,
}

impl AiCaller {
    pub fn chat(
        &self,
        system: &str,
        messages: Vec<ChatMessage>,
        max_tokens: u32,
    ) -> Result<String> {
        match self.profile.protocol.as_str() {
            "anthropic" => self.chat_anthropic(system, messages, max_tokens),
            "gemini" => self.chat_gemini(system, messages, max_tokens),
            "ollama" => self.chat_openai_like(system, messages, max_tokens, false),
            _ => self.chat_openai_like(system, messages, max_tokens, true),
        }
    }

    fn endpoint(&self, path: &str) -> String {
        let base = self
            .profile
            .base_url
            .clone()
            .unwrap_or_default()
            .trim_end_matches('/')
            .to_string();
        format!("{base}{path}")
    }

    fn chat_openai_like(
        &self,
        system: &str,
        messages: Vec<ChatMessage>,
        max_tokens: u32,
        auth: bool,
    ) -> Result<String> {
        let mut all_messages = vec![json!({"role":"system","content":system})];
        for m in &messages {
            all_messages.push(json!({"role": m.role, "content": m.content}));
        }
        let mut payload = json!({
            "model": self.profile.model,
            "max_tokens": max_tokens,
            "messages": all_messages,
        });
        // ollama uses num_predict instead of max_tokens
        if self.profile.protocol == "ollama" {
            payload.as_object_mut().unwrap().remove("max_tokens");
            payload
                .as_object_mut()
                .unwrap()
                .insert("num_predict".into(), json!(max_tokens));
            payload
                .as_object_mut()
                .unwrap()
                .insert("stream".into(), json!(false));
        }
        let url = self.endpoint("/chat/completions");
        let agent = http_agent();
        let mut req = agent.post(&url).set("Content-Type", "application/json");
        if auth && !self.api_key.is_empty() {
            req = req.set("Authorization", &format!("Bearer {}", self.api_key));
        }
        let resp = req
            .send_json(payload)
            .map_err(|err| ai_request_error(err, &self.api_key))?;
        let body = read_json_limited(resp)?;
        body["choices"][0]["message"]["content"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| Error::msg("AI response missing content"))
    }

    fn chat_anthropic(
        &self,
        system: &str,
        messages: Vec<ChatMessage>,
        max_tokens: u32,
    ) -> Result<String> {
        let url = self.endpoint("/v1/messages");
        let body = json!({
            "model": self.profile.model,
            "max_tokens": max_tokens,
            "system": system,
            "messages": messages.iter().map(|m| json!({"role": if m.role=="assistant" {"assistant"} else {"user"}, "content": m.content})).collect::<Vec<_>>(),
        });
        let resp = http_agent()
            .post(&url)
            .set("Content-Type", "application/json")
            .set("x-api-key", &self.api_key)
            .set("anthropic-version", "2023-06-01")
            .send_json(body)
            .map_err(|err| ai_request_error(err, &self.api_key))?;
        let body = read_json_limited(resp)?;
        body["content"][0]["text"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| Error::msg("AI response missing content"))
    }

    fn chat_gemini(
        &self,
        system: &str,
        messages: Vec<ChatMessage>,
        max_tokens: u32,
    ) -> Result<String> {
        let url = format!(
            "{}models/{}:generateContent",
            self.endpoint("/"),
            self.profile.model
        );
        let mut contents = messages
            .iter()
            .filter(|m| m.role == "user" || m.role == "assistant")
            .map(|m| json!({"role": if m.role=="assistant" {"model"} else {"user"}, "parts":[{"text":m.content}]}))
            .collect::<Vec<_>>();
        if !system.is_empty() {
            contents.insert(0, json!({"role":"user","parts":[{"text":system}]}));
        }
        let body = json!({
            "contents": contents,
            "generationConfig": {"maxOutputTokens": max_tokens}
        });
        let resp = http_agent()
            .post(&url)
            .set("Content-Type", "application/json")
            .set("x-goog-api-key", &self.api_key)
            .send_json(body)
            .map_err(|err| ai_request_error(err, &self.api_key))?;
        let body = read_json_limited(resp)?;
        body["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| Error::msg("AI response missing content"))
    }
}

const MAX_AI_RESPONSE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_EVIDENCE_FILE_BYTES: u64 = 512 * 1024;

fn http_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(Duration::from_secs(60))
        .timeout_write(Duration::from_secs(60))
        .build()
}

fn read_json_limited(response: ureq::Response) -> Result<Value> {
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(MAX_AI_RESPONSE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|err| Error::msg(format!("AI response could not be read: {err}")))?;
    if bytes.len() as u64 > MAX_AI_RESPONSE_BYTES {
        return Err(Error::msg("AI response exceeds the 4 MiB limit"));
    }
    serde_json::from_slice(&bytes).map_err(|err| Error::msg(format!("AI response invalid: {err}")))
}

fn ai_request_error(error: ureq::Error, api_key: &str) -> Error {
    let mut message = error.to_string();
    if !api_key.is_empty() {
        message = message.replace(api_key, "[redacted]");
    }
    Error::msg(format!("AI request failed: {message}"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceBundle {
    pub text: String,
    pub files: Vec<String>,
    pub character_count: usize,
    pub suspicious_secret_count: usize,
}

pub fn evidence_for(path: &Path, max_chars: usize) -> Result<EvidenceBundle> {
    if max_chars == 0 {
        return Ok(EvidenceBundle {
            text: String::new(),
            files: Vec::new(),
            character_count: 0,
            suspicious_secret_count: 0,
        });
    }

    let mut parts = Vec::new();
    let mut files = Vec::new();
    let mut selected_paths: Vec<PathBuf> = Vec::new();
    let mut suspicious_secret_count = 0;
    for name in [
        "README.md",
        "Readme.md",
        "readme.md",
        "package.json",
        "Cargo.toml",
        "go.mod",
        "pyproject.toml",
        "pom.xml",
        "tauri.conf.json",
    ] {
        let file = path.join(name);
        if file.is_file() {
            if selected_paths
                .iter()
                .any(|selected| same_evidence_path(selected, &file))
            {
                continue;
            }
            if let Some(text) = read_evidence_file(&file) {
                let redacted = redact_secret_lines(&text);
                suspicious_secret_count += redacted.suspicious_secret_count;
                parts.push(format!("===== {name} =====\n{}", redacted.text));
                files.push(name.to_string());
                selected_paths.push(file);
                if parts.iter().map(|part| part.chars().count()).sum::<usize>() > max_chars {
                    break;
                }
            }
        }
    }
    let joined = parts.join("\n\n");
    let text: String = joined.chars().take(max_chars).collect();
    let character_count = text.chars().count();
    Ok(EvidenceBundle {
        text,
        files,
        character_count,
        suspicious_secret_count,
    })
}

/// Read evidence with a per-file ceiling. A repository can contain generated
/// manifests that are much larger than the final prompt budget; bounding the
/// read here prevents those files from consuming memory before `max_chars`
/// is applied. The explicit marker keeps the snapshot honest for the model.
fn read_evidence_file(path: &Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take(MAX_EVIDENCE_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    let truncated = bytes.len() as u64 > MAX_EVIDENCE_FILE_BYTES;
    bytes.truncate(MAX_EVIDENCE_FILE_BYTES as usize);
    if bytes.contains(&0) {
        return None;
    }
    let mut text = String::from_utf8(bytes).ok()?;
    if truncated {
        text.push_str("\n[RepoAtlas truncated this evidence file at 512 KiB]");
    }
    Some(text)
}

#[cfg(windows)]
fn same_evidence_path(left: &Path, right: &Path) -> bool {
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
}

#[cfg(not(windows))]
fn same_evidence_path(left: &Path, right: &Path) -> bool {
    left == right
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RedactedText {
    text: String,
    suspicious_secret_count: usize,
}

fn redact_secret_lines(text: &str) -> RedactedText {
    let mut redacted = String::with_capacity(text.len());
    let mut suspicious_secret_count = 0;
    for line in text.split_inclusive('\n') {
        let has_newline = line.ends_with('\n');
        let content = line.strip_suffix('\n').unwrap_or(line);
        if is_suspicious_secret_line(content) {
            redacted.push_str("[REDACTED]");
            suspicious_secret_count += 1;
        } else {
            redacted.push_str(content);
        }
        if has_newline {
            redacted.push('\n');
        }
    }
    RedactedText {
        text: redacted,
        suspicious_secret_count,
    }
}

fn is_suspicious_secret_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    [
        "api_key",
        "api-key",
        "apikey",
        "access_token",
        "access-token",
        "accesstoken",
        "auth_token",
        "auth-token",
        "authtoken",
        "refresh_token",
        "refresh-token",
        "refreshtoken",
        "client_secret",
        "client-secret",
        "clientsecret",
        "private_key",
        "private-key",
        "privatekey",
        "secret_key",
        "secret-key",
        "secretkey",
        "private key",
        "authorization",
        "bearer ",
        "password",
        "passwd",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
        || ["token", "secret", "key"]
            .iter()
            .any(|name| has_named_assignment(&lower, name))
}

fn has_named_assignment(line: &str, name: &str) -> bool {
    let mut offset = 0;
    while let Some(found) = line[offset..].find(name) {
        let start = offset + found;
        let end = start + name.len();
        let before = line[..start].chars().next_back();
        let after = line[end..].chars().next();
        let boundary = |character: Option<char>| {
            character
                .map(|value| !value.is_ascii_alphanumeric() && value != '_' && value != '-')
                .unwrap_or(true)
        };
        if boundary(before) && boundary(after) {
            let remainder = line[end..].trim_start();
            if remainder.starts_with('=')
                || remainder.starts_with(':')
                || remainder
                    .strip_prefix('"')
                    .is_some_and(|value| value.trim_start().starts_with(':'))
            {
                return true;
            }
        }
        offset = end;
    }
    false
}

pub fn is_local_provider(profile: &ProviderProfile) -> bool {
    if profile.protocol.trim().eq_ignore_ascii_case("ollama")
        || profile.name.trim().eq_ignore_ascii_case("lm studio")
    {
        return true;
    }
    let base_url = profile.base_url.as_deref().unwrap_or_default().trim();
    let authority = base_url
        .split_once("://")
        .map(|(_, remainder)| remainder)
        .unwrap_or(base_url)
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or_else(|| {
            base_url
                .split_once("://")
                .map(|(_, remainder)| remainder)
                .unwrap_or(base_url)
                .split(['/', '?', '#'])
                .next()
                .unwrap_or_default()
        });
    let host = authority
        .strip_prefix('[')
        .and_then(|value| value.split(']').next())
        .or_else(|| authority.rsplit_once(':').map(|(host, _)| host))
        .unwrap_or(authority)
        .to_ascii_lowercase();
    matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1")
}

fn profile_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProviderProfile> {
    Ok(ProviderProfile {
        id: row.get(0)?,
        name: row.get(1)?,
        protocol: row.get(2)?,
        base_url: row.get(3)?,
        model: row.get(4)?,
        credential_ref: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn memory_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AiMemoryItem> {
    Ok(AiMemoryItem {
        id: row.get(0)?,
        project_id: row.get(1)?,
        text: row.get(2)?,
        created_at: row.get(3)?,
    })
}

fn summary_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AiSummary> {
    Ok(AiSummary {
        id: row.get(0)?,
        project_id: row.get(1)?,
        provider_id: row.get(2)?,
        model: row.get(3)?,
        evidence_snapshot: row.get(4)?,
        text: row.get(5)?,
        created_at: row.get(6)?,
    })
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::{evidence_for, is_local_provider};
    use crate::models::ProviderProfile;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn evidence_lists_files_counts_characters_and_redacts_secret_lines() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("README.md"),
            "# Project\nAPI_KEY=do-not-send-this\n正常内容\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{"name":"safe","token":"also-do-not-send"}"#,
        )
        .unwrap();

        let evidence = evidence_for(dir.path(), 8000).unwrap();

        assert_eq!(evidence.files, vec!["README.md", "package.json"]);
        assert_eq!(evidence.suspicious_secret_count, 2);
        assert_eq!(evidence.character_count, evidence.text.chars().count());
        assert!(evidence.text.contains("[REDACTED]"));
        assert!(!evidence.text.contains("do-not-send-this"));
        assert!(!evidence.text.contains("also-do-not-send"));
    }

    #[test]
    fn local_provider_detection_matches_supported_local_endpoints() {
        let profile = |name: &str, protocol: &str, base_url: Option<&str>| ProviderProfile {
            id: "provider".into(),
            name: name.into(),
            protocol: protocol.into(),
            base_url: base_url.map(str::to_string),
            model: "model".into(),
            credential_ref: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
        };

        assert!(is_local_provider(&profile(
            "Ollama",
            "ollama",
            Some("http://localhost:11434"),
        )));
        assert!(is_local_provider(&profile(
            "LM Studio",
            "openai-compatible",
            Some("http://127.0.0.1:1234/v1"),
        )));
        assert!(is_local_provider(&profile(
            "Local IPv6",
            "openai-compatible",
            Some("http://[::1]:1234/v1"),
        )));
        assert!(!is_local_provider(&profile(
            "OpenAI",
            "openai-chat",
            Some("https://api.openai.com/v1"),
        )));
    }
}
