//! Additional read-only adapters. Region-specific credentials never cross hosts.
use super::*;
use serde_json::json;

pub(super) fn endpoint(provider: &str) -> Option<&'static str> {
    Some(match provider {
        "cursor" => "https://cursor.com/api/usage-summary",
        "zai" => "https://api.z.ai/api/monitor/usage/quota/limit",
        "zhipu" => "https://open.bigmodel.cn/api/monitor/usage/quota/limit",
        "minimax" => "https://api.minimax.io/v1/token_plan/remains",
        "minimax-cn" => "https://api.minimaxi.com/v1/token_plan/remains",
        "openrouter" => "https://openrouter.ai/api/v1/key",
        "deepseek" => "https://api.deepseek.com/user/balance",
        _ => return None,
    })
}

pub(super) fn opencode_key(ids: &[&str]) -> Option<String> {
    let auth = read_json(base("XDG_DATA_HOME", ".local/share").join("opencode/auth.json")).ok()?;
    ids.iter().find_map(|id| {
        let entry = &auth[id];
        (entry["type"] == "api")
            .then(|| {
                entry["key"]
                    .as_str()
                    .filter(|s| !s.trim().is_empty())
                    .map(str::to_owned)
            })
            .flatten()
    })
}

pub(super) fn credential(provider: &str) -> Option<String> {
    let (vars, ids): (&[&str], &[&str]) = match provider {
        "zai" => (
            &["ZAI_API_KEY", "Z_AI_API_KEY"],
            &["zai-coding-plan", "zai"],
        ),
        "zhipu" => (
            &["ZHIPU_API_KEY", "GLM_API_KEY"],
            &["zhipuai-coding-plan", "zhipuai"],
        ),
        "minimax" => (&["MINIMAX_CODING_API_KEY"], &["minimax-coding-plan"]),
        "minimax-cn" => (&["MINIMAX_CN_CODING_API_KEY"], &["minimax-cn-coding-plan"]),
        "openrouter" => (&["OPENROUTER_API_KEY"], &["openrouter"]),
        "deepseek" => (&["DEEPSEEK_API_KEY"], &["deepseek"]),
        "cursor" => {
            return env_token(&["CURSOR_SESSION_TOKEN"])
                .or_else(cursor_desktop_token)
                .and_then(cursor_cookie)
        }
        _ => return None,
    };
    env_token(vars).or_else(|| opencode_key(ids))
}

fn cursor_desktop_token() -> Option<String> {
    #[cfg(windows)]
    let root = base("APPDATA", "AppData/Roaming");
    #[cfg(target_os = "macos")]
    let root = home().join("Library/Application Support");
    #[cfg(not(any(windows, target_os = "macos")))]
    let root = base("XDG_CONFIG_HOME", ".config");
    let path = root.join("Cursor/User/globalStorage/state.vscdb");
    let meta = std::fs::symlink_metadata(&path).ok()?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return None;
    }
    let db =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .ok()?;
    db.busy_timeout(Duration::from_millis(250)).ok()?;
    db.query_row(
        "SELECT value FROM ItemTable WHERE key='cursorAuth/accessToken' AND length(value)<=16384",
        [],
        |row| row.get(0),
    )
    .ok()
}

fn cursor_cookie(token: String) -> Option<String> {
    use base64::Engine;
    let token = token.trim();
    if token.is_empty()
        || token.len() > 16384
        || token.chars().any(|c| c.is_whitespace() || c == ';')
    {
        return None;
    }
    if token.contains("%3A%3A") {
        return Some(token.into());
    }
    if token.contains("::") {
        return Some(token.replacen("::", "%3A%3A", 1));
    }
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(token.split('.').nth(1)?)
        .ok()?;
    let value: Value = serde_json::from_slice(&payload).ok()?;
    let sub = value["sub"].as_str()?;
    let user = sub.split('|').find(|s| s.starts_with("user_"))?;
    if !user.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    Some(format!("{user}%3A%3A{token}"))
}

