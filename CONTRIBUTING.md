# Contributing to RepoAtlas

Thank you for helping make RepoAtlas a useful local-first tool. This project is in pre-release development, so focused feedback and small, verifiable changes are especially valuable.

## Before you start

1. Read [`CONTEXT.md`](CONTEXT.md) for the product vocabulary and local-first boundaries.
2. Read the relevant architecture decision in [`docs/adr/`](docs/adr/) before changing persistence, scanning, execution, Git, AI, MCP, or platform behavior.
3. Read [`DESIGN.md`](DESIGN.md) before changing UI, content, accessibility, motion, or visual tokens.
4. Check the working tree before editing. Preserve unrelated local changes and never add real project data to this repository.

## Local setup

```bash
pnpm install
pnpm tauri dev
```

Useful focused checks are:

```bash
pnpm typecheck
pnpm test -- --run
cargo fmt --all -- --check
cargo test -p repoatlas-core
cargo test -p repoatlas-mcp
cargo check -p repoatlas
```

Run the smallest relevant checks while iterating, then run every affected layer before opening a pull request. If a check cannot run on your platform, say so in the pull request instead of describing it as passing.

## Change boundaries

- Keep shared business rules in `repoatlas-core` when both the desktop app and MCP need them.
- Keep Tauri commands thin, frontend calls typed, and protocol adapters free of duplicate policy.
- Preserve explicit Scan Root authorization, manual refresh, local data storage, conservative Git operations, desktop approval for external execution, and non-destructive record removal.
- Do not put credentials, personal paths, generated build output, application databases, screenshots containing private data, or task logs in commits.
- Keep Rust incremental compilation enabled; a local file-lock issue is not fixed by disabling it.
- Use Base UI primitives and the existing Phosphor icon set. Keep user-facing copy available in English and Simplified Chinese when the affected surface is localized.

## Branches and commits

Use a short-lived branch based on `main`. Keep each change focused and explain behavior changes in the commit or pull request description. The release contract uses tags in the exact form `vX.Y.Z`; do not create release tags for ordinary development work.

## Pull requests

A good pull request includes:

- the user-visible problem and the chosen behavior;
- the affected layers and any migration or compatibility notes;
- focused tests and the exact commands run;
- screenshots for UI or styling changes, including the relevant theme and language;
- known pre-existing or platform-specific failures.

Reviewers may ask for an ADR or documentation update when a change deliberately alters a recorded architectural decision.

## Security reports

Please do not disclose security vulnerabilities in a public issue. Follow [`SECURITY.md`](SECURITY.md) for the private reporting route.

## License

By contributing, you agree that your contribution is provided under the [MIT License](LICENSE).
