//! Grok Build credits, following Token Monitor's gRPC-web billing contract.
use super::*;
use serde_json::json;
const ENDPOINT: &str = "https://grok.com/grok_api_v2.GrokBuildBilling/GetGrokCreditsConfig";

fn auth_entry(root: &Value) -> Option<String> {
    let entries = root.as_object()?;
    entries
        .iter()
        .filter(|(scope, _)| scope.starts_with("https://auth.x.ai::"))
        .chain(
            entries
                .iter()
                .filter(|(scope, _)| scope.as_str() == "https://accounts.x.ai/sign-in"),
        )
        .find_map(|(_, entry)| {
            entry["key"]
                .as_str()
                .filter(|s| !s.trim().is_empty())
                .map(str::to_owned)
        })
}
fn grpc_status(value: &str) -> std::result::Result<(), &'static str> {
    match value.trim() {
        "" | "0" => Ok(()),
        "7" | "16" => Err("login_required"),
        "8" => Err("rate_limited"),
        _ => Err("unavailable"),
    }
}
pub(super) fn fetch() -> std::result::Result<SubscriptionUsage, &'static str> {
    let token = env_token(&["GROK_BEARER_TOKEN"])
        .or_else(|| {
            read_json(base("GROK_HOME", ".grok").join("auth.json"))
                .ok()
                .and_then(|v| auth_entry(&v))
        })
        .ok_or("not_connected")?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(4))
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "network_error")?;
    let response = client
        .post(ENDPOINT)
        .bearer_auth(token)
        .header("X-XAI-Token-Auth", "xai-grok-cli")
        .header("Content-Type", "application/grpc-web+proto")
        .header("User-Agent", "Grok Build")
        .header("Origin", "https://grok.com")
        .header("Referer", "https://grok.com/?_s=usage")
        .header("x-grpc-web", "1")
        .header("x-user-agent", "connect-es/2.1.1")
        .body(vec![0u8; 5])
        .send()
        .map_err(|_| "network_error")?;
    match response.status().as_u16() {
        200..=299 => (),
        401 | 403 => return Err("login_required"),
        429 => return Err("rate_limited"),
        _ => return Err("unavailable"),
    }
    grpc_status(
        response
            .headers()
            .get("grpc-status")
            .and_then(|v| v.to_str().ok())
            .unwrap_or(""),
    )?;
    let mut bytes = vec![];
    response
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "network_error")?;
    if bytes.len() > 1024 * 1024 {
        return Err("unsupported");
    }
    parse_frames(&bytes)
}
#[derive(Default)]
struct Fields {
    floats: Vec<(Vec<u64>, f32)>,
    ints: Vec<(Vec<u64>, u64)>,
}
fn varint(bytes: &[u8], offset: &mut usize) -> Option<u64> {
    let mut value = 0u64;
    for shift in (0..=63).step_by(7) {
        let byte = *bytes.get(*offset)?;
        *offset += 1;
        if shift == 63 && byte > 1 {
            return None;
        }
        value |= u64::from(byte & 127) << shift;
        if byte & 128 == 0 {
            return Some(value);
        }
    }
    None
}
fn scan(bytes: &[u8], path: &[u64], fields: &mut Fields) {
    if path.len() > 8 || fields.ints.len() + fields.floats.len() > 4096 {
        return;
    }
    let mut offset = 0;
    while offset < bytes.len() && fields.ints.len() + fields.floats.len() < 4096 {
        let Some(key) = varint(bytes, &mut offset) else {
            return;
        };
        if key >> 3 == 0 {
            return;
        }
        let mut next = path.to_vec();
        next.push(key >> 3);
        match key & 7 {
            0 => {
                let Some(value) = varint(bytes, &mut offset) else {
                    return;
                };
                fields.ints.push((next, value));
            }
            1 => {
                offset = offset.saturating_add(8);
            }
            2 => {
                let Some(length) = varint(bytes, &mut offset).and_then(|n| usize::try_from(n).ok())
                else {
                    return;
                };
                let Some(end) = offset.checked_add(length).filter(|e| *e <= bytes.len()) else {
                    return;
                };
                scan(&bytes[offset..end], &next, fields);
                offset = end;
            }
            5 => {
                let Some(value) = bytes.get(offset..offset + 4) else {
                    return;
                };
                fields
                    .floats
                    .push((next, f32::from_le_bytes(value.try_into().unwrap())));
                offset += 4;
            }
            _ => return,
        }
    }
}
fn parse_frames(bytes: &[u8]) -> std::result::Result<SubscriptionUsage, &'static str> {
    let mut offset = 0;
    let mut fields = Fields::default();
    while offset < bytes.len() {
        let header = bytes.get(offset..offset + 5).ok_or("unsupported")?;
        let length = u32::from_be_bytes(header[1..5].try_into().unwrap()) as usize;
        offset += 5;
        let end = offset
            .checked_add(length)
            .filter(|e| *e <= bytes.len())
            .ok_or("unsupported")?;
        if header[0] == 128 {
            for line in String::from_utf8_lossy(&bytes[offset..end]).lines() {
                if let Some((name, value)) = line.split_once(':') {
                    if name.trim().eq_ignore_ascii_case("grpc-status") {
                        grpc_status(value)?;
                    }
                }
            }
        } else if header[0] == 0 {
            scan(&bytes[offset..end], &[], &mut fields);
        } else {
            return Err("unsupported");
        }
        offset = end;
    }
    // Field 1 in the credits config is a float percentage, already on a 0–100 scale.
    let percent = fields
        .floats
        .iter()
        .filter(|(p, n)| p.last() == Some(&1) && n.is_finite() && (0. ..=100.).contains(n))
        .min_by_key(|(p, _)| p.len())
        .map(|(_, n)| *n as f64);
    let timestamp = |path: &[u64]| {
        fields
            .ints
            .iter()
            .find(|(p, n)| p == path && (1_700_000_000..=2_100_000_000).contains(n))
            .map(|(_, n)| *n)
    };
    let start = timestamp(&[1, 4, 1]);
    let reset = timestamp(&[1, 5, 1]);
    // Proto default zero is legitimate only when a recognized quota period exists.
    let explicit_period = fields
        .ints
        .iter()
        .any(|(p, n)| p.starts_with(&[1, 6]) || (p == &[1, 8, 1] && matches!(n, 1 | 2)));
    let percent = percent
        .or_else(|| (fields.floats.is_empty() && reset.is_some() && explicit_period).then_some(0.))
        .ok_or("unsupported")?;
    let mut w = window("credits", "credits", &json!({"used_percent":percent}), None);
    w.resets_at = reset.and_then(|n| time(&json!(n)));
    w.window_minutes = start
        .zip(reset)
        .filter(|(s, e)| e > s)
        .map(|(s, e)| (e - s) as f64 / 60.);
    w.label = match w.window_minutes {
        Some(n) if (10000. ..=10200.).contains(&n) => "weekly",
        Some(n) if n > 40000. => "monthly",
        _ => "credits",
    }
    .into();
    let mut result = SubscriptionUsage::empty("grok", true);
    result.status = "ready".into();
    result.windows.push(w);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(body: &[u8], kind: u8) -> Vec<u8> {
        let mut v = vec![kind];
        v.extend((body.len() as u32).to_be_bytes());
        v.extend(body);
        v
    }
    #[test]
    fn grpc_percent_and_trailer_errors() {
        let mut payload = vec![13];
        payload.extend(71.5f32.to_le_bytes());
        let bytes = frame(&payload, 0);
        let result = parse_frames(&bytes).unwrap();
        assert_eq!(result.windows[0].used_percent, Some(71.5));
        assert!(result.windows[0].resets_at.is_none());
        let mut error = bytes;
        error.extend(frame(b"grpc-status: 16\r\ngrpc-message: secret", 128));
        assert_eq!(parse_frames(&error).unwrap_err(), "login_required");
        assert!(parse_frames(&[0, 0, 255, 255, 255]).is_err());
        assert!(parse_frames(&frame(&[], 0)).is_err());
    }
    #[test]
    fn auth_does_not_select_unrelated_scopes() {
        assert_eq!(
            auth_entry(&json!({"https://evil.test":{"key":"secret"}})),
            None
        );
        assert_eq!(auth_entry(&json!({"https://accounts.x.ai/sign-in":{"key":"legacy"},"https://auth.x.ai::client":{"key":"oidc"}})).as_deref(),Some("oidc"));
    }
}
