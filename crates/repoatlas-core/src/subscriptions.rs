//! Read-only, allowlisted provider usage. Never log or serialize credentials.
use crate::{Error, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{io::Read, path::PathBuf, time::Duration};
mod catalog;
mod extended;
mod google;
mod grok;

pub const PROVIDERS: &[&str] = &[
    "codex",
    "claude",
    "copilot",
    "opencode-go",
    "kimi",
    "cursor",
    "zai",
    "zhipu",
    "minimax",
    "minimax-cn",
    "openrouter",
    "deepseek",
    "grok",
    "antigravity",
    "factory",
    "zed",
    "stepfun",
];
pub fn validate_provider(provider: &str) -> Result<()> {
    if PROVIDERS.contains(&provider) {
        Ok(())
    } else {
        Err(Error::msg("usage_unsupported_provider"))
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    pub id: String,
    pub label: String,
    pub used_percent: Option<f64>,
    pub used: Option<f64>,
    pub limit: Option<f64>,
    pub unlimited: bool,
    pub resets_at: Option<String>,
    pub window_minutes: Option<f64>,
    #[serde(default)]
    pub remaining: Option<f64>,
    #[serde(default)]
    pub currency: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionUsage {
    pub provider: String,
    pub enabled: bool,
    pub plan: Option<String>,
    pub status: String,
    pub fetched_at: Option<String>,
    pub attempted_at: Option<String>,
    pub windows: Vec<UsageWindow>,
}
impl SubscriptionUsage {
    pub fn empty(provider: &str, enabled: bool) -> Self {
        Self {
            provider: provider.into(),
            enabled,
            plan: None,
            status: if enabled { "not_connected" } else { "disabled" }.into(),
            fetched_at: None,
            attempted_at: None,
            windows: vec![],
        }
    }
    pub fn fresh(&self, force: bool) -> bool {
        let age = self
            .attempted_at
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|t| Utc::now().signed_duration_since(t).num_seconds());
        age.is_some_and(|age| {
            age >= 0
                && age
                    < if force {
                        30
                    } else if self.status == "ready" {
                        300
                    } else {
                        60
                    }
        })
    }
}
fn number(v: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|k| {
            v.get(k)
                .and_then(|v| v.as_f64().or_else(|| v.as_str()?.parse().ok()))
        })
        .filter(|v| v.is_finite() && *v >= 0.)
}
fn string(v: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|k| v.get(k)?.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.chars().filter(|c| !c.is_control()).take(100).collect())
}
fn time(v: &Value) -> Option<String> {
    if let Some(s) = v.as_str() {
        if let Ok(d) = DateTime::parse_from_rfc3339(s) {
            return Some(d.to_utc().to_rfc3339());
        }
        if let Ok(d) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
            return Some(d.and_hms_opt(0, 0, 0)?.and_utc().to_rfc3339());
        }
    }
    v.as_i64()
        .or_else(|| v.as_str()?.parse().ok())
        .and_then(|n| {
            if n >= 1_000_000_000_000 {
                DateTime::from_timestamp_millis(n)
            } else {
                DateTime::from_timestamp(n, 0)
            }
        })
        .map(|d| d.to_rfc3339())
}
fn window(id: &str, label: &str, v: &Value, reset: Option<&Value>) -> UsageWindow {
    let limit = number(v, &["limit", "entitlement", "total"]);
    let remaining = number(v, &["remaining"]);
    let used = number(v, &["used", "consumed"]).or_else(|| {
        limit
            .zip(remaining)
            .map(|(limit, remaining)| (limit - remaining).max(0.))
    });
    let percent = number(
        v,
        &["used_percent", "usedPercent", "utilization", "percent"],
    )
    .or_else(|| number(v, &["percent_remaining"]).map(|n| 100. - n))
    .or_else(|| {
        limit.filter(|n| *n > 0.).and_then(|limit| {
            used.or_else(|| remaining.map(|r| (limit - r).max(0.)))
                .map(|u| 100. * u / limit)
        })
    });
    UsageWindow {
        id: id.into(),
        label: label.into(),
        used_percent: percent.map(|p| p.clamp(0., 100.)),
        used,
        limit,
        unlimited: v["unlimited"] == true,
        resets_at: ["resets_at", "resetsAt", "reset_at", "resetAt", "resetTime"]
            .iter()
            .find_map(|k| time(&v[k]))
            .or_else(|| reset.and_then(time)),
        window_minutes: number(v, &["windowDurationMins", "window_duration_mins"])
            .or_else(|| number(v, &["limit_window_seconds"]).map(|s| s / 60.)),
        remaining,
        currency: None,
    }
}
pub fn parse(provider: &str, value: &Value) -> Result<SubscriptionUsage> {
    validate_provider(provider)?;
    let mut result = SubscriptionUsage::empty(provider, true);
    result.plan = string(
        value,
        &["plan_type", "planType", "copilot_plan", "subscriptionType"],
    );
    match provider {
        "codex" => {
            let canonical = value
                .get("rateLimitsByLimitId")
                .or_else(|| value.get("rate_limits_by_limit_id"))
                .and_then(|v| v.get("codex"));
            let rates = canonical
                .or_else(|| value.get("rate_limit"))
                .or_else(|| value.get("rateLimits"))
                .or_else(|| value.get("rate_limits"));
            if let Some(rates) = rates {
                for (key, alias, label) in [
                    ("primary_window", "primary", "rolling"),
                    ("secondary_window", "secondary", "weekly"),
                ] {
                    if let Some(v) = rates.get(key).or_else(|| rates.get(alias)) {
                        result.windows.push(window(alias, label, v, None));
                    }
                }
                result.plan = result
                    .plan
                    .or_else(|| string(rates, &["planType", "plan_type"]));
            }
            if let Some(additional) = value["additional_rate_limits"].as_array() {
                for entry in additional.iter().take(16) {
                    let id =
                        string(entry, &["metered_feature"]).unwrap_or_else(|| "additional".into());
                    let label = string(entry, &["limit_name"]).unwrap_or_else(|| id.clone());
                    for key in ["primary_window", "secondary_window"] {
                        if let Some(v) = entry["rate_limit"].get(key) {
                            result
                                .windows
                                .push(window(&format!("{id}:{key}"), &label, v, None));
                        }
                    }
                }
            }
        }
        "claude" => {
            for (key, label) in [
                ("five_hour", "rolling"),
                ("seven_day", "weekly"),
                ("seven_day_sonnet", "Sonnet"),
                ("seven_day_opus", "Opus"),
            ] {
                if let Some(v) = value.get(key).filter(|v| v.is_object()) {
                    let mut w = window(key, label, v, None);
                    w.window_minutes = Some(if key == "five_hour" { 300. } else { 10080. });
                    result.windows.push(w);
                }
            }
        }
        "copilot" => {
            if let Some(quotas) = value["quota_snapshots"].as_object() {
                for (id, v) in quotas.iter().take(16) {
                    result
                        .windows
                        .push(window(id, id, v, value.get("quota_reset_date")));
                }
            }
        }
        "opencode-go" => {
            result.plan = Some("Go".into());
            for (id, label) in [
                ("rolling", "rolling"),
                ("weekly", "weekly"),
                ("monthly", "monthly"),
            ] {
                if let Some(v) = value["usage"].get(id) {
                    result.windows.push(window(id, label, v, None));
                }
            }
        }
        "kimi" => {
            let value = value.get("data").unwrap_or(value);
            if let Some(v) = value.get("usage") {
                result.windows.push(window("weekly", "weekly", v, None));
            }
            if let Some(limits) = value["limits"].as_array() {
                for (i, v) in limits.iter().take(16).enumerate() {
                    let detail = v.get("detail").unwrap_or(v);
                    let mut w = window(&format!("limit:{i}"), "rolling", detail, None);
                    if let Some(n) = number(&v["window"], &["duration"]) {
                        let unit = v["window"]["timeUnit"].as_str().unwrap_or("");
                        w.window_minutes = if unit.contains("MINUTE") {
                            Some(n)
                        } else if unit.contains("HOUR") {
                            Some(n * 60.)
                        } else if unit.contains("DAY") {
                            Some(n * 1440.)
                        } else {
                            None
                        };
                    }
                    result.windows.push(w);
                }
            }
            if result.windows.is_empty() {
                for (id, label) in [
                    ("limit_5h", "rolling"),
                    ("limit_7d", "weekly"),
                    ("limit_monthly", "monthly"),
                ] {
                    let v = &value["usages"][id];
                    if let Some(ratio) = number(v, &["used_ratio", "usedRatio"]) {
                        let mut w = window(id, label, v, None);
                        w.used_percent = Some((ratio * 100.).clamp(0., 100.));
                        result.windows.push(w);
                    }
                }
            }
        }
        _ => extended::parse(provider, value, &mut result),
    }
    result.windows.retain(|w| {
        w.used_percent.is_some()
            || w.unlimited
            || w.currency.is_some() && (w.remaining.is_some() || w.used.is_some())
    });
    result.status = if result.windows.is_empty() {
        "unsupported"
    } else {
        "ready"
    }
    .into();
    Ok(result)
}

