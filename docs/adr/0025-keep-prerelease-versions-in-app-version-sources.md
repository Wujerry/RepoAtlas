# Keep prerelease versions in the app version

Status: Accepted

## Context

The Tauri updater compares the installed app version with the `version` field in `latest.json` using SemVer. The previous release contract kept `0.1.0` in `package.json`, workspace `Cargo.toml`, and `src-tauri/tauri.conf.json` while only the release tag carried the `-beta.N` suffix, so the updater manifest advertised `0.1.0-beta.2` against an installed `0.1.0`. SemVer ranks `0.1.0-beta.2` below `0.1.0`, so installed prerelease builds never saw newer betas as updates, and the desktop UI showed `v0.1.0` for builds that were actually tagged `v0.1.0-beta.2`.

## Decision

The three in-repo version sources carry the exact release version, including any prerelease suffix (for example, `0.1.0-beta.2`). Release tags are exactly `v` plus that version, and `scripts/check-version.mjs` enforces the equality instead of allowing the tag to add a suffix.

## Consequences

- Prerelease builds report their full version (for example, `v0.1.0-beta.2`) in the desktop UI and to the updater, so beta-to-beta upgrades compare correctly.
- Every release must bump all three version sources to the new full version; `check-version.mjs` rejects tags that do not match exactly.
- Installs of builds released before this change report `0.1.0` and cannot be updated to prerelease builds through the updater; they need a manual reinstall of a build with the new version.
- The updater endpoint `https://github.com/Wujerry/RepoAtlas/releases/latest/download/latest.json` still resolves only to non-prerelease releases, so update checks stay unavailable until the first stable release is published.
