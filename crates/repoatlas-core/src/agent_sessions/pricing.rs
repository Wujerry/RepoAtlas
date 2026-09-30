//! Offline API-equivalent estimates, never a subscription invoice.
//! Standard USD / 1M token rates verified 2026-09-30; see docs/usage.md.
use super::usage::TokenUsage;
use serde::{Deserialize, Serialize};

pub const PRICE_DATE: &str = "2026-09-30";
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CostEstimate {
    pub usd: f64,
    pub priced_tokens: u64,
    pub unpriced_tokens: u64,
    pub price_date: String,
    pub assumptions: bool,
}
impl CostEstimate {
    pub fn add(&mut self, value: &Self) {
        self.usd += value.usd;
        self.priced_tokens = self.priced_tokens.saturating_add(value.priced_tokens);
        self.unpriced_tokens = self.unpriced_tokens.saturating_add(value.unpriced_tokens);
        self.assumptions |= value.assumptions;
        self.price_date = PRICE_DATE.into();
    }
}

// Exact model names or dated snapshots only. Never guess the rate of a new model.
fn canonical(model: &str) -> String {
    let model = model
        .rsplit('/')
        .next()
        .unwrap_or(model)
        .to_ascii_lowercase();
    if !model.is_ascii() {
        return model;
    }
    if model.len() > 11
        && model.as_bytes()[model.len() - 11] == b'-'
        && chrono::NaiveDate::parse_from_str(&model[model.len() - 10..], "%Y-%m-%d").is_ok()
    {
        return model[..model.len() - 11].into();
    }
    if model.len() > 9
        && model.as_bytes()[model.len() - 9] == b'-'
        && model.as_bytes()[model.len() - 8..]
            .iter()
            .all(u8::is_ascii_digit)
    {
        return model[..model.len() - 9].into();
    }
    model
}
// input, cache read, 5-minute cache write, output, long-context threshold.
fn rates(model: &str) -> Option<(f64, f64, f64, f64, u64)> {
    Some(match model {
        "gpt-6-astra" => (10., 1., 12.5, 50., 272000),
        "gpt-6.1-sol" => (2., 0.1, 2.5, 10., 272000),
        "gpt-6-sol" => (2., 0.2, 2.5, 10., 272000),
        "gpt-6-luna" => (0.1, 0.01, 0.125, 0.5, 272000),
        "gpt-5.6-sol" => (4., 0.4, 5., 20., 272000),
        "gpt-5.6-terra" => (2., 0.2, 2., 12., 272000),
        "gpt-5.6-luna" => (0.2, 0.02, 0.2, 1.2, 272000),
        "gpt-5.5" => (5., 0.5, 5., 30., 272000),
        "gpt-5.4" => (2.5, 0.25, 2.5, 15., 272000),
        "gpt-5.4-mini" => (0.75, 0.075, 0.75, 4.5, u64::MAX),
        "gpt-5.4-nano" => (0.2, 0.02, 0.2, 1.25, u64::MAX),
        "gpt-5.3-codex" => (1.75, 0.175, 1.75, 14., u64::MAX),
        "claude-fable-5-1" | "claude-mythos-5-1" => (10., 0.25, 12.5, 50., u64::MAX),
        "claude-fable-5" | "claude-mythos-5" => (10., 1., 12.5, 50., u64::MAX),
        "claude-opus-5-5" => (4., 0.2, 5., 20., u64::MAX),
        "claude-opus-5" | "claude-opus-4-8" | "claude-opus-4-7" | "claude-opus-4-6"
        | "claude-opus-4-5" => (5., 0.5, 6.25, 25., u64::MAX),
        "claude-opus-4-1" | "claude-opus-4" => (15., 1.5, 18.75, 75., u64::MAX),
        "claude-sonnet-5-5" | "claude-sonnet-5" => (2., 0.2, 2.5, 10., u64::MAX),
        "claude-sonnet-4-6" => (3., 0.3, 3.75, 15., u64::MAX),
        "claude-sonnet-4-5" | "claude-sonnet-4" => (3., 0.3, 3.75, 15., 200000),
        "claude-haiku-4-5" => (1., 0.1, 1.25, 5., u64::MAX),
        "claude-3-5-haiku" => (0.8, 0.08, 1., 4., u64::MAX),
        "gemini-3.8-flash" | "gemini-3.7-flash" | "gemini-3.6-flash" => {
            (0.75, 0.075, 0.75, 3.75, u64::MAX)
        }
        "gemini-3.5-flash" => (1.5, 0.15, 1.5, 9., u64::MAX),
        "gemini-3.1-pro-preview" | "gemini-3.1-pro-preview-customtools" => {
            (2., 0.2, 2., 12., 200000)
        }
        "kimi-k3" => (3., 0.3, 3., 15., u64::MAX),
        "kimi-k2.7-code" => (0.95, 0.19, 0.95, 4., u64::MAX),
        "kimi-k2.7-code-highspeed" => (1.9, 0.38, 1.9, 8., u64::MAX),
        "kimi-k2.6" => (0.95, 0.16, 0.95, 4., u64::MAX),
        "glm-5.3-flash" => (0.15, 0.03, 0.15, 0.5, u64::MAX),
        "glm-5.3-flashx" => (0.37, 0.075, 0.37, 1.25, u64::MAX),
        "glm-5.3" | "glm-5.2" | "glm-5.1" => (1.4, 0.26, 1.4, 4.4, u64::MAX),
        "glm-5" => (1., 0.2, 1., 3.2, u64::MAX),
        "glm-4.7" | "glm-4.6" | "glm-4.5" => (0.6, 0.11, 0.6, 2.2, u64::MAX),
        "glm-4.7-flash" | "glm-4.5-flash" => (0., 0., 0., 0., u64::MAX),
        "minimax-m3" => (0.3, 0.06, 0.3, 1.2, 512000),
        "minimax-m2.7" => (0.3, 0.06, 0.375, 1.2, u64::MAX),
        "minimax-m2.7-highspeed" => (0.6, 0.06, 0.375, 2.4, u64::MAX),
        "minimax-m2.5" | "minimax-m2.1" | "minimax-m2" => (0.3, 0.03, 0.375, 1.2, u64::MAX),
        "minimax-m2.5-highspeed" | "minimax-m2.1-highspeed" => (0.6, 0.03, 0.375, 2.4, u64::MAX),
        _ => return None,
    })
}

