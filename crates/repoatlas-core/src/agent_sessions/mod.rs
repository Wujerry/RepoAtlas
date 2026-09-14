//! Read-only external conversation adapters. Authorization lives in Core.
pub mod adapters;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSource {
    pub id: String,
    pub adapter: String,
    pub path: String,
    pub enabled: bool,
    pub last_scanned_at: Option<String>,
    pub last_error: Option<String>,
}
impl SessionSource {
    pub fn is_fresh(&self) -> bool {
        self.last_scanned_at
            .as_ref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .is_some_and(|t| {
                let seconds = chrono::Utc::now().signed_duration_since(t).num_seconds();
                (0..60).contains(&seconds)
            })
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionCapabilities {
    pub search: bool,
    pub transcript: bool,
    pub direct_resume: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AgentSession {
    pub id: String,
    pub source_id: String,
    pub adapter: String,
    pub external_id: String,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub match_kind: String,
    pub cwd: String,
    pub title: String,
    pub last_user_excerpt: String,
    pub started_at: String,
    pub updated_at: String,
    pub message_count: usize,
    pub archived: bool,
    pub source_missing: bool,
    pub source_locator: String,
    pub revision: String,
    pub resume_reason: Option<String>,
    pub capabilities: SessionCapabilities,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMessage {
    pub index: usize,
    pub role: String,
    pub content: String,
    pub timestamp: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionQuery {
    pub query: String,
    pub project_id: Option<String>,
    pub adapter: Option<String>,
    pub after: Option<String>,
    pub before: Option<String>,
    pub archived: bool,
    pub offset: usize,
    pub limit: usize,
    pub resumable_only: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSearchHit {
    pub session: AgentSession,
    pub snippets: Vec<SessionMessage>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSearchResult {
    pub items: Vec<SessionSearchHit>,
    pub total: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMessagePage {
    pub items: Vec<SessionMessage>,
    pub total: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionRefreshJob {
    pub running: bool,
    pub canceled: bool,
    pub processed: usize,
    pub errors: usize,
    pub source_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionResumeSpec {
    pub app: Option<crate::launch::SessionAppTarget>,
    pub agent: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub command: String,
    pub env: std::collections::BTreeMap<String, String>,
}

pub fn resume_args(adapter: &str, id: &str) -> crate::Result<Vec<String>> {
    if !id
        .as_bytes()
        .first()
        .is_some_and(|b| b.is_ascii_alphanumeric())
        || id.len() > 200
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    {
        return Err(crate::Error::msg("invalid_session_id"));
    }
    Ok(match adapter {
        "codex" => vec!["resume".into(), id.into()],
        "claude" | "gemini" | "qwen" => vec!["--resume".into(), id.into()],
        "opencode" | "kimi" => vec!["--session".into(), id.into()],
        "cursor-cli" | "copilot" => vec![format!("--resume={id}")],
        _ => return Err(crate::Error::msg("unsupported_adapter")),
    })
}
