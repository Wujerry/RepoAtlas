//! Read-only Antigravity quota RPCs. Only owned listener ports of the running
//! Antigravity language server are queried; no OAuth refresh, login or model call.
use super::*;
use serde_json::json;
use std::time::Instant;

// Deliberately no Debug: this in-memory observation includes the CSRF token.
#[derive(PartialEq, Eq)]
struct Server {
    pid: u32,
    started: u64,
    executable: std::path::PathBuf,
    csrf: String,
    hub: Option<u16>,
    #[cfg(windows)]
    native_identity: crate::port_processes::ProcessIdentity,
}

impl Server {
    fn observe(pid: u32) -> Option<Self> {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
        // Fence command-line reads with exact Windows creation identity, not
        // just a PID or sysinfo's second-resolution timestamp.
        #[cfg(windows)]
        let native_identity = crate::port_processes::native::inspect(pid).ok()?;
        let mut system = System::new();
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[sysinfo::Pid::from_u32(pid)]),
            true,
            ProcessRefreshKind::nothing()
                .with_cmd(UpdateKind::Always)
                .with_exe(UpdateKind::Always),
        );
        let process = system.process(sysinfo::Pid::from_u32(pid))?;
        let executable = process.exe()?.to_path_buf();
        let args: Vec<_> = process
            .cmd()
            .iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        if !is_server(&executable.to_string_lossy(), &args) || process.start_time() == 0 {
            return None;
        }
        #[cfg(windows)]
        if crate::port_processes::native::inspect(pid).ok()? != native_identity {
            return None;
        }
        Some(Self {
            pid,
            started: process.start_time(),
            executable,
            csrf: flag(&args, "--csrf_token")?,
            hub: flag(&args, "--hub-port").and_then(|v| v.parse::<u16>().ok()),
            #[cfg(windows)]
            native_identity,
        })
    }
}

#[derive(Clone, Copy)]
struct LoopbackEndpoint(u16);

impl LoopbackEndpoint {
    fn owned_by(
        listeners: &[crate::broker::ListeningEndpoint],
        pid: u32,
        port: u16,
    ) -> Option<Self> {
        if pid == 0 || port == 0 {
            return None;
        }
        let mut found = false;
        for listener in listeners.iter().filter(|listener| listener.port == port) {
            // The RPC destination is exactly 127.0.0.1, not any 127/8 address.
            // Same-owner dual-stack/address siblings are harmless, but cannot
            // independently authorize this IPv4 destination. Any different
            // owner on the port makes the observation ambiguous.
            if listener.pid != pid {
                return None;
            }
            found |= matches!(listener.address, std::net::IpAddr::V4(ip)
                if ip == std::net::Ipv4Addr::LOCALHOST || ip.is_unspecified());
        }
        found.then_some(Self(port))
    }

    fn recheck(self, server: &Server, deadline: Instant) -> bool {
        let Some(listeners) =
            crate::broker::listening_endpoints(deadline.saturating_duration_since(Instant::now()))
        else {
            return false;
        };
        Self::owned_by(&listeners, server.pid, self.0).is_some()
            && Server::observe(server.pid).as_ref() == Some(server)
    }
}

#[derive(Clone, Copy)]
enum LocalScheme {
    Https,
    Http,
}

#[derive(Clone, Copy)]
enum QuotaRpc {
    Summary,
    Status,
}

// Keep the intentional self-signed TLS exception private to this transport.
// Neither its URL nor RPC method can be supplied by a provider response.
struct LocalTransport(reqwest::blocking::Client);