pub fn estimate(usage: &TokenUsage) -> CostEstimate {
    let unknown = || CostEstimate {
        unpriced_tokens: usage.total_tokens,
        price_date: PRICE_DATE.into(),
        ..Default::default()
    };
    let Some(model) = usage.model.as_deref().map(canonical) else {
        return unknown();
    };
    let Some((input_rate, read_rate, write_rate, output_rate, threshold)) = rates(&model) else {
        return unknown();
    };
    let (Some(input), Some(output)) = (usage.input_tokens, usage.output_tokens) else {
        return unknown();
    };
    let read = usage.cache_read_tokens.unwrap_or(0);
    let write = usage.cache_write_tokens.unwrap_or(0);
    let hour = usage.cache_write_hour_tokens.unwrap_or(0);
    if usage.partial
        || input.checked_add(output) != Some(usage.total_tokens)
        || read.saturating_add(write) > input
        || hour > write
    {
        return unknown();
    }
    let long = input > threshold;
    let input_multiplier = if long { 2. } else { 1. };
    let output_multiplier = if long {
        if model == "minimax-m3" {
            2.
        } else {
            1.5
        }
    } else {
        1.
    };
    let cost = ((input - read - write) as f64 * input_rate
        + read as f64 * read_rate
        + (write - hour) as f64 * write_rate
        + hour as f64 * input_rate * 2.)
        * input_multiplier
        + output as f64 * output_rate * output_multiplier;
    CostEstimate {
        usd: cost / 1_000_000.,
        priced_tokens: usage.total_tokens,
        unpriced_tokens: 0,
        price_date: PRICE_DATE.into(),
        assumptions: usage.cache_read_tokens.is_none()
            || write > 0 && usage.cache_write_hour_tokens.is_none(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn usage(model: &str) -> TokenUsage {
        TokenUsage {
            model: Some(model.into()),
            total_tokens: 1100,
            input_tokens: Some(1000),
            output_tokens: Some(100),
            cache_read_tokens: Some(500),
            ..Default::default()
        }
    }
    #[test]
    fn cache_and_reasoning_subsets_and_unknown_models() {
        let mut u = usage("openai/gpt-5.4");
        u.reasoning_tokens = Some(50);
        assert!((estimate(&u).usd - 0.002875).abs() < 1e-9);
        u.model = Some("gpt-5.4-unreleased".into());
        assert_eq!(estimate(&u).unpriced_tokens, 1100);
        u.model = Some("gpt-5.4-2026-03-05".into());
        assert_eq!(estimate(&u).priced_tokens, 1100);
        u.cache_read_tokens = Some(1001);
        assert_eq!(estimate(&u).priced_tokens, 0);
    }
    #[test]
    fn long_context_and_hour_cache_writes_use_request_level_rates() {
        let mut u = usage("gpt-6-astra");
        u.input_tokens = Some(300000);
        u.output_tokens = Some(1000);
        u.total_tokens = 301000;
        u.cache_read_tokens = Some(200000);
        assert!((estimate(&u).usd - 2.475).abs() < 1e-9);
        let mut u = usage("claude-sonnet-4-6");
        u.cache_write_tokens = Some(300);
        u.cache_write_hour_tokens = Some(100);
        assert!((estimate(&u).usd - 0.0036).abs() < 1e-9);
    }
}
