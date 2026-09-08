# Security policy

RepoAtlas is a local-first desktop application. Its security model depends on explicit Scan Root authorization, non-destructive record management, typed task execution, desktop approvals for external execution, and user-controlled AI evidence selection.

## Supported versions

The project is currently pre-release. Until the first stable release, security fixes are made against the `main` branch and the latest published draft or pre-release build. Once stable releases exist, the latest two minor release lines will be supported unless a release note says otherwise.

## Reporting a vulnerability

Please do not open a public issue for a suspected vulnerability. Use [GitHub's private vulnerability reporting form](https://github.com/Wujerry/RepoAtlas/security/advisories/new) when the repository is public. Include:

- a clear description of the impact and affected boundary;
- the smallest reproduction or proof of concept;
- affected version, platform, and configuration;
- any mitigations or disclosure timing you consider important.

If private reporting is not yet enabled, contact the repository owner privately through the [wujer GitHub profile](https://github.com/wujer) and do not attach live credentials, personal project data, or full database files.

We will acknowledge a report when practical, investigate it within the project’s capacity, and coordinate a fix or mitigation before public disclosure. Please do not test against another person’s projects, files, credentials, or running application.

## Sensitive data

Never commit provider keys, signing keys, personal paths, application SQLite files, WAL/SHM files, task logs, or screenshots containing private project data. Credential references are safe to discuss; credential values are not.