impl LocalTransport {
    fn new() -> std::result::Result<Self, &'static str> {
        reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(900))
            .connect_timeout(Duration::from_millis(350))
            .danger_accept_invalid_certs(true)
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            // Every request uses the destination whose ownership was just
            // observed, rather than reusing an earlier pooled connection.
            .pool_max_idle_per_host(0)
            .build()
            .map(Self)
            .map_err(|_| "network_error")
    }

    fn call(
        &self,
        endpoint: LoopbackEndpoint,
        scheme: LocalScheme,
        rpc: QuotaRpc,
        csrf: &str,
        deadline: Instant,
        recheck: impl FnOnce() -> bool,
    ) -> std::result::Result<Value, &'static str> {
        if Instant::now() >= deadline || !recheck() {
            return Err("unavailable");
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("unavailable");
        }
        let scheme = match scheme {
            LocalScheme::Https => "https",
            LocalScheme::Http => "http",
        };
        let (method, body) = match rpc {
            QuotaRpc::Summary => ("RetrieveUserQuotaSummary", json!({"forceRefresh":true})),
            QuotaRpc::Status => (
                "GetUserStatus",
                json!({"metadata":{"ideName":"antigravity","extensionName":"antigravity"}}),
            ),
        };
        // Snapshot validation narrows, but cannot eliminate, the race between
        // OS ownership inspection and connect/send. This is not TLS identity
        // authentication; executable/CLI recognition is still heuristic.
        response_json(
            self.0
                .post(format!(
                    "{scheme}://127.0.0.1:{}/exa.language_server_pb.LanguageServerService/{method}",
                    endpoint.0
                ))
                .timeout(remaining.min(Duration::from_millis(900)))
                .header("x-codeium-csrf-token", csrf)
                .header("Connect-Protocol-Version", "1")
                .json(&body)
                .send()
                .map_err(|_| "unavailable")?,
        )
    }
}

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
    let deadline = Instant::now() + Duration::from_secs(8);
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
            Some(pid.as_u32())
        })
        .take(4)
        .filter_map(|pid| {
            (Instant::now() < deadline)
                .then(|| Server::observe(pid))
                .flatten()
        })
        .collect();
    if servers.is_empty() {
        return Err("client_not_running");
    }
    let listeners =
        crate::broker::listening_endpoints(deadline.saturating_duration_since(Instant::now()))
            .ok_or("unavailable")?;
    let client = LocalTransport::new()?;
    for server in servers {
        let mut ports: Vec<_> = listeners
            .iter()
            .filter(|listener| listener.pid == server.pid)
            .map(|listener| listener.port)
            .filter(|port| LoopbackEndpoint::owned_by(&listeners, server.pid, *port).is_some())
            .collect();
        ports.sort_by_key(|p| (Some(*p) != server.hub, *p));
        ports.dedup();
        for port in ports.into_iter().take(5) {
            let endpoint = LoopbackEndpoint(port);
            for scheme in [LocalScheme::Https, LocalScheme::Http] {
                let call = |rpc| {
                    client.call(endpoint, scheme, rpc, &server.csrf, deadline, || {
                        endpoint.recheck(&server, deadline)
                    })
                };
                if Instant::now() >= deadline {
                    return Err("unavailable");
                }
                let summary = call(QuotaRpc::Summary);
                // An unsupported RPC can still be an older server with user-status quotas.
                if let Ok(summary) = summary {
                    let mut parsed = parse(&summary);
                    if parsed.status == "ready" {
                        if let Ok(identity) = call(QuotaRpc::Status) {
                            parsed.plan = parse(&identity).plan;
                        }
                        return Ok(parsed);
                    }
                }
                if let Ok(identity) = call(QuotaRpc::Status) {
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

    fn listener(address: &str, port: u16, pid: u32) -> crate::broker::ListeningEndpoint {
        crate::broker::ListeningEndpoint {
            address: address.parse().unwrap(),
            port,
            pid,
        }
    }

    #[test]
    fn loopback_authorization_requires_ipv4_destination_and_unambiguous_owner() {
        for address in ["127.0.0.1", "0.0.0.0"] {
            let owned = listener(address, 5000, 42);
            assert!(LoopbackEndpoint::owned_by(&[owned], 42, 5000).is_some());
            assert!(LoopbackEndpoint::owned_by(&[owned, owned], 42, 5000).is_some());
            for sibling in ["::", "::1", "::ffff:127.0.0.1", "192.0.2.10", "127.0.0.2"] {
                assert!(LoopbackEndpoint::owned_by(
                    &[owned, listener(sibling, 5000, 42)],
                    42,
                    5000
                )
                .is_some());
            }
            // Different owners are conservatively rejected regardless of the
            // address/family, and regardless of enumeration order.
            for other in [
                "127.0.0.1",
                "0.0.0.0",
                "127.0.0.2",
                "192.0.2.10",
                "::",
                "::1",
            ] {
                let conflict = listener(other, 5000, 43);
                assert!(LoopbackEndpoint::owned_by(&[owned, conflict], 42, 5000).is_none());
                assert!(LoopbackEndpoint::owned_by(&[conflict, owned], 42, 5000).is_none());
                assert!(
                    LoopbackEndpoint::owned_by(&[owned, listener(other, 5001, 43)], 42, 5000)
                        .is_some()
                );
            }
        }
        for address in ["::", "::1", "::ffff:127.0.0.1", "192.0.2.10", "127.0.0.2"] {
            let wrong_address = listener(address, 5000, 42);
            assert!(LoopbackEndpoint::owned_by(&[wrong_address], 42, 5000).is_none());
            // Regression: genuine CSRF owner listens elsewhere on the port,
            // while a different process owns the actual 127.0.0.1 destination.
            assert!(LoopbackEndpoint::owned_by(
                &[wrong_address, listener("127.0.0.1", 5000, 99)],
                42,
                5000
            )
            .is_none());
        }
        assert!(LoopbackEndpoint::owned_by(&[], 42, 5000).is_none());
        assert!(LoopbackEndpoint::owned_by(&[listener("127.0.0.1", 0, 42)], 42, 0).is_none());
        assert!(LoopbackEndpoint::owned_by(&[listener("127.0.0.1", 5000, 0)], 0, 5000).is_none());
    }

    fn synthetic_server() -> Server {
        Server {
            pid: 42,
            started: 100,
            executable: "/synthetic/antigravity/language_server".into(),
            csrf: "synthetic-csrf".into(),
            hub: Some(5000),
            #[cfg(windows)]
            native_identity: crate::port_processes::ProcessIdentity {
                pid: 42,
                created_at: "1000000000".into(),
                executable: "/synthetic/antigravity/language_server".into(),
            },
        }
    }

    #[test]
    fn selected_process_identity_does_not_follow_pid_reuse_or_token_changes() {
        let original = synthetic_server();
        let mut changed = synthetic_server();
        assert!(Some(&original) == Some(&changed));
        changed.started += 1;
        assert!(original != changed);
        let mut changed = synthetic_server();
        changed.csrf = "replacement-synthetic-csrf".into();
        assert!(original != changed);
        let mut changed = synthetic_server();
        changed.executable = "/different/antigravity/language_server".into();
        assert!(original != changed);
        #[cfg(windows)]
        {
            let mut changed = synthetic_server();
            // Same second, different exact Windows process creation time.
            changed.native_identity.created_at = "1000000001".into();
            assert!(original != changed);
        }
    }

    fn serve_dummy_once(
        listener: std::net::TcpListener,
        response: String,
    ) -> std::thread::JoinHandle<String> {
        std::thread::spawn(move || {
            use std::io::Write;
            listener.set_nonblocking(true).unwrap();
            let deadline = Instant::now() + Duration::from_secs(3);
            let mut socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "dummy server was not contacted");
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("dummy accept failed: {error}"),
                }
            };
            socket.set_nonblocking(false).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            socket
                .set_write_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut buffer = [0; 1024];
                let read = socket.read(&mut buffer).unwrap();
                assert!(read > 0 && bytes.len() + read <= 8192);
                bytes.extend_from_slice(&buffer[..read]);
                if let Some(end) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.strip_prefix("content-length:")?
                                .trim()
                                .parse::<usize>()
                                .ok()
                        })
                        .unwrap_or(0);
                    if bytes.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            socket.write_all(response.as_bytes()).unwrap();
            String::from_utf8(bytes).unwrap()
        })
    }

    #[test]
    fn scoped_transport_rechecks_every_rpc_and_does_not_follow_redirects() {
        use std::net::TcpListener;
        let transport = LocalTransport::new().unwrap();
        let redirect_target = TcpListener::bind("127.0.0.1:0").unwrap();
        redirect_target.set_nonblocking(true).unwrap();
        for (rpc, method) in [
            (QuotaRpc::Summary, "RetrieveUserQuotaSummary"),
            (QuotaRpc::Status, "GetUserStatus"),
        ] {
            let socket = TcpListener::bind("127.0.0.1:0").unwrap();
            socket.set_nonblocking(true).unwrap();
            let port = socket.local_addr().unwrap().port();
            let initial = [listener("127.0.0.1", port, 42)];
            let endpoint = LoopbackEndpoint::owned_by(&initial, 42, port).unwrap();
            let deadline = Instant::now() + Duration::from_secs(3);
            // A conflicting owner appears after discovery. No TCP connection,
            // and therefore no synthetic credential, may reach the listener.
            let conflict = [initial[0], listener("0.0.0.0", port, 99)];
            assert!(transport
                .call(
                    endpoint,
                    LocalScheme::Http,
                    rpc,
                    "synthetic-csrf",
                    deadline,
                    || { LoopbackEndpoint::owned_by(&conflict, 42, port).is_some() }
                )
                .is_err());
            assert_eq!(
                socket.accept().unwrap_err().kind(),
                std::io::ErrorKind::WouldBlock
            );
            let response = format!(
                "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://127.0.0.1:{}/leak\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                redirect_target.local_addr().unwrap().port()
            );
            let server = serve_dummy_once(socket, response);
            let checked = std::cell::Cell::new(false);
            assert!(transport
                .call(
                    endpoint,
                    LocalScheme::Http,
                    rpc,
                    "synthetic-csrf",
                    deadline,
                    || {
                        checked.set(true);
                        true
                    }
                )
                .is_err());
            assert!(checked.get());
            let request = server.join().unwrap();
            assert!(request.starts_with(&format!(
                "POST /exa.language_server_pb.LanguageServerService/{method} HTTP/1.1\r\n"
            )));
            assert!(request
                .to_lowercase()
                .contains("x-codeium-csrf-token: synthetic-csrf\r\n"));
            assert_eq!(
                redirect_target.accept().unwrap_err().kind(),
                std::io::ErrorKind::WouldBlock
            );
        }
    }

    #[test]
    fn expired_transport_deadline_never_rechecks_or_connects() {
        let transport = LocalTransport::new().unwrap();
        let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        socket.set_nonblocking(true).unwrap();
        let result = transport.call(
            LoopbackEndpoint(socket.local_addr().unwrap().port()),
            LocalScheme::Http,
            QuotaRpc::Summary,
            "synthetic-csrf",
            Instant::now(),
            || panic!("an expired request must not inspect or send"),
        );
        assert!(result.is_err());
        assert_eq!(
            socket.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

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
