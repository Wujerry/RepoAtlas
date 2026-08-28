# RepoAtlas Agent Guide

## Start here

1. Read `CONTEXT.md` before changing product language or domain models. Use its terms exactly: a local checkout is a **Project**, an authorized discovery directory is a **Scan Root**, and repository identity is **Repository Lineage**.
2. Read the relevant file in `docs/adr/` before changing architecture, persistence, scanning, execution, Git, AI providers, MCP, or platform scope. ADRs are binding; update or add one when deliberately changing a recorded decision.
3. Read `DESIGN.md` before changing UI, interaction, content, accessibility, motion, or visual tokens. Keep design tokens in `src/styles.css` and accessible reusable behavior in `src/components/ui/`.
4. Inspect the current working tree and preserve unrelated tracked and untracked changes. Keep edits scoped to the request.

Work is complete when the implementation, its focused tests, and the applicable contracts above agree.

## Architecture map

- `src/`: React 19 and TypeScript desktop UI. `src/lib/api.ts` is the typed Tauri boundary.
- `src-tauri/`: Tauri commands and desktop integration. Commands delegate product behavior to the shared Rust core.
- `crates/repoatlas-core/`: authoritative application core for SQLite, scanning, Git, tasks, audit, import/export, and AI-provider behavior.
- `crates/repoatlas-mcp/`: newline-delimited stdio MCP adapter. It must reuse `repoatlas-core`; it does not create parallel persistence or policy paths.
- `src/test/` and `crates/repoatlas-core/tests/`: frontend and Rust integration coverage.

Keep business rules in `repoatlas-core` when both desktop and MCP need them. Keep Tauri commands thin, frontend calls typed, and protocol adapters free of duplicated policy.

## Product and safety invariants

- Project identity: each canonical local checkout is a distinct Project. Repository Lineage may relate checkouts; a Module becomes a Project only through explicit promotion.
- Local-first: product data stays in the application data directory. Do not write RepoAtlas metadata into managed projects.
- Authorization boundary: discover only beneath explicit Scan Roots. Scanning is user-triggered manual refresh; do not introduce filesystem watching, whole-disk crawling, or default symlink traversal. Keep secret-prone and generated directories excluded.
- Non-destructive records: removing a Project or Scan Root changes RepoAtlas records or authorization only. It never deletes, moves, or edits a real project directory.
- Structured execution: normal tasks use a program plus an argument vector through the Rust Command Broker. Shell evaluation is an explicit higher-risk mode.
- External execution: MCP task requests become pending desktop approvals. MCP cannot perform Git writes, shell-mode evaluation, arbitrary command execution, or filesystem deletion.
- Conservative Git: use typed operations, system Git credentials, fast-forward-only pulls, and file-granularity staging. Exclude destructive checkout and reset behavior.
- Bounded AI: preview an Analysis Plan before sending selected project evidence externally. Never upload a whole project implicitly, fail over providers silently, or write AI Memory without user confirmation.
- Provider ownership: manage RepoAtlas provider profiles without rewriting other tools' configuration. Third-party plugin loading remains out of first-release scope.
- SVN scope: keep SVN support read-only for the first-release beta.
- Secret-free storage and logs: persist credential references rather than secrets; keep audit events useful without exposing sensitive values.
- Platform scope: Windows and macOS are first-release targets. Treat path syntax, launch behavior, title bars, and packaging as cross-platform concerns.
- Preserve Rust incremental compilation. A local file-lock problem is not solved by disabling incremental builds.

## Implementation conventions

- Preserve offline startup and cached project loading. Network-dependent AI features must not block core project management.
- Keep project-list virtualization intact. Avoid row styles that conflict with TanStack Virtual measurement.
- Sort project trees naturally, folder-first, with deterministic tie-breaking.
- Use Base UI primitives and Phosphor icons already present in the project. Do not add external CDN assets or Unicode symbols as interface icons.
- All async actions need visible pending, success, and failure feedback. Keyboard interaction and focus-visible behavior are part of the feature, not optional polish.
- Follow the graphite/neutral visual system and amber action color from `DESIGN.md`. Support light and dark system themes, Chinese and English, reduced motion, and the 1100x720 minimum window.
- Project and folder context actions must be reachable by pointer and keyboard; never rely on hover-only controls.
- Prefer targeted changes. Generated output in `dist/` and `target/` is not source and should not be committed.

## Verification

Run the smallest relevant checks while iterating, then cover every changed layer before handoff:

```powershell
pnpm typecheck
pnpm test -- --run
cargo test -p repoatlas-core
cargo test -p repoatlas-mcp
cargo check -p repoatlas
pnpm build
```

- Frontend logic or components: run the focused Vitest file plus `pnpm typecheck`.
- Shared Rust behavior: run the focused crate test; add integration coverage for cross-boundary workflows.
- Tauri command or API contract changes: verify both Rust compilation and frontend typecheck, and update both sides of the typed boundary together.
- MCP changes: test the adapter and confirm valid newline-delimited JSON-RPC behavior for affected methods or tools.
- Layout or styling changes: inspect the live Tauri UI at relevant widths and themes. Tests and typecheck alone do not establish visual correctness.
- Cross-layer or release-sensitive changes: run the full applicable command set above. Use `pnpm tauri dev` for an end-to-end desktop smoke test when runtime behavior changed.

Report commands run, results, and any known pre-existing or environment-specific failures. Do not describe an unrun check as passing.

## Change discipline

- Add or update tests for behavior changes and regressions.
- Keep user-facing copy consistent with `CONTEXT.md`, especially confirmations involving records versus real directories.
- Update `README.md` when setup or user-visible capabilities change; update `DESIGN.md` when the active design contract changes; record architectural decisions in `docs/adr/`.
- Before finishing, review `git diff` and `git status` so generated noise, unrelated edits, secrets, and real project data remain outside the change.
