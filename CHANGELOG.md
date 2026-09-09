# Changelog

All notable changes to RepoAtlas will be documented here. The project is currently pre-release; dates and package availability are not promises until a release is published.

## [Unreleased]

- Prepare bilingual open-source documentation and contribution policies.
- Prepare reproducible CI, security checks, draft release packaging, and GitHub Pages deployment workflows.
- Prepare signed updater metadata and the Windows/macOS release process.
- Ship the repoatlas-mcp binary inside desktop installers so installed apps can provide a ready-to-use MCP configuration.
- Let Agent scan instructions write project descriptions, tasks, and icons directly after summarizing discovery results.
- Stop background helper processes (git reads, version probes, task termination) from flashing console windows on Windows.
- Require Agent scan instructions to write project descriptions and task summaries in the language the user is using.
- Kill entire task process trees on stop via a Windows Job Object, so detached children (dev servers, watchers) cannot keep running after Stop.
- Run multiple task runs concurrently within a project and show all running tasks across projects in a global Active Tasks view (title-bar badge, per-run output, jump-to-project, stop).
- Polish the global Active Tasks view: 3-second live refresh, per-run elapsed-time counters, copy-command action, and a Stop-all button alongside per-run stop.
- Add a dark and a light console theme with a per-console toggle, readable text selection colors, and a light-tuned ANSI palette.
- Stop running RepoAtlas processes (app and MCP sidecar) in the NSIS installer's pre-install and pre-uninstall hooks, so upgrades no longer fail with "Error opening file for writing" while an MCP session holds the executable.

## [0.1.0-beta.2] - 2026-09-09

- Add a title-bar update entry that appears once a signed update is discovered, downloading, ready to install, or fails, with a release-notes panel and manual check, download, install, restart, and postpone actions.
- Add task search, kind/source/state filters, and run-count/last-run sorting to the project Tasks view, with a filtered-results count and clear-filters reset.
- Add an Overview Modules chapter that appears when a Project has detected modules or directory grouping is enabled.
- Harden the release pipeline: idempotent release runs, trimmed public release assets, and cross-platform updater key normalization.

## [0.1.0] - Unreleased

Initial pre-release application baseline:

- local Project library backed by SQLite;
- explicit Scan Root discovery and manual refresh;
- project facets, favorites, tags, archive state, and natural search;
- project dashboard with IDE, terminal, and file-browser launchers;
- typed task execution with desktop approvals, Task Runs, and logs;
- Git status and read-oriented project inspection;
- local Atlas reports, environment facts, AI Summary, AI Memory, and project conversations;
- stdio MCP adapter sharing the Rust core and its safety boundaries;
- English and Simplified Chinese interface support with light, dark, and system themes.

[Unreleased]: https://github.com/Wujerry/RepoAtlas/compare/v0.1.0...HEAD
[0.1.0-beta.2]: https://github.com/Wujerry/RepoAtlas/compare/v0.1.0-beta.1...v0.1.0-beta.2
[0.1.0]: https://github.com/Wujerry/RepoAtlas/releases/tag/v0.1.0
