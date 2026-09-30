//! Recorded token accounting. Input includes cache reads/writes; reasoning is an
//! output subset. Components are optional, so an absent measurement is not zero.
use super::pricing::{self, CostEstimate};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    pub total_tokens: u64,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub model: Option<String>,
    pub partial: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<CostEstimate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write_hour_tokens: Option<u64>,
}

fn count(v: &Value, names: &[&str]) -> Option<u64> {
    names
        .iter()
        .find_map(|n| v.get(n)?.as_u64())
        .filter(|n| *n <= 9_007_199_254_740_991)
}
fn sum(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    if a.is_none() && b.is_none() {
        None
    } else {
        Some(a.unwrap_or(0).saturating_add(b.unwrap_or(0)))
    }
}
fn normalize(v: &Value, adapter: &str) -> Option<TokenUsage> {
    let mut input = count(
        v,
        &[
            "input_tokens",
            "inputTokens",
            "input",
            "input_other",
            "promptTokenCount",
            "prompt_tokens",
        ],
    );
    let output = count(
        v,
        &[
            "output_tokens",
            "outputTokens",
            "output",
            "candidatesTokenCount",
            "completion_tokens",
        ],
    );
    let cache_read = count(
        v,
        &[
            "cached_input_tokens",
            "cache_read_input_tokens",
            "input_cache_read",
            "cached",
            "cachedContentTokenCount",
        ],
    )
    .or_else(|| count(&v["cache"], &["read"]))
    .or_else(|| count(&v["input_tokens_details"], &["cached_tokens"]));
    let cache_write = count(v, &["cache_creation_input_tokens", "input_cache_creation"])
        .or_else(|| count(&v["cache"], &["write"]));
    let reasoning = count(
        v,
        &[
            "reasoning_output_tokens",
            "reasoning",
            "thoughts",
            "thoughtsTokenCount",
        ],
    );
    if matches!(adapter, "claude" | "opencode")
        || (adapter == "kimi" && v.get("input_other").is_some())
    {
        input = sum(sum(input, cache_read), cache_write);
    }
    let total = count(
        v,
        &["total_tokens", "totalTokens", "total", "totalTokenCount"],
    )
    .or_else(|| {
        sum(input, output).map(|total| {
            if adapter == "gemini" {
                total.saturating_add(reasoning.unwrap_or(0))
            } else {
                total
            }
        })
    });
    total.map(|total_tokens| TokenUsage {
        total_tokens,
        input_tokens: input,
        output_tokens: if adapter == "gemini" {
            sum(output, reasoning)
        } else {
            output
        },
        cache_read_tokens: cache_read,
        cache_write_tokens: cache_write,
        reasoning_tokens: reasoning,
        model: None,
        partial: input.is_none() || output.is_none(),
        cost: None,
        cache_write_hour_tokens: count(&v["cache_creation"], &["ephemeral_1h_input_tokens"]),
    })
}

