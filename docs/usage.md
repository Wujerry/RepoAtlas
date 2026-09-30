# Usage sources and API-equivalent pricing

## Subscription observations

The title bar shows each connected provider's most constrained quota when its
navigation switch is enabled, without a provider-count limit. If space runs out,
scroll the quota rail horizontally with the wheel/trackpad or focus it and use the
arrow keys; the other toolbar actions remain fixed. Percentages mean **remaining**; meters
mean **used**. A reset countdown is the service's timestamp, not a promise that a
local timer will refill an allowance. Expired observations keep their old values
and display a cached/pending state until a successful response arrives.

| Connection | Query | Windows / units |
| --- | --- | --- |
| Codex | `chatgpt.com/backend-api/wham/usage` | Provider rate-limit windows and plan |
| Claude | `api.anthropic.com/api/oauth/usage` | Five-hour, weekly and model-specific windows |
| GitHub Copilot | `api.github.com/copilot_internal/user` | Premium interactions, chat, completions |
| OpenCode Go | `opencode.ai` Go usage API | Short, weekly and monthly limits |
| Kimi Code | `api.kimi.com/coding/v1/usages` | Available rolling/weekly limits |
| Cursor | `cursor.com/api/usage-summary` | Model pools, plan/on-demand/team usage; cents converted to USD |
| Z.ai | `api.z.ai/api/monitor/usage/quota/limit` | Coding credit/token periods and MCP allowance |
| GLM Coding Plan | `open.bigmodel.cn/api/monitor/usage/quota/limit` | Regional coding periods and MCP allowance |
| MiniMax Global | `api.minimax.io/v1/token_plan/remains` | General coding bucket, five-hour and weekly |
| MiniMax CN | `api.minimaxi.com/v1/token_plan/remains` | Separate regional five-hour and weekly quotas |
| OpenRouter | `openrouter.ai/api/v1/key` | API-key spend/cap, original daily/weekly/monthly period; no invented reset timestamp |
| DeepSeek | `api.deepseek.com/user/balance` | API balance in USD/CNY; no invented quota percentage |
| Grok | `grok.com/grok_api_v2.GrokBuildBilling/GetGrokCreditsConfig` | gRPC-web current credit period and reset |
| Google Antigravity | Identified local language server's `RetrieveUserQuotaSummary` / `GetUserStatus` | Five-hour/weekly quota groups or legacy model pools |
| Factory | `api.factory.ai/api/billing/limits` | Standard/core token-limit periods and extra USD balance |
| Zed | `cloud.zed.dev/frontend/billing/usage` and optional current subscription | USD spend, edit predictions and plan period |
| StepFun | `platform.stepfun.com` Dashboard `QueryStepPlanRateLimit` and optional `GetStepPlanStatus` | Five-hour/weekly or credit pools |

