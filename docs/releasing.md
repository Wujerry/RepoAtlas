# Releasing RepoAtlas

This document describes the intended release path for the public `Wujerry/RepoAtlas` repository. The repository is being prepared locally; do not add a remote, push a tag, publish a Release, or deploy Pages as part of local preparation.

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

- Preferred OSS path: SignPath Foundation Windows Authenticode signing after project approval. SignPath keeps the certificate private key in its signing service; do not expect a project-owned PFX in this mode.\n- Fallback/private-certificate path: `WINDOWS_CERTIFICATE` (base64-encoded PFX) and `WINDOWS_CERTIFICATE_PASSWORD` for Windows Authenticode.
- `APPLE_CERTIFICATE` (base64-encoded P12), `APPLE_CERTIFICATE_PASSWORD`, and `APPLE_SIGNING_IDENTITY` for the macOS Developer ID certificate.
- `APPLE_TEAM_ID`, `APPLE_ID`, and `APPLE_PASSWORD` (an app-specific password) for notarization.

The release workflow always creates a draft. A stable tag without Windows Authenticode credentials fails before any build starts; a prerelease tag continues without them and stages Windows artifacts as `UNSIGNED-BETA`. Missing platform-signing secrets are reported in the run summary and must not be treated as stable-release approval. The updater signing key remains mandatory for every tag because a Tauri updater artifact without its signature cannot be consumed by existing installations. Before building, the workflow also asserts that `bundle.windows.nsis.installerHooks` is still configured in `src-tauri/tauri.conf.json`, and `scripts/copy-mcp-sidecar.mjs` rebuilds the MCP sidecar for the target platform so installers never ship a stale binary from another target triple.


### SignPath Foundation preparation

RepoAtlas is preparing an application to the SignPath Foundation open-source code-signing program. The public [Code signing policy](../CODE_SIGNING_POLICY.md) defines signing roles, privacy commitments, origin verification, and manual approval requirements.

Before applying or enabling SignPath signing:

1. Publish at least one unsigned Windows beta in the same installer form that will later be signed.
2. Keep the repository, release artifacts, documentation, and license publicly accessible.
3. Ensure GitHub and SignPath accounts used by the signing team have MFA enabled.
4. Configure SignPath's GitHub trusted build/origin verification after project approval.
5. Preserve manual approval for every signing request.
6. Update the release workflow so the Windows artifact is signed by SignPath before it is attached to the final GitHub Release.
7. Keep Tauri updater signing enabled independently; SignPath Authenticode does not replace `TAURI_SIGNING_PRIVATE_KEY`.

Do not claim that a release is SignPath-signed until the project has been accepted and the artifact's Authenticode signature has been verified.


### Updater signing key normalization

The release workflow normalizes whitespace in `TAURI_SIGNING_PRIVATE_KEY` before invoking Tauri. Tauri updater keys are base64 strings; a trailing newline introduced while copying the secret must not make a release fail. The workflow validates that the normalized value decodes to a minisign/Tauri secret-key box without printing the key.

When Apple Developer signing credentials are not fully configured, macOS prerelease builds use Tauri's ad-hoc signing identity (`-`) instead of exporting empty Apple certificate variables.

### Public Release assets

Release builds intentionally keep the GitHub Release page small. `tauri-action` runs in build-only mode; all bundle and signature files remain in short-lived Actions artifacts, while the public Release receives only the files users or the updater need.

A normal Windows + macOS release publishes seven assets: one Windows NSIS installer, two macOS DMGs, two macOS updater archives, `latest.json`, and `SHA256SUMS`. Detached `.sig` files and the Windows MSI stay internal; updater verification still works because signature contents are embedded in `latest.json`.

### Existing releases and reruns

The release workflow is idempotent around the GitHub Release object:

- If no Release exists for the tag, the workflow creates a draft Release.
- If a Release already exists for the tag, including one created or published from the GitHub web UI, the workflow reuses it instead of trying to create a duplicate.
- A manual workflow run may reuse an existing tag when that tag points to a commit in `main` history. This allows release-pipeline fixes on `main` to repair an earlier failed build without moving or recreating the tag.
- Existing draft/published and prerelease state is preserved rather than silently changed.
- Generated assets are uploaded with replacement semantics so a rerun can repair or refresh the same release.
- The unsigned-Windows notice is marker-based and is added at most once.

For the cleanest public launch, prefer letting Actions create a draft and publish it after the build finishes. Creating and publishing a Release first is supported, but users may briefly see the Release before all CI-built assets arrive.

## Release workflow

1. Update the changelog and the four version sources together.
2. Run the local checks listed below.
3. Choose one release trigger only when the repository is ready to be published:
   - Preferred: open **Actions → Release → Run workflow**, select `main`, and enter the exact release tag (for example `v0.1.0-beta.1`). The workflow validates the release contract and creates the lightweight tag at the selected `main` commit.
   - Alternative: create and push the exact `vX.Y.Z` / prerelease tag locally; the existing tag-push trigger remains supported.
4. GitHub Actions creates a draft Release when none exists, or reuses an existing Release for the same tag. This makes the workflow safe to rerun and also supports a Release/tag created from the GitHub web UI. Existing draft/published and prerelease state is preserved.
5. The workflow builds the three supported targets with `tauri-apps/tauri-action` and attaches installer/updater artifacts and `.sig` files using replace-on-conflict uploads.
6. A final job generates the platform map in `latest.json` and a `SHA256SUMS` file, verifies it with `sha256sum -c`, then uploads both to the same draft. After downloading `SHA256SUMS` and the listed assets into one directory, run `sha256sum -c SHA256SUMS` to verify them locally.
7. Review the draft on every platform. Verify platform signatures, updater URLs, release notes, and the absence of private paths or credentials.
8. Publish the draft only after platform signing, installation, upgrade, rollback, and smoke checks are complete.

The updater endpoint is:

```text
https://github.com/Wujerry/RepoAtlas/releases/latest/download/latest.json
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

The release workflow supports both tag pushes and an explicit `workflow_dispatch`
entry point. Manual releases must be launched from `main` and require an explicit
release-tag input; the workflow creates that tag only after version, updater-key,
and platform-signing gates pass. Pages is a separate workflow and is not deployed
as part of release preparation.

On Windows, inspect the generated MSI and NSIS bundles and their `.sig` files. On macOS, inspect both Intel and Apple Silicon bundles, signatures, and notarization results. Keep Rust incremental compilation enabled during local verification.

## Rollback and incident handling

If an artifact, signature, or update note is wrong, keep the Release as a draft or unpublish it according to GitHub's release controls. Do not reuse a released version number with different bytes. Build a new patch version, rerun the consistency check, and document the correction in the changelog. If a signing key may have leaked, stop publishing and rotate according to the Tauri updater key-rotation plan before shipping another update.