pub(super) fn parse(provider: &str, value: &Value, result: &mut SubscriptionUsage) {
    match provider {
        "zai" | "zhipu" => {
            let data = &value["data"];
            result.plan = string(data, &["planName", "plan_name", "packageName"]);
            if let Some(limits) = data["limits"].as_array() {
                for (index, v) in limits.iter().take(16).enumerate() {
                    let kind = v["type"].as_str().unwrap_or("");
                    if !matches!(kind, "TOKENS_LIMIT" | "CREDIT_LIMIT" | "TIME_LIMIT") {
                        continue;
                    }
                    let limit = number(v, &["usage"]);
                    let used = number(v, &["currentValue", "current_value"]);
                    let remaining = number(v, &["remaining"]);
                    let used = match (used, limit.zip(remaining).map(|(l, r)| (l - r).max(0.))) {
                        (Some(a), Some(b)) => Some(a.max(b)),
                        (a, b) => a.or(b),
                    };
                    let mut w = window(
                        &format!("quota:{index}"),
                        if kind == "TIME_LIMIT" {
                            "MCP"
                        } else {
                            "rolling"
                        },
                        &json!({"limit":limit,"used":used,"used_percent":if limit.is_some_and(|l| l>0.) && used.is_some() {None} else {number(v,&["percentage","usedPercent","used_percent"])}}),
                        v.get("nextResetTime").or_else(|| v.get("next_reset_time")),
                    );
                    if kind != "TIME_LIMIT" {
                        w.window_minutes = number(v, &["number"])
                            .zip(number(v, &["unit"]))
                            .and_then(|(n, u)| match u as u64 {
                                5 => Some(n),
                                3 => Some(n * 60.),
                                1 => Some(n * 1440.),
                                6 => Some(n * 10080.),
                                _ => None,
                            });
                    }
                    result.windows.push(w);
                }
            }
        }
        "minimax" | "minimax-cn" => {
            let rows = value["data"]["model_remains"]
                .as_array()
                .or_else(|| value["model_remains"].as_array());
            if let Some(v) = rows.and_then(|r| r.iter().find(|r| r["model_name"] == "general")) {
                for (id, field, status, reset, minutes) in [
                    (
                        "rolling",
                        "current_interval_remaining_percent",
                        "current_interval_status",
                        "end_time",
                        300.,
                    ),
                    (
                        "weekly",
                        "current_weekly_remaining_percent",
                        "current_weekly_status",
                        "weekly_end_time",
                        10080.,
                    ),
                ] {
                    let Some(remaining) = number(v, &[field]) else {
                        continue;
                    };
                    if number(v, &[status]) == Some(3.) && remaining >= 100. {
                        continue;
                    }
                    let mut w = window(
                        id,
                        id,
                        &json!({"used_percent":100.-remaining.clamp(0.,100.)}),
                        v.get(reset),
                    );
                    w.window_minutes = Some(minutes);
                    result.windows.push(w);
                }
            }
        }
        "cursor" => {
            result.plan = string(value, &["membershipType"]);
            let plan = &value["individualUsage"]["plan"];
            for (field, label) in [
                ("autoPercentUsed", "Cursor Models"),
                ("apiPercentUsed", "Other Models"),
            ] {
                if let Some(percent) = number(plan, &[field]) {
                    result.windows.push(window(
                        field,
                        label,
                        &json!({"used_percent":percent}),
                        value.get("billingCycleEnd"),
                    ));
                }
            }
            if result.windows.is_empty() {
                let mut w = window("plan", "monthly", plan, value.get("billingCycleEnd"));
                w.used_percent = number(plan, &["totalPercentUsed"]).or(w.used_percent);
                // Cursor's raw monetary amounts are cents, not dollars.
                money_cents(&mut w);
                result.windows.push(w);
            }
            for (scope, key, label) in [
                ("individualUsage", "onDemand", "on_demand"),
                ("teamUsage", "pooled", "team_pool"),
            ] {
                let mut w = window(key, label, &value[scope][key], value.get("billingCycleEnd"));
                money_cents(&mut w);
                result.windows.push(w);
            }
        }
        "openrouter" => {
            let data = &value["data"];
            let period = data["limit_reset"].as_str().unwrap_or("");
            let field = match period {
                "daily" => "usage_daily",
                "weekly" => "usage_weekly",
                "monthly" => "usage_monthly",
                _ => "usage",
            };
            let limit = number(data, &["limit"]);
            let remaining = number(data, &["limit_remaining"]);
            let used = if let Some((l, r)) = limit.zip(remaining) {
                Some((l - r).max(0.))
            } else {
                let used = number(data, &[field]);
                if data["include_byok_in_limit"] == true {
                    used.zip(number(data, &[&format!("byok_{field}")]))
                        .map(|(a, b)| a + b)
                } else {
                    used
                }
            };
            let mut w = window(
                "key",
                if period.is_empty() {
                    "key_limit"
                } else {
                    period
                },
                &json!({"limit":limit,"used":used,"remaining":remaining}),
                None,
            );
            w.currency = Some("USD".into());
            result.windows.push(w);
        }
        "deepseek" => {
            if let Some(rows) = value["balance_infos"].as_array() {
                for row in rows.iter().take(4) {
                    let Some(currency) = row["currency"]
                        .as_str()
                        .filter(|c| matches!(*c, "USD" | "CNY"))
                    else {
                        continue;
                    };
                    let Some(amount) = number(row, &["total_balance"]) else {
                        continue;
                    };
                    let mut w = window(currency, "balance", &json!({"remaining":amount}), None);
                    w.currency = Some(currency.into());
                    result.windows.push(w);
                }
            }
        }
        _ => (),
    }
}
fn money_cents(w: &mut UsageWindow) {
    w.currency = Some("USD".into());
    w.used = w.used.map(|n| n / 100.);
    w.limit = w.limit.map(|n| n / 100.);
    w.remaining = w.remaining.map(|n| n / 100.);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adapters_preserve_provider_units_and_do_not_invent_missing_quota() {
        let z=super::super::parse("zhipu",&json!({"data":{"limits":[{"type":"TOKENS_LIMIT","usage":100,"remaining":60,"currentValue":50,"unit":6,"number":1,"nextResetTime":1800000000000i64}]}})).unwrap();
        assert_eq!(z.windows[0].used_percent, Some(50.));
        assert_eq!(z.windows[0].window_minutes, Some(10080.));
        assert!(z.windows[0].resets_at.as_ref().unwrap().starts_with("2027"));
        let m=super::super::parse("minimax",&json!({"data":{"model_remains":[{"model_name":"speech","current_interval_remaining_percent":0},{"model_name":"general","current_interval_remaining_percent":"75","current_weekly_remaining_percent":"100","current_weekly_status":3}]}})).unwrap();
        assert_eq!(m.windows.len(), 1);
        assert_eq!(m.windows[0].used_percent, Some(25.));
        let c = super::super::parse(
            "cursor",
            &json!({"membershipType":"pro","individualUsage":{"plan":{"used":500,"limit":2000}}}),
        )
        .unwrap();
        assert_eq!(c.windows[0].used, Some(5.));
        assert_eq!(c.windows[0].used_percent, Some(25.));
        let d = super::super::parse(
            "deepseek",
            &json!({"balance_infos":[{"currency":"CNY","total_balance":"12.50"}]}),
        )
        .unwrap();
        assert_eq!(d.windows[0].remaining, Some(12.5));
        assert_eq!(d.windows[0].used_percent, None);
        for provider in ["cursor", "zai", "minimax", "openrouter", "deepseek"] {
            assert_eq!(
                super::super::parse(provider, &json!({})).unwrap().status,
                "unsupported"
            );
        }
    }
    #[test]
    fn openrouter_period_and_byok_are_not_lifetime_spend() {
        let r=super::super::parse("openrouter",&json!({"data":{"limit":20,"usage":1000,"usage_daily":3,"limit_reset":"daily","include_byok_in_limit":true,"byok_usage_daily":2}})).unwrap();
        assert_eq!(r.windows[0].used_percent, Some(25.));
        assert_eq!(r.windows[0].resets_at, None);
        let r=super::super::parse("openrouter",&json!({"data":{"limit":20,"limit_remaining":19,"usage_daily":3,"limit_reset":"daily"}})).unwrap();
        assert_eq!(r.windows[0].used, Some(1.));
    }
    #[test]
    fn cursor_cookie_and_region_allowlist_are_bounded() {
        assert_eq!(
            cursor_cookie("user_123::token".into()).as_deref(),
            Some("user_123%3A%3Atoken")
        );
        assert!(cursor_cookie("user_123::token;other=x".into()).is_none());
        assert_ne!(endpoint("minimax"), endpoint("minimax-cn"));
        assert!(endpoint("https://evil.test").is_none());
    }
}