Credential sources are listed in the [README](../README.md#token-usage-and-subscription-limits)
and disclosed by the connection action. Enablement is separate from history-source
authorization. Credentials are read only for enabled connections, sent only to their
fixed HTTPS endpoint (or the identified Antigravity client's loopback RPC), never persisted/copied across IPC, and never refreshed by
RepoAtlas. Cursor reads one key from its known SQLite application store in read-only
mode. MiniMax/Z.ai regional credentials do not cross regional hosts.

Adapters were independently implemented in Rust using the MIT-licensed
[Token Monitor](https://github.com/Javis603/token-monitor) provider contracts, including
its `src/shared/providers` limit parsers. Some usage endpoints are unofficial and
can change. Missing logins, expired credentials, unsupported response shapes and
network failures remain distinct visible states. Google support is specifically
Antigravity: its signed-in IDE language server or CSRF-enabled CLI hub must be
running. RepoAtlas does not add Google OAuth or refresh tokens. Standalone Gemini,
Google One and Qwen account queries are not implemented; local transcript support
does not imply an account quota connection. Other plans may expose fewer windows.

The seventeen connections are a subset of Token Monitor's catalogue, not full
provider parity. Cline, Command Code, MiMo, Z.ai Team, Kiro, WorkBuddy, Qoder, Devin,
TypeSafe, Volcengine, Ollama, Trae, Alibaba and custom third-party connections are
not yet offered; OpenCode support currently covers Go. Factory's legacy billing
fallback and tokenless Antigravity CLI are also not supported. Account queries
remain read-only: no model probes, browser cookie scraping or account management.

Navigation visibility switches live on connected-service rows in the full Usage page.
The quota popover contains no settings switches; its management action opens that page.
Hiding a service does not disconnect it. An all-hidden readout keeps a compact Usage
button, so the full list and its settings remain reachable.

## Recorded tokens and price estimates

Counts come from authorized local transcript records, during the existing manual
incremental indexing pass. There is no tokenizer estimate or scan when rendering
rows. Input includes cache reads/writes; reasoning is an output subset. Cumulative
Codex snapshots and streamed assistant blocks are deduplicated. Missing counters
remain unknown, not zero.

Prices are **API-equivalent USD**, not the subscription bill or the provider's
reported actual spend. The offline catalogue was checked on **2026-09-30** against:

- [OpenAI standard API pricing](https://developers.openai.com/api/docs/pricing)
  and [GPT-6 Astra](https://developers.openai.com/api/docs/models/gpt-6-astra)
- [Anthropic model, cache and long-context pricing](https://platform.claude.com/docs/en/about-claude/pricing)
- [Gemini API pricing](https://ai.google.dev/gemini-api/docs/pricing)
- [Z.ai model pricing](https://docs.z.ai/guides/overview/pricing)
- [MiniMax pay-as-you-go pricing](https://platform.minimax.io/docs/guides/pricing-paygo)
- [Kimi chat pricing](https://platform.kimi.ai/docs/pricing/chat)

The exact supported model identifiers and rates live in
[`pricing.rs`](../crates/repoatlas-core/src/agent_sessions/pricing.rs).
Provider prefixes and dated snapshot suffixes are normalized; other model aliases
are not guessed. Model switches are priced request by request before aggregation.
For each complete request:

```
USD = (uncached_input * input_rate + cache_read * cache_read_rate
     + cache_write_5m * write_rate + cache_write_1h * hour_write_rate
     + output * output_rate) / 1_000_000
```

Request-level long-context multipliers apply to supported models. A known one-hour
cache creation count uses the corresponding multiplier. An unspecified cache write
TTL assumes five minutes; an unspecified cache read count assumes uncached input.
These assumptions are flagged in the breakdown. Gemini 3.6–3.8 Flash rates in this
snapshot include the advertised promotion through 2026-12-31; the displayed pricing
date makes the catalogue's age visible. Future rate changes require a catalogue
and parser fingerprint update followed by the user's authorized history refresh.

Unknown model/rates, missing or inconsistent components, and imported cumulative
gaps are **unpriced**, not billed as zero. The UI reports priced-token coverage and
labels partial totals. All-unpriced sessions show no monetary badge. A genuinely
zero-priced supported model may show `$0.00`. Tool fees, cache storage time, taxes,
priority/batch rates, subscription charges and reseller markups are excluded.

## Performance and refresh

- First paint uses SQLite metadata. Network work runs outside the main Core lock.
- The visible title bar delays its initial refresh, then refreshes enabled accounts
  every five minutes. Hidden windows pause scheduling.
- Each surface starts at most three queries concurrently; desktop coalesces each
  provider's concurrent requests across windows. Successes have a five-minute TTL,
  failures a one-minute TTL, and manual retries a 30-second floor.
- Disconnect changes the connection generation. A late request cannot repopulate a
  revoked/reconnected account. Backups omit connection authorization and snapshots.
- Session costs and aggregate totals use compact metadata; displaying them neither
  decodes transcripts nor probes Agent installations. Longer conversations retain
  bounded message pages and deferred code highlighting.

Adapter fixtures cover response normalization and failure states. Real account
availability still depends on the external tool's login and current provider API.
