# Observe Agent usage without owning accounts

Status: Accepted

## Decision

Session token usage is extracted alongside visible conversation text during the
existing authorized, fingerprinted refresh. Core stores compact recorded counts
in Session metadata; missing measurements remain unknown. Cache and reasoning
subsets must never be counted twice. No tokenizer estimation, guessed model rates,
filesystem watcher or extra transcript scan is introduced by rendering a badge.

This deliberately extends ADR 0022/0027 for read-only subscription observation.
Desktop users may separately enable an allowlisted provider's usage connection.
That choice permits reading its existing local login reference and sending that
credential only to that provider's fixed HTTPS usage endpoint. History-source
authorization alone never enables account reads. Credentials stay in memory, never
cross IPC, never enter SQLite, audit or logs, and are not refreshed or rewritten.
RepoAtlas does not add login, account switching, model execution or billing actions.
Missing/expired credentials direct the user back to the external Agent.

Antigravity is a deliberately narrow local transport exception: after separate
enablement, Core identifies the installed client's language-server/CLI-hub process,
its CSRF token and listeners owned by that PID. Only fixed read-only quota/status
RPCs may be sent to `127.0.0.1` on those listeners. Local self-signed TLS is accepted
only by this loopback-only client, with redirects and proxies disabled and bounded
discovery/requests. No arbitrary endpoint, Google OAuth flow or token refresh is
added. A stopped client has an explicit unavailable state.

Core owns provider response normalization, authorization and sanitized snapshot
persistence. Desktop schedules bounded network work outside the main Core lock.
Requests coalesce per provider, cache successes for five minutes, and throttle
explicit retries. Old snapshots remain visible with their timestamps after a
failure; expired windows are not presented as a fresh zero. Network work happens
only for explicitly connected accounts in a visible desktop window or explicit
refresh. The persistent title-bar readout starts with local metadata, schedules its
first request after paint and refreshes every five minutes while visible. A surface
starts at most three requests concurrently; Core coalesces each provider's work.
Opening a Project or Session adds no account query or transcript scan. MCP exposes Session token metadata
through its existing authorized reads, but no subscription credential/network API.

Per-provider navigation visibility is a local AppSettings preference in the existing
settings store, exported and restored with appearance preferences. Its dedicated
typed setter updates only that key; it cannot authorize or disconnect an account.

## References and limits

The provider shapes and counting semantics were researched in the MIT-licensed
[Token Monitor](https://github.com/Javis603/token-monitor), particularly
`src/shared/providers/{codex,claude,copilot,kimi}/limits.js`,
`src/shared/providers/opencode/goApi.js` and `src/shared/sessionDetail.js`.
Implementations are local Rust adapters. Unofficial usage endpoints can change;
unsupported responses, missing fields and login failures are visible states.
Local history is not an invoice and a quota percentage is not a token count.

Additional adapters use Token Monitor's Cursor, Z.ai/Zhipu, MiniMax, OpenRouter and
DeepSeek response contracts. Regional connections have separate allowlisted hosts
and credentials; API balances keep their original currency. Cursor reads only its
known application SQLite key in read-only mode, never browser cookie databases.
Providers without a verified adapter are not offered as functioning connections.

## API-equivalent estimates

Core prices recorded requests using an offline, dated catalogue of verified official
standard API rates. It accounts for input/cache/output subsets and request-level
long-context tiers before aggregating into metadata. Repeated cumulative snapshots
and streamed assistant blocks are deduplicated. A changed model never reprices the
whole session at its last model's rate. Missing model/counters, imported cumulative
gaps and unrecognized model identifiers remain unpriced, with coverage shown.
Unknown cache split/TTL assumptions are disclosed; tool fees, cache storage, taxes,
subscription charges and reseller markups are excluded. No pricing network lookup
or model call is added. Updating this catalogue bumps the session parser fingerprint
so the next authorized manual refresh rebuilds estimates. See [usage](../usage.md).

Conversation rendering uses the same safe, bounded reader in Sessions and search.
Known client envelopes are folded, Markdown is rendered without executable HTML
or automatically fetching images, and original text stays available for inspection
and verbatim copying. This is deterministic presentation, not AI summarization.
