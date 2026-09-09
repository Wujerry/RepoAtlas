<p align="center">
  <img src="assets/brand/repoatlas-mark.png" width="96" height="96" alt="RepoAtlas logo" />
</p>

<h1 align="center">RepoAtlas</h1>

<p align="center">
  <a href="README.zh-CN.md">简体中文</a>
</p>

<p align="center">
  <a href="https://github.com/Wujerry/RepoAtlas/actions/workflows/ci.yml"><img src="https://github.com/Wujerry/RepoAtlas/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI status" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-FFA31A.svg" alt="MIT License" /></a>
</p>

RepoAtlas turns scattered local checkouts into one working library. See what each Project is, open it in the right coding Agent or tool, run repeatable development tasks, and keep the output and history on your machine.

> **macOS contributors wanted.** RepoAtlas has not yet been tested on real Mac hardware, so there is no supported macOS build today. If you have a Mac, help us build, test, and document it through [CONTRIBUTING.md](CONTRIBUTING.md), or open an [issue](https://github.com/Wujerry/RepoAtlas/issues) with reproducible results.

## Let your AI Agent do the setup

The fastest first run is not filling in a project catalog by hand:

1. Open RepoAtlas and copy the onboarding instruction to your external AI coding Agent.
2. The Agent connects RepoAtlas MCP, asks which directories it may scan, and registers the approved Projects.
3. Using evidence from each checkout, the Agent adds useful descriptions, reusable Task Definitions, and recognizable Project icons.
4. RepoAtlas refreshes the local library so you can review the result and start working.

For a single open checkout, the Agent can call `register_project`. For a larger archive, the Agent asks for your approval, adds the approved directory as a **Scan Root**, and explicitly calls `scan_root`. Manual registration and scanning remain available in the desktop app.

The Agent keeps ownership of its model, account, provider configuration, conversation, and decisions about reading project files. RepoAtlas keeps the durable Project and task records.

Agent changes to the shared library continue to appear after onboarding, including edits to existing descriptions, tasks, and Collections. The desktop checks local database changes while visible and when you return to it; this does not scan project directories. Task requests expire after 15 minutes and must be requested again if they were not approved in time.

## Main features

- **Working dashboard** — start with local Project and Collection status, seven-day Task Run results, recent work, and items that need action.
- **Agent-assisted initialization** — let an external AI Agent inventory approved directories and write evidence-backed descriptions, tasks, and icons through MCP.
- **Project library** — search Projects in English or Chinese, group related work with Project Collections, and relate multiple checkouts through Repository Lineage.
- **Project Brief and read-only Files** — inspect detected facts, environment requirements, README, source, configuration, recent activity, and run history without turning RepoAtlas into an editor.
- **Saved development tasks** — keep dev, test, build, and packaging commands as reviewed programs plus argument vectors, then run them in a real PTY.
- **Global task workbench** — follow active work across Projects in one full-window surface with up to four live terminals.
- **Runtime visibility** — see process-tree CPU, memory, child processes, listening ports, and safe localhost previews next to the terminal.
- **Conservative Git** — review status, diffs, and history; pull fast-forward only. SVN support remains read-only.
- **Attention and approval** — collect failed runs, unavailable Projects, environment mismatches, and Agent task requests that still need desktop approval.
- **Local-first records** — Project metadata, task output, exit codes, logs, and audit history stay in local SQLite; no RepoAtlas account or sync server is required.

## Project library

![The RepoAtlas library view with Project Collections, distinct Project icons, branch state, and attention items](assets/screenshots/en-dark-library.png)

Every canonical checkout is a **Project**. A **Scan Root** is a directory you explicitly authorize for discovery. Scanning is manual, stays under those roots, and never turns into a whole-disk crawler or filesystem watcher.

Right-click a folder in the project list and choose **Rescan folder** to refresh that folder within existing Scan Root authorizations. Scanning shows progress and can be cancelled. Folders without an authorized scan area have this action disabled; add a Scan Root in Settings first.

Projects remain tied to their real paths. Collections organize them without moving directories, and removing a RepoAtlas record does not delete the checkout on disk.

RepoAtlas opens on the Dashboard instead of choosing a Project for you. Collections are quick entries into the existing filtered project tree; the snapshot is calculated from bounded local records and does not scan checkouts, refresh Git, read source files, or contact a remote analytics service.

The Dashboard highlights a recent available Project for quick re-entry, with cached status metrics and direct access to recent Projects, Task Runs, Collections, and Attention.

The project tree toolbar adds expand-all, collapse-all, collapse-to-selected, and jump-to-current-project controls so long collections stay navigable without scrolling blind.

## Task workbench

![The RepoAtlas global task workbench with a runtime monitor and live terminal output](assets/screenshots/en-light-tasks.png)

A **Task Definition** records the executable, argument vector, and working directory instead of hiding work inside an assembled shell string. Each **Task Run** streams through a real PTY and retains its exit code and full log.

Open the global workbench to follow active runs across Projects. Runtime observations sit beside each terminal, with CPU, memory, process count, detected listening ports, and safe localhost preview links available while work remains active. Stopping a task terminates its process tree.

## Download — Windows x64 beta

1. Download the latest installer from [Releases](https://github.com/Wujerry/RepoAtlas/releases).
2. Verify its SHA-256 checksum against the published `SHA256SUMS`:

   ```powershell
   Get-FileHash .\RepoAtlas_0.1.0_x64-setup.exe -Algorithm SHA256
   ```

3. Run the installer. SmartScreen will show an “unknown publisher” warning for the unsigned beta. Choose **More info**, then **Run anyway**.

## Code signing policy

RepoAtlas is preparing to use SignPath Foundation for trusted Windows Authenticode signing. Until that setup is approved and enabled, Windows beta installers may be unsigned and can trigger a SmartScreen warning.

See the [Code signing policy](CODE_SIGNING_POLICY.md) for signing roles, privacy commitments, build-origin requirements, and release verification.

## Run from source

You need Node.js 24.11 or later (below 25), pnpm 10.20.0, Rust stable, and the [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform.

```sh
pnpm install
pnpm tauri dev
```

The desktop development command builds and copies the MCP sidecar before starting the app; the first run may take longer.

On first launch, choose **Copy for Agent, scan and discover** in the onboarding dialog. RepoAtlas prepares an instruction that lets your Agent configure MCP, ask for the directories you approve, discover Projects, and populate their descriptions, tasks, and icons.

## Architecture and development

- `src/` — React 19 and TypeScript desktop UI. `src/lib/api.ts` is the typed Tauri boundary.
- `src-tauri/` — Tauri 2 commands and desktop integration.
- `crates/repoatlas-core/` — the authoritative core for SQLite, scanning, safe file inspection, Git, tasks, approvals, and audit.
- `crates/repoatlas-mcp/` — the stdio MCP adapter. It reuses `repoatlas-core` and creates no parallel persistence path.

Checks:

```sh
pnpm typecheck
pnpm test -- --run
cargo test -p repoatlas-core
cargo test -p repoatlas-mcp
cargo check -p repoatlas
pnpm build
```

See [CONTRIBUTING.md](CONTRIBUTING.md), [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md), [SECURITY.md](SECURITY.md), and [docs/releasing.md](docs/releasing.md).

## License

[MIT](LICENSE)

### Mixed projects and Modules

Scanning continues below a root `package.json` or checkout. A Project can contain Node, Java and other Modules with their own evidence, runtime requirements and tasks. Module task actions run in the Module directory. Independent nested checkouts remain Projects; other candidates can be explicitly promoted with **Manage as independent Project**. **Use as directory group** retains the root entry and promotes its constituents. These choices survive rescans and never change project files.

The Guide starts with copyable **Initialize MCP** and **Scan directories** prompts. Agents must first obtain confirmed absolute paths, call `add_scan_root`, then `scan_root`. `get_project`, `get_project_brief` and `list_modules` expose cached Module evidence. Use `promote_module` / `set_directory_group` only for a user's explicit management choice. Module task IDs use their parent Project; promoted Modules use their own Project. `run_task` still requires desktop approval.
