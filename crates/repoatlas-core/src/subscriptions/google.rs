//! Read-only Antigravity quota RPCs. Only owned listener ports of the running
//! Antigravity language server are queried; no OAuth refresh, login or model call.
use super::*;
use serde_json::json;
use std::time::Instant;

fn fraction(value: &Value) -> Option<f64> {
    number(value, &["remainingFraction"])
        .or_else(|| number(&value["remaining"], &["remainingFraction"]))
        .or_else(|| {
            (value["remaining"]["case"] == "remainingFraction")
                .then(|| number(&value["remaining"], &["value"]))
                .flatten()
        })
        .filter(|n| *n <= 1.)
}
fn append_window(
    result: &mut SubscriptionUsage,
    id: &str,
    label: &str,
    bucket: &Value,
    minutes: Option<f64>,
) {
    if bucket["disabled"] == true {
        return;
    }
    let Some(remaining) = fraction(bucket) else {
        return;
    };
    let mut w = window(
        id,
        label,
        &json!({"used_percent":(1.-remaining)*100.}),
        None,
    );
    w.resets_at = time(&bucket["resetTime"]);
    w.window_minutes = minutes;
    result.windows.push(w);
}
fn parse(value: &Value) -> SubscriptionUsage {
    let mut result = SubscriptionUsage::empty("antigravity", true);
    let status = &value["userStatus"];
    result.plan = string(&status["userTier"], &["name"]).or_else(|| {
        string(
            &status["planStatus"]["planInfo"],
            &[
                "planDisplayName",
                "displayName",
                "productName",
                "planName",
                "planShortName",
            ],
        )
    });
    let summary = value
        .get("response")
        .or_else(|| value.get("summary"))
        .unwrap_or(value);
    if let Some(groups) = summary["groups"].as_array() {
        for (group_index, group) in groups.iter().take(32).enumerate() {
            let name = string(group, &["displayName"]).unwrap_or_else(|| "Quota".into());
            for (bucket_index, bucket) in group["buckets"]
                .as_array()
                .into_iter()
                .flatten()
                .take(8)
                .enumerate()
            {
                let kind = ["window", "bucketId", "displayName"]
                    .iter()
                    .filter_map(|k| bucket[k].as_str())
                    .map(|s| {
                        s.to_lowercase()
                            .replace('_', "-")
                            .trim_end_matches(" limit")
                            .to_owned()
                    })
                    .find_map(|s| {
                        if s == "weekly" || s.ends_with("-weekly") {
                            Some(("weekly", 10080.))
                        } else if ["session", "5h", "5-hour", "five hour", "five-hour"]
                            .iter()
                            .any(|a| s == *a || s.ends_with(&format!("-{a}")))
                        {
                            Some(("5h", 300.))
                        } else {
                            None
                        }
                    });
                if let Some((kind, minutes)) = kind {
                    append_window(
                        &mut result,
                        &format!("{group_index}:{bucket_index}"),
                        &format!("{name} {kind}"),
                        bucket,
                        Some(minutes),
                    );
                }
            }
        }
    }
    // Older desktop versions expose per-model pools in GetUserStatus.
    if result.windows.is_empty() {
        let configs = status["cascadeModelConfigData"]["clientModelConfigs"]
            .as_array()
            .or_else(|| value["clientModelConfigs"].as_array());
        for (i, model) in configs.into_iter().flatten().take(64).enumerate() {
            let id = string(&model["modelOrAlias"], &["model"]).unwrap_or_else(|| i.to_string());
            let name = string(model, &["label"]).unwrap_or_else(|| id.clone());
            append_window(&mut result, &id, &name, &model["quotaInfo"], None);
        }
    }
    result.status = if result.windows.is_empty() {
        "unsupported"
    } else {
        "ready"
    }
    .into();
    result
}
fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .enumerate()
        .find_map(|(i, arg)| {
            if arg == name {
                args.get(i + 1).cloned()
            } else {
                arg.strip_prefix(&format!("{name}=")).map(str::to_owned)
            }
        })
        .filter(|v| !v.is_empty() && v.len() <= 16384 && !v.contains(['\r', '\n']))
}
fn is_server(executable: &str, args: &[String]) -> bool {
    let path = executable.replace('\\', "/").to_lowercase();
    let name = path.rsplit('/').next().unwrap_or("");
    let language = name.starts_with("language_server") || name.starts_with("language-server");
    let known_path = path.contains("/antigravity/")
        || path.contains("/antigravity.app/")
        || path.contains("/antigravity ide.app/")
        || path.contains("/antigravity-ide/");
    (language && known_path)
        || (matches!(
            name,
            "antigravity-cli" | "antigravity-cli.exe" | "agy" | "agy.exe"
        ) && args.iter().any(|a| a == "--hub"))
}
fn response_json(
    response: reqwest::blocking::Response,
) -> std::result::Result<Value, &'static str> {
    if !response.status().is_success() {
        return Err("unavailable");
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
pub(super) fn fetch() -> std::result::Result<SubscriptionUsage, &'static str> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_cmd(UpdateKind::Always)
            .with_exe(UpdateKind::Always),
    );
    let servers: Vec<_> = system
        .processes()
        .iter()
        .filter_map(|(pid, p)| {
            let executable = p.exe()?.to_string_lossy();
            let args: Vec<_> = p
                .cmd()
                .iter()
                .map(|s| s.to_string_lossy().into_owned())
                .collect();
            if !is_server(&executable, &args) {
                return None;
            }
            let csrf = flag(&args, "--csrf_token")?;
            Some((
                pid.as_u32(),
                csrf,
                flag(&args, "--hub-port").and_then(|v| v.parse::<u16>().ok()),
            ))
        })
        .take(4)
        .collect();
    if servers.is_empty() {
        return Err("client_not_running");
    }
    let listeners = crate::broker::listening_ports().ok_or("unavailable")?;
    // Native RPC certificates are self-signed. This separate client has no proxy,
    // redirects or remote URLs, and receives only process-local CSRF credentials.
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(900))
        .connect_timeout(Duration::from_millis(350))
        .danger_accept_invalid_certs(true)
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "network_error")?;
    let deadline = Instant::now() + Duration::from_secs(8);
    for (pid, csrf, hub) in servers {
        let mut ports: Vec<_> = listeners
            .iter()
            .filter(|(_, owner)| *owner == pid)
            .map(|(port, _)| *port)
            .collect();
        ports.sort_by_key(|p| (Some(*p) != hub, *p));
        ports.dedup();
        for port in ports.into_iter().take(5) {
            for scheme in ["https", "http"] {
                let url = format!(
                    "{scheme}://127.0.0.1:{port}/exa.language_server_pb.LanguageServerService/"
                );
                let call =
                    |method: &str, body: Value| -> std::result::Result<Value, &'static str> {
                        let remaining = deadline.saturating_duration_since(Instant::now());
                        if remaining.is_zero() {
                            return Err("unavailable");
                        }
                        response_json(
                            client
                                .post(format!("{url}{method}"))
                                .timeout(remaining.min(Duration::from_millis(900)))
                                .header("x-codeium-csrf-token", &csrf)
                                .header("Connect-Protocol-Version", "1")
                                .json(&body)
                                .send()
                                .map_err(|_| "unavailable")?,
                        )
                    };
                if Instant::now() >= deadline {
                    return Err("unavailable");
                }
                let summary = call("RetrieveUserQuotaSummary", json!({"forceRefresh":true}));
                // An unsupported RPC can still be an older server with user-status quotas.
                if let Ok(summary) = summary {
                    let mut parsed = parse(&summary);
                    if parsed.status == "ready" {
                        if let Ok(identity) = call(
                            "GetUserStatus",
                            json!({"metadata":{"ideName":"antigravity","extensionName":"antigravity"}}),
                        ) {
                            parsed.plan = parse(&identity).plan;
                        }
                        return Ok(parsed);
                    }
                }
                if let Ok(identity) = call(
                    "GetUserStatus",
                    json!({"metadata":{"ideName":"antigravity","extensionName":"antigravity"}}),
                ) {
                    let parsed = parse(&identity);
                    if parsed.status == "ready" {
                        return Ok(parsed);
                    }
                }
            }
        }
    }
    Err("unavailable")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grouped_google_quotas_preserve_reset_and_unknown() {
        let r = parse(
            &json!({"groups":[{"displayName":"Gemini","buckets":[{"window":"session","remainingFraction":0.25,"resetTime":"2026-10-01T08:00:00Z"},{"window":"weekly","remaining":{"case":"remainingFraction","value":0.8}},{"window":"session"},{"window":"weekly","remainingFraction":0,"disabled":true}]}]}),
        );
        assert_eq!(r.windows.len(), 2);
        assert_eq!(r.windows[0].used_percent, Some(75.));
        assert_eq!(r.windows[0].window_minutes, Some(300.));
        assert!(r.windows[0].resets_at.is_some());
        assert_eq!(parse(&json!({})).status, "unsupported");
    }
    #[test]
    fn legacy_model_pools_and_plan() {
        let r = parse(
            &json!({"userStatus":{"userTier":{"name":"Google AI Ultra"},"cascadeModelConfigData":{"clientModelConfigs":[{"modelOrAlias":{"model":"gemini-pro"},"label":"Gemini Pro","quotaInfo":{"remainingFraction":0.5}},{"modelOrAlias":{"model":"unknown"}}]}}}),
        );
        assert_eq!(r.plan.as_deref(), Some("Google AI Ultra"));
        assert_eq!(r.windows.len(), 1);
        assert_eq!(r.windows[0].used_percent, Some(50.));
    }
    #[test]
    fn only_known_server_binaries_and_flag_forms() {
        assert!(is_server(
            "C:\\Apps\\Antigravity\\extensions\\antigravity\\bin\\language_server_windows_x64.exe",
            &[]
        ));
        assert!(!is_server(
            "C:\\Apps\\Other\\language_server.exe",
            &["antigravity".into()]
        ));
        assert!(!is_server("/tmp/antigravity/helper.exe", &[]));
        assert_eq!(
            flag(&["--csrf_token=token".into()], "--csrf_token").as_deref(),
            Some("token")
        );
        assert_eq!(
            flag(&["--csrf_token".into(), "token".into()], "--csrf_token").as_deref(),
            Some("token")
        );
    }
}
