# Code signing policy

## Status

RepoAtlas is preparing an application to the SignPath Foundation open-source code-signing program. Existing Windows releases, if any, must be treated as unsigned unless their Authenticode signature can be verified independently.

After the project is accepted and SignPath signing is enabled, signed Windows releases will follow this policy.

## Project

- Project: RepoAtlas
- Repository: https://github.com/Wujerry/RepoAtlas
- Homepage: https://wujerry.github.io/RepoAtlas/
- License: MIT
- Maintainer: [Wujerry](https://github.com/Wujerry)

RepoAtlas is a local-first desktop control center for local source-code checkouts. It helps developers organize projects, inspect repository state, launch coding tools and AI agents, and run reviewed development tasks while keeping RepoAtlas's own records on the local machine.

## Signing service

Once the SignPath Foundation application is approved and signing is enabled:

> Free code signing provided by SignPath.io, certificate by SignPath Foundation.

Only release artifacts built from this public repository by the project's trusted CI workflow may be submitted for SignPath signing.

RepoAtlas's Tauri updater signatures are separate from Windows Authenticode code signing. The updater key protects update artifacts from tampering; the SignPath Foundation certificate identifies the Windows publisher and the verified build origin.

## Team roles

RepoAtlas is currently maintained by a single maintainer.

- Committer: [Wujerry](https://github.com/Wujerry)
- Reviewer: [Wujerry](https://github.com/Wujerry)
- Signing approver: [Wujerry](https://github.com/Wujerry)

Changes proposed by contributors who do not have direct commit access must be reviewed by the maintainer before merging. Each SignPath signing request must receive manual approval from the signing approver.

SignPath signing will only be enabled while all team members with signing-related access use multi-factor authentication for both GitHub and SignPath.

## Build and release origin

Release artifacts are built by GitHub Actions from the public repository. The release branch is `main`, and release tags use the `vX.Y.Z` or `vX.Y.Z-<prerelease>` format.

The release workflow and build scripts are part of the signed source-of-truth and must be reviewed with the same care as application code. SignPath origin verification will be used for signed release artifacts.

Only RepoAtlas-owned binaries may be signed using the project's SignPath subscription. Unsigned upstream open-source binaries may be bundled where permitted by SignPath Foundation policy, but they must not be re-signed as RepoAtlas-owned binaries.

## Privacy

RepoAtlas is local-first and does not provide a RepoAtlas account or hosted synchronization service. Project metadata, task output, exit codes, logs, and audit history are stored locally.

RepoAtlas does not upload project contents to a RepoAtlas-operated service. Network activity may occur when the user explicitly invokes functionality that requires it, such as Git remote operations, opening external resources, using external development/AI tools, or checking/downloading application updates. Those external systems and services have their own privacy policies.

## User control and system changes

RepoAtlas may run development commands selected or approved by the user. Agent-requested tasks require desktop approval before execution. The application does not intentionally make unrelated system configuration changes without user action.

RepoAtlas installers provide an uninstall path through the operating system.

## Release verification

Before publishing a signed Windows release, the maintainer must verify:

1. The artifact was produced by the configured trusted GitHub Actions release workflow from the intended release tag.
2. The SignPath signing request passed origin verification.
3. The Authenticode signature is valid and identifies SignPath Foundation as the certificate subject/publisher as expected by the program.
4. Tauri updater artifacts and `.sig` files are present and valid.
5. Published SHA-256 checksums match the release artifacts.
6. Installation, launch, update, and uninstall smoke tests pass.

See [docs/releasing.md](docs/releasing.md) for the complete release process.
