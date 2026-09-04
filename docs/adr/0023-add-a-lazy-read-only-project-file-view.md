# Add a lazy read-only Project file view

Status: Accepted

## Context

RepoAtlas users often need to inspect a Project's structure, README, configuration, or source without leaving the project workspace. Opening a full editor is unnecessary for this quick orientation, but recursively scanning every Project on workspace entry would conflict with offline startup, large-project performance, and the existing manual-refresh policy.

This capability must not turn RepoAtlas into an editor, introduce a second filesystem policy path, or grant external Agents additional file access through MCP. Symbolic links and Windows reparse points also cannot become a route outside the Project boundary.

## Decision

The desktop Project workspace includes a read-only Files tab. Activating it reads only the root directory's direct children. Each directory is loaded on first expansion and cached for the current Project session. A single directory response is capped at 20,000 entries; additional entries are reported as skipped and remain discoverable through path search or an external editor. The tree is projected into a flat list and virtualized. RepoAtlas does not add a filesystem watcher; Refresh clears the session cache and reloads visible branches.

Path search is separate from browsing. The first query starts a cancellable, process-memory path index, excluding version-control metadata and, by default, generated directories. The index is bounded by entry count and estimated heap use, emits throttled progress, and is discarded on refresh or Project change. Index builds and searches are single-flight; a newer query cancels older computation instead of merely ignoring its result. No database tables are added.

All directory, preview, image, and indexing operations share `repoatlas-core::project_files`. Capability-based directory handles reject parent traversal and do not follow symbolic links or reparse points. Text and Markdown previews are bounded; images are checked by content, byte size, and decoded dimensions before raw IPC delivery.

README, AGENTS.md, and Files Markdown use one sanitized renderer with GFM, frontmatter, math, GitHub alerts, Mermaid, safe HTML, and Shiki highlighting. Markdown parsing is memoized, stale highlighting work is coalesced, and files above 512 KiB use the plain virtualized code view rather than syntax tokenization. Remote images are not downloaded automatically. Local links and images are resolved only inside the Project.

The Files tab does not edit, create, delete, move, rename, or save files. Its commands are desktop-only and are not added to MCP. External Agents continue to choose and perform their own project-file reads.

## Consequences

- Opening Overview or another workspace tab performs no Files directory scan or path indexing.
- Large directory trees remain responsive through lazy loading, session caching, and row virtualization.
- External changes become visible after a manual refresh or the first expansion of a directory that has not yet been loaded.
- RepoAtlas gains a useful inspection surface without becoming a code editor or adding a watcher.
- A future editable workspace, persistent index, filesystem watcher, or MCP file-reading capability requires a separate ADR.
