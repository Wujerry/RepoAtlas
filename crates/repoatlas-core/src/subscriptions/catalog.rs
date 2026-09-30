//! Additional fixed-endpoint contracts verified against Token Monitor.
use super::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use reqwest::blocking::{Client, RequestBuilder};
use serde_json::json;

fn cookie(raw: &str, names: &[&str], required: &str) -> Option<String> {
    if raw.len() > 16384 || raw.chars().any(char::is_control) {
        return None;
    }
    let mut pairs = vec![];
    let mut found = false;
    for part in raw.trim().strip_prefix("Cookie:").unwrap_or(raw).split(';') {
        let Some((key, value)) = part.trim().split_once('=') else {
            continue;
        };
        if names.contains(&key) && !value.trim().is_empty() {
            pairs.push(format!("{key}={}", value.trim()));
            found |= key == required;
        }
    }
    found.then(|| pairs.join("; "))
}
fn request(request: RequestBuilder) -> std::result::Result<Value, &'static str> {
    let response = request.send().map_err(|_| "network_error")?;
    match response.status().as_u16() {
        200..=299 => (),
        401 | 403 => return Err("login_required"),
        429 => return Err("rate_limited"),
        _ => return Err("unavailable"),
    }
    let mut bytes = vec![];
    response
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "network_error")?;
    if bytes.len() > 1024 * 1024 {
        return Err("unsupported");
    }
    serde_json::from_slice(&bytes).map_err(|_| "unsupported")
}
fn stepfun_device_id(token: &str) -> String {
    for jwt in token.rsplit("...") {
        let Some(payload) = jwt.split('.').nth(1) else {
            continue;
        };
        let Some(value) = URL_SAFE_NO_PAD
            .decode(payload)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        else {
            continue;
        };
        if let Some(id) = value["device_id"].as_str().filter(|id| {
            !id.is_empty()
                && id.len() <= 128
                && id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        }) {
            return id.to_owned();
        }
    }
    "c8a1002d2c457e758785a9979832217c7c0b884c".into()
}
pub(super) fn fetch(provider: &str) -> std::result::Result<SubscriptionUsage, &'static str> {
    let client = Client::builder()
        .timeout(Duration::from_secs(8))
        .connect_timeout(Duration::from_secs(3))
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("RepoAtlas/0.1")
        .build()
        .map_err(|_| "network_error")?;
    let value = match provider {
        "factory" => {
            let token = env_token(&["FACTORY_API_KEY"]).ok_or("not_connected")?;
            request(
                client
                    .get("https://api.factory.ai/api/billing/limits")
                    .bearer_auth(token)
                    .header("x-factory-client", "web-app")
                    .header("Origin", "https://app.factory.ai")
                    .header("Referer", "https://app.factory.ai/"),
            )?
        }
        "zed" => {
            let raw =
                env_token(&["ZED_COOKIE", "TOKEN_MONITOR_ZED_COOKIE"]).ok_or("not_connected")?;
            let token = cookie(
                &raw,
                &["zed.session", "c15t", "__cf_bm", "cf_clearance"],
                "zed.session",
            )
            .ok_or("not_connected")?;
            let mut usage = request(
                client
                    .get("https://cloud.zed.dev/frontend/billing/usage")
                    .header("Cookie", &token),
            )?;
            // A plan response is optional; usage still works if only this endpoint is unavailable.
            if let Ok(plan) = request(
                client
                    .get("https://cloud.zed.dev/frontend/billing/subscriptions/current")
                    .timeout(Duration::from_secs(2))
                    .header("Cookie", token),
            ) {
                usage["subscription"] = plan["subscription"].clone();
            }
            usage
        }
        "stepfun" => {
            let raw = env_token(&["STEPFUN_TOKEN", "TOKEN_MONITOR_STEPFUN_TOKEN"])
                .ok_or("not_connected")?;
            let token = if raw.contains("Oasis-Token=") {
                cookie(&raw, &["Oasis-Token"], "Oasis-Token")
                    .and_then(|v| v.strip_prefix("Oasis-Token=").map(str::to_owned))
            } else {
                Some(raw)
            }
            .ok_or("not_connected")?;
            if token.len() > 16384 || token.chars().any(|c| c.is_control() || c == ';') {
                return Err("not_connected");
            }
            let webid = stepfun_device_id(&token);
            let mut value=request(client.post("https://platform.stepfun.com/api/step.openapi.devcenter.Dashboard/QueryStepPlanRateLimit")
                .header("oasis-appid","10300").header("oasis-platform","web").header("oasis-webid",&webid)
                .header("Cookie",format!("Oasis-Token={token}; Oasis-Webid={webid}")).json(&json!({})))?;
            if value["status"] != 1 {
                return Err("unavailable");
            }
            if let Ok(plan)=request(client.post("https://platform.stepfun.com/api/step.openapi.devcenter.Dashboard/GetStepPlanStatus")
                .timeout(Duration::from_secs(2)).header("oasis-appid","10300").header("oasis-platform","web").header("oasis-webid",&webid)
                .header("Cookie",format!("Oasis-Token={token}; Oasis-Webid={webid}")).json(&json!({}))){value["subscription"]=plan["subscription"].clone();}
            value
        }
        _ => return Err("unsupported"),
    };
    Ok(parse(provider, &value))
}
fn parse(provider: &str, value: &Value) -> SubscriptionUsage {
    let mut result = SubscriptionUsage::empty(provider, true);
    match provider {
        "factory" if value["usesTokenRateLimitsBilling"] == true => {
            for pool in ["standard", "core"] {
                for (key, label, minutes) in [
                    ("fiveHour", "5h", Some(300.)),
                    ("weekly", "weekly", Some(10080.)),
                    ("monthly", "monthly", None),
                ] {
                    let bucket = &value["limits"][pool][key];
                    if number(bucket, &["usedPercent"]).is_none() {
                        continue;
                    }
                    let mut w = window(
                        &format!("{pool}:{key}"),
                        &format!("{pool} {label}"),
                        bucket,
                        None,
                    );
                    w.window_minutes = minutes;
                    w.resets_at = number(bucket, &["secondsRemaining"])
                        .and_then(|n| {
                            Utc::now().checked_add_signed(chrono::Duration::seconds(
                                n.min(31_536_000.) as i64,
                            ))
                        })
                        .map(|d| d.to_rfc3339())
                        .or_else(|| time(&bucket["windowEnd"]));
                    result.windows.push(w);
                }
            }
            if let Some(cents) = number(value, &["extraUsageBalanceCents"]) {
                let mut w = window("balance", "balance", &json!({"remaining":cents/100.}), None);
                w.currency = Some("USD".into());
                result.windows.push(w);
            }
        }
        "zed" => {
            result.plan =
                string(&value["subscription"], &["name"]).or_else(|| string(value, &["plan"]));
            let usage = &value["current_usage"];
            let spend = &usage["token_spend"];
            if let Some((used, limit)) = number(usage, &["token_spend_in_cents"])
                .or_else(|| number(spend, &["spend_in_cents"]))
                .zip(number(spend, &["limit_in_cents"]))
            {
                let mut w = window(
                    "spend",
                    "Token spend",
                    &json!({"used":used/100.,"limit":limit/100.}),
                    Some(&value["subscription"]["period"]["end_at"]),
                );
                w.currency = Some("USD".into());
                result.windows.push(w);
            }
            let prediction = &usage["edit_predictions"];
            if prediction.is_object() {
                let mut w = window("predictions", "Edit predictions", prediction, None);
                w.unlimited = prediction.get("limit").is_some_and(Value::is_null);
                result.windows.push(w);
            }
        }
        "stepfun" if value["status"] == 1 => {
            result.plan = string(&value["subscription"], &["name"]);
            for (prefix, label, minutes) in
                [("five_hour", "rolling", 300.), ("weekly", "weekly", 10080.)]
            {
                let left =
                    number(value, &[&format!("{prefix}_usage_left_rate")]).filter(|n| *n <= 1.);
                if let Some(left) = left {
                    let mut w = window(
                        prefix,
                        label,
                        &json!({"used_percent":(1.-left)*100.}),
                        Some(&value[format!("{prefix}_usage_reset_time")]),
                    );
                    w.window_minutes = Some(minutes);
                    result.windows.push(w);
                }
            }
            if result.windows.is_empty() {
                let credit = &value["plan_credit_rate_limit"];
                let left = if let Some(buckets) = credit["credit_buckets"]
                    .as_array()
                    .filter(|b| !b.is_empty())
                {
                    buckets
                        .iter()
                        .try_fold((0., 0.), |(sum, remaining), b| {
                            let total = number(b, &["credit_total"])?;
                            let left = number(b, &["credit_residual"])?;
                            (total > 0. && left <= total).then_some((sum + total, remaining + left))
                        })
                        .map(|(total, left)| left / total)
                } else {
                    number(
                        credit,
                        &["subscription_credit_left_rate", "topup_credit_left_rate"],
                    )
                    .filter(|n| *n <= 1.)
                };
                if let Some(left) = left {
                    result.windows.push(window(
                        "credits",
                        "credits",
                        &json!({"used_percent":(1.-left)*100.}),
                        Some(&credit["subscription_credit_reset_time"]),
                    ));
                }
            }
        }
        _ => (),
    }
    result.windows.retain(|w| {
        w.used_percent.is_some()
            || w.unlimited
            || w.currency.is_some() && (w.used.is_some() || w.remaining.is_some())
    });
    result.status = if result.windows.is_empty() {
        "unsupported"
    } else {
        "ready"
    }
    .into();
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stepfun_device_identity_is_bounded_and_header_safe() {
        let token = |id: &str| {
            format!(
                "header.{}.signature",
                URL_SAFE_NO_PAD.encode(json!({"device_id": id}).to_string())
            )
        };
        assert_eq!(
            stepfun_device_id(&format!("{}...{}", token("first"), token("device_2-a"))),
            "device_2-a"
        );
        for id in ["", "bad;cookie=value", "bad\r\nheader", &"a".repeat(129)] {
            assert_eq!(
                stepfun_device_id(&token(id)),
                stepfun_device_id("malformed")
            );
        }
    }
    #[test]
    fn factory_periods_keep_observed_expired_usage() {
        let r = parse(
            "factory",
            &json!({"usesTokenRateLimitsBilling":true,"limits":{"standard":{"fiveHour":{"usedPercent":95,"windowEnd":"2020-01-01T00:00:00Z"},"weekly":{"usedPercent":30,"secondsRemaining":60}}},"extraUsageBalanceCents":150}),
        );
        assert_eq!(r.windows.len(), 3);
        assert_eq!(r.windows[0].used_percent, Some(95.));
        assert_eq!(r.windows[2].remaining, Some(1.5));
    }
    #[test]
    fn zed_money_and_unlimited_are_distinct() {
        let r = parse(
            "zed",
            &json!({"current_usage":{"token_spend_in_cents":250,"token_spend":{"limit_in_cents":1000},"edit_predictions":{"limit":null}},"subscription":{"name":"pro","period":{"end_at":"2026-10-01T00:00:00Z"}}}),
        );
        assert_eq!(r.windows[0].used, Some(2.5));
        assert_eq!(r.windows[0].used_percent, Some(25.));
        assert!(r.windows[1].unlimited);
    }
    #[test]
    fn stepfun_credit_and_rolling_contracts() {
        let r = parse(
            "stepfun",
            &json!({"status":1,"five_hour_usage_left_rate":0.6,"weekly_usage_left_rate":0.2,"five_hour_usage_reset_time":1800000000}),
        );
        assert_eq!(r.windows.len(), 2);
        assert_eq!(r.windows[0].used_percent, Some(40.));
        let r = parse(
            "stepfun",
            &json!({"status":1,"plan_credit_rate_limit":{"credit_buckets":[{"credit_total":100,"credit_residual":20}]}}),
        );
        assert_eq!(r.windows[0].used_percent, Some(80.));
    }
    #[test]
    fn unrelated_cookies_and_empty_quotas_are_rejected() {
        assert!(cookie("other=secret", &["zed.session"], "zed.session").is_none());
        assert_eq!(
            cookie(
                "zed.session=ok; other=secret",
                &["zed.session"],
                "zed.session"
            )
            .as_deref(),
            Some("zed.session=ok")
        );
        for provider in ["factory", "zed", "stepfun"] {
            assert_eq!(parse(provider, &json!({})).status, "unsupported");
        }
    }
}
