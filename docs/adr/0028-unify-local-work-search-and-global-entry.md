# Unify local work search behind a desktop global shortcut

Status: Accepted

## Decision

The desktop command palette searches Projects, authorized Agent Session text and
existing commands together. It reuses Core's Project search and Session APIs; it
does not create another index, source authorization or execution path. Session
metadata search also includes the current Project name, canonical path and recorded
cwd, without copying Project metadata into the transcript index. Each checkout
remains a distinct Project.

Ctrl+K on Windows / Cmd+K on macOS opens the in-window palette. While RepoAtlas is
running, Ctrl+Shift+K / Cmd+Shift+K opens a separate, reusable desktop search window.
The Tauri global-shortcut plugin owns OS registration in Rust. Registration failure
is non-fatal, visible in the palette and Settings, and can be retried. Closing the
main desktop still exits the application; this change adds no tray/background
service, filesystem watcher or autostart.

The auxiliary window mounts the shared search UI without the project tree, task
monitors, onboarding or a second Core runtime. Escape and its Close button hide it.
Blur does not dismiss it, so nested resume dialogs remain usable. Both windows use
the same SQLite/Core state, stored preferences and authorized cached history.
Searching never grants access or starts indexing. Source management remains in
Sessions and is reachable from either entry.

Window navigation is typed as Project ID or Sessions/source-management target,
optionally with a Session ID and message index. Rust queues bounded requests until
the receiving frontend is ready and acknowledges handling, so a first invocation
or a slow bootstrap cannot lose the selected target. Events are wake-up hints,
not storage. Navigation never accepts an arbitrary URL, executable or shell text.

Results retain Agent, timestamp, Project/cwd and literal excerpts. Selecting a
Session loads bounded surrounding messages; copying uses the original message
text. Keyboard preview and explicit continuation are separate actions. Resume
reuses ADR 0027's source validation and original-Agent launch, with visible missing
source/session/cwd errors and no fallback to another checkout. Launch success means
dispatch only; RepoAtlas does not claim recovery inside the external Agent.

MCP capabilities and external execution restrictions are unchanged.

## Verification

Cover shared search and resume regressions in Core, stale frontend searches and
previews, keyboard/IME handling, partial failures, and window delivery/acknowledgment.
OS registration, focus, light/dark rendering and minimum main-window layout require
desktop smoke verification. Windows verification does not establish macOS behavior.