#[derive(Default)]
pub struct UsageAccumulator {
    requests: HashMap<String, TokenUsage>,
    cumulative: Option<TokenUsage>,
    model: Option<String>,
    sequence: usize,
    cost: CostEstimate,
}
impl UsageAccumulator {
    pub fn observe(&mut self, adapter: &str, v: &Value) {
        let p = v.get("payload").unwrap_or(v);
        let message = v.get("message").unwrap_or(v);
        let mut observed_model = None;
        for candidate in [
            p.get("model"),
            message.get("model"),
            v.get("modelID"),
            v["data"].get("model"),
        ] {
            if let Some(model) = candidate.and_then(Value::as_str).filter(|s| !s.is_empty()) {
                self.model = Some(model.chars().take(120).collect());
                observed_model = self.model.clone();
            }
        }
        if adapter == "codex" {
            if v["type"] != "event_msg" || p["type"] != "token_count" {
                return;
            }
            // A session total is a snapshot, never an increment. Idle/repeated
            // token_count events must not charge the previous turn again.
            if let Some(usage) = normalize(&p["info"]["total_token_usage"], adapter) {
                if self
                    .cumulative
                    .as_ref()
                    .is_none_or(|old| usage.total_tokens >= old.total_tokens)
                {
                    let old = self.cumulative.as_ref();
                    let diff = |now: Option<u64>, before: Option<u64>| {
                        now.and_then(|n| n.checked_sub(before.unwrap_or(0)))
                    };
                    let delta = TokenUsage {
                        total_tokens: usage.total_tokens - old.map_or(0, |o| o.total_tokens),
                        input_tokens: diff(usage.input_tokens, old.and_then(|o| o.input_tokens)),
                        output_tokens: diff(usage.output_tokens, old.and_then(|o| o.output_tokens)),
                        cache_read_tokens: diff(
                            usage.cache_read_tokens,
                            old.and_then(|o| o.cache_read_tokens),
                        ),
                        cache_write_tokens: diff(
                            usage.cache_write_tokens,
                            old.and_then(|o| o.cache_write_tokens),
                        ),
                        model: self.model.clone(),
                        ..Default::default()
                    };
                    // Imported/forked history may start with a cumulative total
                    // larger than its first request. Do not price that gap with
                    // the current turn's model or apply a session-wide context tier.
                    let request_total = count(&p["info"]["last_token_usage"], &["total_tokens"]);
                    if request_total.is_some_and(|last| last != delta.total_tokens)
                        && delta.total_tokens > 0
                    {
                        self.cost.unpriced_tokens =
                            self.cost.unpriced_tokens.saturating_add(delta.total_tokens);
                    } else {
                        self.cost.add(&pricing::estimate(&delta));
                    }
                    self.cumulative = Some(usage);
                }
            }
            return;
        }
        let usage = match adapter {
            "claude" | "qwen" if message["role"] == "assistant" || v["type"] == "assistant" => {
                message.get("usage").or_else(|| v.get("usage"))
            }
            "gemini" if v["type"] == "gemini" || v["role"] == "assistant" => {
                v.get("tokens").or_else(|| v.get("usageMetadata"))
            }
            "opencode" if v["role"] == "assistant" => v.get("tokens"),
            "copilot" if v["type"] == "assistant.usage" => v.get("data"),
            "kimi" if message["type"] == "StatusUpdate" => {
                message.get("payload").and_then(|p| p.get("token_usage"))
            }
            "kimi" if message["role"] == "assistant" => message.get("usage"),
            _ => None,
        };
        let Some(mut usage) = usage.and_then(|u| normalize(u, adapter)) else {
            return;
        };
        usage.model = observed_model;
        let id = message
            .get("id")
            .or_else(|| v.get("requestId"))
            .or_else(|| v.get("id"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| {
                self.sequence += 1;
                format!("record:{}", self.sequence)
            });
        // Streaming text/tool blocks share an assistant ID. Keep the largest
        // recorded usage for that request instead of charging every block.
        if self
            .requests
            .get(&id)
            .is_none_or(|old| usage.total_tokens >= old.total_tokens)
        {
            self.requests.insert(id, usage);
        }
    }
    pub fn finish(mut self) -> Option<TokenUsage> {
        if let Some(mut usage) = self.cumulative {
            usage.model = self.model;
            self.cost.price_date = pricing::PRICE_DATE.into();
            usage.cost = Some(self.cost);
            return Some(usage);
        }
        if self.requests.is_empty() {
            return None;
        }
        let mut total = TokenUsage::default();
        let mut cost = CostEstimate::default();
        for usage in self.requests.into_values() {
            cost.add(&pricing::estimate(&usage));
            total.total_tokens = total.total_tokens.saturating_add(usage.total_tokens);
            total.input_tokens = sum(total.input_tokens, usage.input_tokens);
            total.output_tokens = sum(total.output_tokens, usage.output_tokens);
            total.cache_read_tokens = sum(total.cache_read_tokens, usage.cache_read_tokens);
            total.cache_write_tokens = sum(total.cache_write_tokens, usage.cache_write_tokens);
            total.reasoning_tokens = sum(total.reasoning_tokens, usage.reasoning_tokens);
            total.partial |= usage.partial;
        }
        total.model = self.model;
        total.cost = Some(cost);
        Some(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn codex_snapshots_are_not_added_and_subsets_are_not_double_counted() {
        let mut usage = UsageAccumulator::default();
        for total in [110, 110, 220] {
            usage.observe("codex", &json!({"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":total-10,"cached_input_tokens":50,"output_tokens":10,"reasoning_output_tokens":5,"total_tokens":total}}}}));
        }
        let usage = usage.finish().unwrap();
        assert_eq!(usage.total_tokens, 220);
        assert_eq!(usage.input_tokens, Some(210));
        assert_eq!(usage.reasoning_tokens, Some(5));
    }
    #[test]
    fn claude_counts_cache_once_and_deduplicates_streaming_blocks() {
        let mut usage = UsageAccumulator::default();
        for output in [5, 5, 10] {
            usage.observe("claude", &json!({"type":"assistant","message":{"id":"m1","usage":{"input_tokens":20,"cache_read_input_tokens":100,"cache_creation_input_tokens":30,"output_tokens":output}}}));
        }
        let usage = usage.finish().unwrap();
        assert_eq!(usage.total_tokens, 160);
        assert_eq!(usage.input_tokens, Some(150));
    }
    #[test]
    fn missing_and_partial_measurements_are_distinct_from_zero() {
        assert!(UsageAccumulator::default().finish().is_none());
        let mut usage = UsageAccumulator::default();
        usage.observe("gemini", &json!({"type":"gemini","tokens":{"total":0}}));
        let usage = usage.finish().unwrap();
        assert_eq!(usage.total_tokens, 0);
        assert!(usage.partial);
    }
    #[test]
    fn model_switches_price_each_request_not_the_final_model() {
        let mut usage = UsageAccumulator::default();
        for (model, total) in [("gpt-5.4", 1100), ("gpt-6-astra", 2200)] {
            usage.observe(
                "codex",
                &json!({"type":"turn_context","payload":{"model":model}}),
            );
            for _ in 0..2 {
                usage.observe("codex",&json!({"type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"total_tokens":1100},"total_token_usage":{"total_tokens":total,"input_tokens":total/11*10,"output_tokens":total/11,"cached_input_tokens":total/11*5}}}}));
            }
        }
        let cost = usage.finish().unwrap().cost.unwrap();
        assert_eq!(cost.priced_tokens, 2200);
        assert!((cost.usd - 0.013375).abs() < 1e-9);
    }
    #[test]
    fn missing_model_or_imported_cumulative_gap_is_unpriced() {
        let mut usage = UsageAccumulator::default();
        for (id, model) in [("a", Some("claude-sonnet-4-6")), ("b", None)] {
            usage.observe("claude",&json!({"type":"assistant","message":{"id":id,"role":"assistant","model":model,"usage":{"input_tokens":100,"output_tokens":20}}}));
        }
        let cost = usage.finish().unwrap().cost.unwrap();
        assert_eq!(cost.priced_tokens, 120);
        assert_eq!(cost.unpriced_tokens, 120);
        let mut usage = UsageAccumulator::default();
        usage.observe(
            "codex",
            &json!({"type":"turn_context","payload":{"model":"gpt-5.4"}}),
        );
        usage.observe("codex",&json!({"type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"total_tokens":120},"total_token_usage":{"input_tokens":1000,"output_tokens":100,"total_tokens":1100}}}}));
        let cost = usage.finish().unwrap().cost.unwrap();
        assert_eq!(cost.priced_tokens, 0);
        assert_eq!(cost.unpriced_tokens, 1100);
        assert_eq!(cost.price_date, pricing::PRICE_DATE);
    }
}
