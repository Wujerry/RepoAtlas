# Local screenshot showcase

The screenshot dataset is deliberately kept outside the repository at `C:\RepoAtlas Showcase`. It contains eight fictional Projects and no remote URLs, credentials, API keys, or personal paths.

## Prepare it

Close the RepoAtlas desktop app and any RepoAtlas MCP process, then run from the repository root:

```powershell
.\scripts\seed-showcase.ps1 -Force
```

`-Force` is required because the command removes only the RepoAtlas application database (`repoatlas.sqlite`, its `-wal`/`-shm` companions, and direct files in `task-logs`) before rebuilding the local records. The script refuses to continue if a RepoAtlas process is running, if `APPDATA` does not resolve to `io.repoatlas.desktop`, or if an existing showcase directory is not marked by `.repoatlas-demo-marker`.

The seeder creates real local fixture files, initializes seven local Git repositories without remotes, leaves `pulse-mobile` with one staged and one unstaged change, and keeps `ops-playbook` without VCS metadata. The Rust `repoatlas-demo` binary then writes detected facts plus fictional Task Runs, Project Events, Pending Approvals, AI Summary, and AI Memory records directly to the development database. It is not a desktop command, MCP tool, or release bundle entry point.

## Safety boundary

The scripts never delete, move, or edit an existing project outside `C:\RepoAtlas Showcase`. Inside that root they remove only the eight known fixture directories after validating the marker. They do not configure a Provider Profile, make a network request, or persist a secret. The application database is rebuilt without creating a backup because this workflow is intended for disposable local screenshot data.

To clear only the RepoAtlas app data without recreating fixtures:

```powershell
.\scripts\clear-repoatlas-data.ps1 -Force
```

This command does not remove `C:\RepoAtlas Showcase` or any project directory.
