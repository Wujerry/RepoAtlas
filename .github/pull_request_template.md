## Summary

<!-- What user-visible problem does this change solve? -->

## Scope

- [ ] Product behavior
- [ ] UI, content, or accessibility
- [ ] Rust core or persistence
- [ ] Tauri desktop boundary
- [ ] MCP adapter
- [ ] Documentation or release tooling

## Safety and compatibility

<!-- Mention migrations, platform differences, security boundaries, or why no
     ADR is needed. Use Project, Scan Root, Repository Lineage, Task Run, and
     Pending Approval consistently. -->

## Verification

<!-- List exact commands and say when a platform-specific check could not run. -->

- [ ] `pnpm typecheck`
- [ ] `pnpm test -- --run`
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Relevant Rust tests/checks
- [ ] `pnpm build`

## Screenshots

<!-- Required for UI, styling, content, or accessibility changes. Include
     theme/language and confirm no private paths or credentials are visible. -->

## Checklist

- [ ] I preserved unrelated working-tree changes.
- [ ] I did not commit credentials, private paths, databases, logs, or build output.
- [ ] I updated documentation/tests/contracts affected by this change.
