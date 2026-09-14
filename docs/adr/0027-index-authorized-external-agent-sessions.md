# Index authorized external Agent sessions and resume in the source Agent

Status: Accepted

## Decision

RepoAtlas adds local Sessions and Continue Coding without hosting a model,
conversation editor, provider account, generated summary or AI Memory. This extends
ADR 0022; external Agents continue to own conversation and execution.

A Session Source is an explicitly authorized absolute history directory, separate
from Scan Roots. Candidate discovery checks existence only. Enabling a source grants
local indexing and read-only MCP access to its entire history, including unlinked
sessions. MCP and historical text cannot grant new authorization. Revocation blocks
reads before deleting derived data; original histories are never changed.

Core owns adapters, authorization, association, indexing, search and resume specs.
Tauri schedules background parsing and index writes on a separate Core connection outside the desktop Core lock and launches explicit
desktop resume actions. MCP exposes bounded cached reads only: list_agent_sessions,
search_agent_sessions, get_agent_session and get_agent_session_messages. Returned
history is untrusted data, never instructions. Queries and text are not audit payloads.

Migration 22 stores metadata and published index revisions. An independent
session-index.sqlite stores visible user/assistant text with unicode61/trigram FTS5.
System, thinking and tool payloads are excluded. Index commits precede publication
of metadata revisions, so incomplete generations do not become search results.
The index is rebuildable and excluded from Project exports. Database backups disable
source authorization and omit cached session metadata, requiring reauthorization
and reindexing when restored.

Refresh compares file fingerprints including SQLite WAL state. It shows cache first,
coalesces requests, and limits automatic entry refresh to once per 60 seconds.
Manual refresh bypasses that interval; a forced request during indexing schedules another pass. Finished-file checkpoints are published only after all sessions in that file complete. Cancellation and failures preserve cache;
only complete enumeration marks missing sources. No filesystem watcher, symlink
traversal, credential read or Project discovery is introduced.

Manual associations precede exact paths and deepest managed Project/Module ancestors.
Workspace hashes may match already managed canonical paths. Repository Lineage and
directory basenames never choose another checkout automatically. Explicit replacement
cwd survives refresh. Resume uses a validated ID in the original Agent and cwd;
missing sources, directories and binaries report errors rather than opening a new
conversation. Custom sources must preserve a recognized Agent directory layout to resume with the corresponding data-home environment; otherwise they remain searchable with an explicit resume-unavailable reason. Launch success describes dispatch, not confirmation inside the Agent.

## Compatibility

Eight built-in adapters read Claude JSONL, Codex rollouts, OpenCode SQLite, Cursor CLI
Store protobuf, Gemini JSON/JSONL, Copilot events, Kimi context/Wire and Qwen JSONL.
Cursor IDE and Kimi Desktop remain distinct launch-only integrations. Unsupported
formats fail visibly rather than extracting arbitrary strings. OpenCode uses its
observed read-only SQLite schema instead of running a configurable Agent during
indexing, keeping reads constrained to the authorized source.

References:

- https://code.claude.com/docs/en/sessions
- https://github.com/google-gemini/gemini-cli/blob/main/packages/core/src/services/chatRecordingService.ts
- https://github.com/QwenLM/qwen-code/blob/main/packages/core/src/services/chatRecordingService.ts
- https://www.kimi.com/code/docs/en/kimi-code-cli/configuration/data-locations.html
- https://github.com/xhluca/session-migrate/blob/main/docs/cursor-format.md

Synthetic fixtures establish supported parser shapes and command contracts, not
universal compatibility with future releases or real Windows/macOS Agent startup.

Desktop refinements: batch authorization confirms a fixed list of absolute sources through the same Core authorization API. Windows CLI launch uses a no-profile PowerShell process with UTF-16 EncodedCommand so startup profiles and argument quoting do not consume the resume command. The launch panel displays the literal command and directory. Codex App can open a default-source session via its documented `codex://threads/<id>` link; custom sources and other Apps without a verified session entry remain CLI-resumable only. App launch is explicit and never added to MCP. Reference: https://developers.openai.com/codex/app/commands#deeplinks

On Windows, Codex App resume dispatches the validated session deep link through ShellExecuteW and checks its return code. Package executable paths are discovery evidence only: directly spawning a Store WindowsApps executable can fail with access denied. Protocol dispatch does not require elevation or shell evaluation; failure remains visible and never falls back to a new session.

Explicit resume revalidates the selected external ID in the authorized provider file before dispatch. CLI launch probes the resolved executable with --version in the target cwd and source environment, with a 10-second limit and visible failure; probes never run during indexing or MCP reads. These checks do not claim provider-internal recovery and cannot make provider reads atomic with RepoAtlas validation.