fn home() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_default()
}
fn base(env: &str, fallback: &str) -> PathBuf {
    std::env::var_os(env)
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(fallback))
}
fn read_json(path: PathBuf) -> std::result::Result<Value, &'static str> {
    let meta = std::fs::symlink_metadata(&path).map_err(|_| "not_connected")?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > 1024 * 1024 {
        return Err("not_connected");
    }
    let file = std::fs::File::open(path).map_err(|_| "not_connected")?;
    serde_json::from_reader(file.take(1024 * 1024 + 1)).map_err(|_| "not_connected")
}
fn env_token(names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|n| std::env::var(n).ok().filter(|s| !s.trim().is_empty()))
}
#[cfg(windows)]
fn claude_secure_store() -> Option<Value> {
    use windows_sys::Win32::Security::Credentials::{
        CredFree, CredReadW, CREDENTIALW, CRED_TYPE_GENERIC,
    };
    let mut targets = vec!["Claude Code-credentials".to_owned()];
    for key in ["USER", "USERNAME"] {
        if let Ok(user) = std::env::var(key) {
            targets.push(format!("Claude Code-credentials:{user}"));
            targets.push(format!("Claude Code-credentials/{user}"));
        }
    }
    for target in targets {
        let target: Vec<u16> = target.encode_utf16().chain(Some(0)).collect();
        let mut pointer: *mut CREDENTIALW = std::ptr::null_mut();
        // The OS owns this bounded buffer until CredFree, including parse failure.
        unsafe {
            if CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut pointer) == 0
                || pointer.is_null()
            {
                continue;
            }
            let entry = &*pointer;
            let parsed =
                if !entry.CredentialBlob.is_null() && entry.CredentialBlobSize <= 1024 * 1024 {
                    let bytes = std::slice::from_raw_parts(
                        entry.CredentialBlob,
                        entry.CredentialBlobSize as usize,
                    );
                    let utf8 = String::from_utf8_lossy(bytes);
                    serde_json::from_str(utf8.trim_end_matches('\0'))
                        .ok()
                        .or_else(|| {
                            let words: Vec<u16> = bytes
                                .chunks_exact(2)
                                .map(|p| u16::from_le_bytes([p[0], p[1]]))
                                .collect();
                            serde_json::from_str(
                                String::from_utf16_lossy(&words).trim_end_matches('\0'),
                            )
                            .ok()
                        })
                } else {
                    None
                };
            CredFree(pointer.cast());
            if parsed.is_some() {
                return parsed;
            }
        }
    }
    None
}
#[cfg(target_os = "macos")]
fn claude_secure_store() -> Option<Value> {
    use std::process::{Command, Stdio};
    let mut child = Command::new("/usr/bin/security")
        .args([
            "find-generic-password",
            "-s",
            "Claude Code-credentials",
            "-w",
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .ok()?;
    let stdout = child.stdout.take()?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = vec![];
        let result = stdout.take(1024 * 1024 + 1).read_to_end(&mut bytes);
        let _ = tx.send((result, bytes));
    });
    let output = rx.recv_timeout(Duration::from_secs(3)).ok();
    let _ = child.kill();
    let _ = child.wait();
    let (result, bytes) = output?;
    if result.is_err() || bytes.len() > 1024 * 1024 {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}
#[cfg(not(any(windows, target_os = "macos")))]
fn claude_secure_store() -> Option<Value> {
    None
}
struct Credential {
    token: String,
    account: Option<String>,
    plan: Option<String>,
}
fn credential(provider: &str) -> std::result::Result<Credential, &'static str> {
    let mut result = Credential {
        token: String::new(),
        account: None,
        plan: None,
    };
    match provider {
        "codex" => {
            let auth = read_json(base("CODEX_HOME", ".codex").join("auth.json"))?;
            result.token = auth["tokens"]["access_token"].as_str().unwrap_or("").into();
            result.account = auth["tokens"]["account_id"].as_str().map(str::to_owned);
        }
        "claude" => {
            if let Some(token) = env_token(&["CLAUDE_CODE_OAUTH_TOKEN"]) {
                result.token = token;
            } else {
                let auth =
                    read_json(base("CLAUDE_CONFIG_DIR", ".claude").join(".credentials.json"))
                        .ok()
                        .or_else(|| {
                            if std::env::var_os("CLAUDE_CONFIG_DIR").is_none() {
                                claude_secure_store()
                            } else {
                                None
                            }
                        })
                        .ok_or("not_connected")?;
                let oauth = auth
                    .get("claudeAiOauth")
                    .or_else(|| auth.get("oauth"))
                    .unwrap_or(&auth);
                result.token = oauth["accessToken"].as_str().unwrap_or("").into();
                result.plan = string(oauth, &["subscriptionType"]);
            }
        }
        "opencode-go" => {
            let auth = read_json(base("XDG_DATA_HOME", ".local/share").join("opencode/auth.json"))?;
            if auth["opencode-go"]["type"] == "api" {
                result.token = auth["opencode-go"]["key"].as_str().unwrap_or("").into();
            }
        }
        "copilot" => {
            result.token =
                env_token(&["COPILOT_API_TOKEN", "GITHUB_COPILOT_TOKEN"]).unwrap_or_default()
        }
        "kimi" => {
            result.token = env_token(&["KIMI_CODE_API_KEY"])
                .or_else(|| extended::opencode_key(&["kimi-for-coding"]))
                .unwrap_or_default()
        }
        _ => result.token = extended::credential(provider).unwrap_or_default(),
    }
    if result.token.trim().is_empty() {
        Err("not_connected")
    } else {
        Ok(result)
    }
}
fn endpoint(provider: &str) -> Option<&'static str> {
    Some(match provider {
        "codex" => "https://chatgpt.com/backend-api/wham/usage",
        "claude" => "https://api.anthropic.com/api/oauth/usage",
        "copilot" => "https://api.github.com/copilot_internal/user",
        "opencode-go" => "https://opencode.ai/zen/go/v1/usage",
        "kimi" => "https://api.kimi.com/coding/v1/usages",
        _ => return extended::endpoint(provider),
    })
}
/// Call only with a current desktop-authorized connection. Failures contain a
/// fixed status code, never an HTTP body, path, header or reqwest error string.
pub fn fetch(provider: &str) -> SubscriptionUsage {
    let mut result = SubscriptionUsage::empty(provider, true);
    let run = || -> std::result::Result<SubscriptionUsage, &'static str> {
        if provider == "grok" {
            return grok::fetch();
        }
        if provider == "antigravity" {
            return google::fetch();
        }
        if matches!(provider, "factory" | "zed" | "stepfun") {
            return catalog::fetch(provider);
        }
        let endpoint = endpoint(provider).ok_or("unsupported")?;
        let auth = credential(provider)?;
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(4))
            .redirect(reqwest::redirect::Policy::none())
            .https_only(true)
            .build()
            .map_err(|_| "network_error")?;
        let mut request = client
            .get(endpoint)
            .header("Accept", "application/json")
            .header("User-Agent", "RepoAtlas/0.1");
        request = if provider == "cursor" {
            request.header("Cookie", format!("WorkosCursorSessionToken={}", auth.token))
        } else {
            request.bearer_auth(&auth.token)
        };
        if let Some(account) = auth.account {
            request = request.header("ChatGPT-Account-Id", account);
        }
        if provider == "claude" {
            request = request.header("anthropic-beta", "oauth-2025-04-20");
        }
        let response = request.send().map_err(|_| "network_error")?;
        match response.status().as_u16() {
            200..=299 => (),
            401 | 403 => return Err("login_required"),
            429 => return Err("rate_limited"),
            _ => return Err("unavailable"),
        }
        if response.content_length().is_some_and(|n| n > 1024 * 1024) {
            return Err("unsupported");
        }
        let mut bytes = vec![];
        response
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "network_error")?;
        if bytes.len() > 1024 * 1024 {
            return Err("unsupported");
        }
        let value: Value = serde_json::from_slice(&bytes).map_err(|_| "unsupported")?;
        // Several official usage endpoints return business failures with HTTP 200.
        // Keep diagnostics fixed and discard all credential-bearing response text.
        if matches!(provider, "zai" | "zhipu")
            && [401, 403].contains(&value["code"].as_i64().unwrap_or(0))
        {
            return Err("login_required");
        }
        if matches!(provider, "minimax" | "minimax-cn") {
            let code = value["base_resp"]["status_code"].as_i64().unwrap_or(0);
            if code == 1004 {
                return Err("login_required");
            }
            if code != 0 {
                return Err("unavailable");
            }
        }
        let mut parsed = parse(provider, &value).map_err(|_| "unsupported")?;
        parsed.plan = parsed.plan.or(auth.plan);
        Ok(parsed)
    };
    match run() {
        Ok(parsed) => result = parsed,
        Err(status) => result.status = status.into(),
    }
    result.attempted_at = Some(Utc::now().to_rfc3339());
    if result.status == "ready" {
        result.fetched_at = result.attempted_at.clone();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn missing_values_are_unknown_not_full_quota() {
        assert_eq!(
            parse(
                "codex",
                &json!({"rate_limit":{"primary_window":{"used_percent":null}}})
            )
            .unwrap()
            .status,
            "unsupported"
        );
        assert_eq!(
            parse("claude", &json!({"five_hour":{"utilization":0}}))
                .unwrap()
                .windows[0]
                .used_percent,
            Some(0.)
        );
    }
    #[test]
    fn provider_units_resets_and_unlimited_are_preserved() {
        let codex=parse("codex",&json!({"plan_type":"plus","rate_limit":{"primary_window":{"used_percent":25,"reset_at":1800000000,"limit_window_seconds":18000}}})).unwrap();
        assert_eq!(codex.windows[0].window_minutes, Some(300.));
        assert!(codex.windows[0].resets_at.is_some());
        let copilot=parse("copilot",&json!({"quota_reset_date":"2026-10-01","quota_snapshots":{"premium_interactions":{"entitlement":300,"remaining":240},"chat":{"unlimited":true}}})).unwrap();
        assert!(copilot.windows.iter().any(|w| w.used_percent == Some(20.)));
        assert!(copilot.windows.iter().any(|w| w.unlimited));
        let go = parse(
            "opencode-go",
            &json!({"usage":{"rolling":{"percent":75,"resetsAt":"2026-10-01T01:00:00Z"}}}),
        )
        .unwrap();
        assert_eq!(go.windows[0].used_percent, Some(75.));
        assert_eq!(go.windows[0].limit, None);
        let kimi=parse("kimi",&json!({"usage":{"used":30,"limit":100},"limits":[{"window":{"duration":300,"timeUnit":"TIME_UNIT_MINUTE"},"detail":{"used":20,"limit":100}}]})).unwrap();
        assert_eq!(kimi.windows.len(), 2);
    }
    #[test]
    fn additional_codex_buckets_do_not_replace_the_main_quota() {
        let parsed = parse(
            "codex",
            &json!({"rateLimitsByLimitId":{"codex":{},"other":{"primary":{"usedPercent":1}}}}),
        )
        .unwrap();
        assert!(parsed.windows.is_empty());
        assert!(endpoint("https://untrusted.test").is_none());
    }
    #[test]
    fn cache_and_retry_throttles_bound_manual_and_automatic_refresh() {
        let mut value = SubscriptionUsage::empty("codex", true);
        value.status = "ready".into();
        value.attempted_at = Some((Utc::now() - chrono::Duration::seconds(90)).to_rfc3339());
        assert!(value.fresh(false));
        assert!(!value.fresh(true));
        value.status = "network_error".into();
        assert!(!value.fresh(false));
        value.attempted_at = Some(Utc::now().to_rfc3339());
        assert!(value.fresh(true));
        value.attempted_at = Some((Utc::now() + chrono::Duration::minutes(2)).to_rfc3339());
        assert!(!value.fresh(false));
    }
}
