# Discover Modules below Project manifests

Status: Accepted

## Decision

Manifest detection and Project identity are separate. Manual Scan Root refresh continues below a discovered manifest or checkout, excluding generated directories and symlink/reparse-point directories. A manifest cannot stop discovery or imply that every descendant has its technology stack.

The nearest managed ancestor Project owns a cached Module candidate unless the directory has its own checkout boundary or is already explicitly managed as a Project. Each Module carries its canonical and relative location, manifest evidence, observation time, availability, technology, runtime requirements, and task IDs. Literal Node workspaces, pnpm package lists, Cargo workspace members and Maven modules can establish declared membership; unsupported or ambiguous declarations stay candidates. No build configuration is executed to determine membership.

Module evidence is stored in the shared Core database, separate from Project identity. Reading project detail or listing Modules uses cached records and never scans. Registering or manually refreshing a Project discovers within that Project directory; scanning a Scan Root discovers only within that authorized root. Desktop and MCP reuse the same Core classification and mutation paths.

Module tasks belong to their owning Project, with an explicit Module working directory. IDs remain stable across refresh. Explicit promotion creates or reuses the canonical Project, transfers task definitions, keeps old Task Runs with their original owner, and preserves user edits. Existing independent Projects never automatically become Modules on refresh. Directory grouping is an explicit, persistent Project preference: its root entry and metadata remain, existing Modules are promoted, and future discovered immediate constituents become Projects. Disabling grouping never demotes or merges existing Projects.

MCP exposes `list_modules`, `promote_module`, and `set_directory_group`, and includes Modules in project detail and brief. Promotion and grouping change only RepoAtlas records and are audited. Agent instructions require a user request before changing management boundaries. Execution still uses the Command Broker and desktop Pending Approvals; no new arbitrary execution or file API is introduced. Module launcher actions validate the cached path against the owning Project before using the existing desktop launchers.

## Consequences

- A mixed Node/Java directory retains an overall Project entry while showing each constituent's own evidence and scoped tasks.
- Root technology facets include discovered constituent stacks, while per-Module evidence remains distinct.
- Missing Module records remain inspectable as unavailable; refresh does not delete project directories.
- Directory-group preferences survive backup and metadata export/import. Module evidence can be rediscovered by explicit refresh.
- Scanning now visits more source directories. Cancellation remains active throughout walking and scan ingestion; cached startup is unchanged.
- The Guide begins with copyable initialization and authorized-directory scanning prompts in both languages.
