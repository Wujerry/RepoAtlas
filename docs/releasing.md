# Releasing RepoAtlas

This document describes the intended release path for the public `wujer/RepoAtlas` repository. The repository is being prepared locally; do not add a remote, push a tag, publish a Release, or deploy Pages as part of local preparation.

## Release contract

- The release branch is `main`.
- GitHub releases are created as drafts. A maintainer reviews artifacts, notes, signatures, and checksums before publishing.
- Release tags are `vX.Y.Z` for stable releases (for example, `v0.1.0`) or `vX.Y.Z-<prerelease>` for prereleases (for example, `v0.1.0-beta.1`).
- The tag version, root `package.json`, workspace `Cargo.toml`, and `src-tauri/tauri.conf.json` must contain the same SemVer version; the tag may add a prerelease suffix. `node scripts/check-version.mjs` is the source-of-truth check.
- Prerelease tags create GitHub prerelease drafts. Windows artifacts built without Authenticode credentials are labeled `UNSIGNED-BETA` in their filenames, the checksum report, and the release notes. Stable tags fail the workflow when Windows Authenticode credentials are missing.
- First-release installer targets are Windows x64, macOS Intel, and macOS Apple Silicon. Linux is checked in CI but is not packaged for the first release.

The local consistency check is:

```bash
node scripts/check-version.mjs --tag v0.1.0
node scripts/check-version.mjs --tag v0.1.0-beta.1
```

The repository also pins the local toolchain contract in `rust-toolchain.toml` and
`package.json` (`packageManager` and Node.js engine). Keep those pins aligned with
the versions used by the CI workflows.

## One-time GitHub setup

After the repository is intentionally published, configure these Actions secrets/variables. Keep all private values outside the repository.

### Required for updater artifacts

- `TAURI_SIGNING_PRIVATE_KEY`: Tauri updater private key content or path.
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: password for the updater key, when used.

The matching public key is safe to commit in the Tauri updater configuration. The private key is not. Generate it once with the Tauri signer and back it up securely; losing it prevents existing installations from accepting future updates.

### Platform signing and notarization

Configure platform credentials only in GitHub encrypted secrets and only when a release is ready to be signed. The release workflow exposes these inputs separately from the updater key:

- `WINDOWS_CERTIFICATE` (base64-encoded PFX) and `WINDOWS_CERTIFICATE_PASSWORD` for Windows Authenticode.
- `APPLE_CERTIFICATE` (base64-encoded P12), `APPLE_CERTIFICATE_PASSWORD`, and `APPLE_SIGNING_IDENTITY` for the macOS Developer ID certificate.
- `APPLE_TEAM_ID`, `APPLE_ID`, and `APPLE_PASSWORD` (an app-specific password) for notarization.

The release workflow always creates a draft. A stable tag without Windows Authenticode credentials fails before any build starts; a prerelease tag continues without them and stages Windows artifacts as `UNSIGNED-BETA`. Missing platform-signing secrets are reported in the run summary and must not be treated as stable-release approval. The updater signing key remains mandatory for every tag because a Tauri updater artifact without its signature cannot be consumed by existing installations. Before building, the workflow also asserts that `bundle.windows.nsis.installerHooks` is still configured in `src-tauri/tauri.conf.json`, and `scripts/copy-mcp-sidecar.mjs` rebuilds the MCP sidecar for the target platform so installers never ship a stale binary from another target triple.

## Release workflow

1. Update the changelog and the four version sources together.
2. Run the local checks listed below.
3. Create a local tag in the exact `vX.Y.Z` form only when the repository is ready to be published.
4. Push the branch and tag only after an explicit maintainer decision to publish.
5. GitHub Actions creates a draft Release, builds the three supported targets with `tauri-apps/tauri-action`, and attaches installer/updater artifacts and `.sig` files.
6. A final job generates the platform map in `latest.json` and a `SHA256SUMS` file, verifies it with `sha256sum -c`, then uploads both to the same draft. After downloading `SHA256SUMS` and the listed assets into one directory, run `sha256sum -c SHA256SUMS` to verify them locally.
7. Review the draft on every platform. Verify platform signatures, updater URLs, release notes, and the absence of private paths or credentials.
8. Publish the draft only after platform signing, installation, upgrade, rollback, and smoke checks are complete.

The updater endpoint is:

```text
https://github.com/wujer/RepoAtlas/releases/latest/download/latest.json
```

Tauri updater metadata must contain a valid SemVer version, RFC 3339 publication date, platform-specific URLs, and the complete signature text for each updater artifact. A signature URL is not a substitute for the signature content.

## Local verification

Run the smallest relevant checks while iterating, then the complete applicable set:

```bash
pnpm install --frozen-lockfile
pnpm check:version
pnpm typecheck
pnpm test -- --run
pnpm build
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo check -p repoatlas
node scripts/check-version.mjs --tag v0.1.0
```

The release workflow is tag-triggered and intentionally has no local/development
shortcut. It must not be run against a branch name. Pages is a separate workflow
and is also not deployed during local preparation.

On Windows, inspect the generated MSI and NSIS bundles and their `.sig` files. On macOS, inspect both Intel and Apple Silicon bundles, signatures, and notarization results. Keep Rust incremental compilation enabled during local verification.

## Rollback and incident handling

If an artifact, signature, or update note is wrong, keep the Release as a draft or unpublish it according to GitHub's release controls. Do not reuse a released version number with different bytes. Build a new patch version, rerun the consistency check, and document the correction in the changelog. If a signing key may have leaked, stop publishing and rotate according to the Tauri updater key-rotation plan before shipping another update.
