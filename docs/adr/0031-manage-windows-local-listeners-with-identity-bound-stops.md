# Manage Windows local listeners with identity-bound desktop stops

Status: Accepted. Extends ADR 0021's observation scope; retains ADRs 0016, 0017,
0019 and 0022 execution and MCP boundaries.

The top navigation bar opens a standalone, manually refreshed Windows native TCP
listener inventory, including processes launched outside RepoAtlas. It is separate
from the task workbench. It reuses the Broker's
listener inspection and port preflight implementation. This is process inspection,
not Project discovery; no Scan Root authorization is extended and no files are
scanned. Other platforms retain existing task controls and show an explicit
unsupported state for the new inventory.

Core owns attribution, process identity, ephemeral associations, confirmation
tokens and native termination. Tauri dispatches blocking system work off its UI
thread, reads Project records briefly under the Core lock, and records secret-free
intent/outcome audit events. MCP exposes none of these new commands.

Read-only identity inspection requests only PROCESS_QUERY_LIMITED_INFORMATION,
using GetExitCodeProcess for the nonblocking exit check. SYNCHRONIZE and
PROCESS_TERMINATE are requested only for the explicit force-stop handle. Query
denial remains a visible unavailable-path state; inspection never requests elevation.

Attribution uses verified Windows Job membership for managed Task Runs, a manual
association for an exact process instance, then an observed working directory or
executable beneath a registered Project. Nested Projects choose the longest path
with a directory boundary. Names, bare port numbers, command-line substrings and
unverified parent PIDs are never ownership evidence. Unknown and inaccessible
processes remain visible. Associations last only for the process instance and
desktop session; they never follow a PID, port or service restart and are not
exported. No command lines or environment values are stored or shown.

Stopping requires a reviewed target and impact preview with a one-use 60-second
token. Confirmation rechecks PID, exact FILETIME creation value (serialized as a
string), executable, managed run identity and listening-port scope. External force
termination opens a handle, verifies identity, checks critical-process status,
and calls TerminateProcess on that same handle. It never walks an external tree,
calls taskkill, terminates by name, or asks for elevation. Core blocks the current
application, its ancestors, protected processes and known WSL/Docker host helpers.

Managed force stop reuses the retained Job Object, covering all descendants even
if parents exit. The old Windows taskkill-before-Job path is removed because a
numeric PID can be reused. Normal managed stop sends Ctrl+C through its existing
PTY and reports only a request until exit; it never escalates automatically.
External consoles do not have a generally safe targeted cooperative-stop channel:
the UI directs normal stopping to the original terminal/IDE and offers separately
confirmed single-process force termination. Child and parent processes remain
running, and all ports of the selected external process are disclosed.

No automatic stop, port rewriting, external restart, WSL or Docker management is
introduced. Listener observations can become stale; refresh and repeated preflight
are explicit. Tests cover unknown/ambiguous attribution, PID reuse, inaccessible
and exited targets, single-use confirmations and actual isolated Windows listeners.

Windows API references: [process handle lifetime](https://learn.microsoft.com/en-us/windows/win32/procthread/process-handles-and-identifiers)
and [console process groups](https://learn.microsoft.com/en-us/windows/console/console-process-groups).
