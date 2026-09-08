# RepoAtlas

RepoAtlas is a local code asset control center. Its language distinguishes a local project asset from its repository lineage, discovered evidence, runnable tasks, and external Agent integrations.

## Code assets

**Scan Root**:
A directory explicitly authorized by the user for recursive project discovery.
_Avoid_: Search folder, workspace

**Project**:
A manageable local code checkout identified by its canonical filesystem location. Multiple checkouts of the same repository are distinct projects.
_Avoid_: Repository, codebase

**Repository Lineage**:
The optional shared identity that relates projects originating from the same version-control repository.
_Avoid_: Project, remote

**Module**:
A constituent package or component within a project, especially inside a monorepo. A module is not a standalone project unless the user promotes it. A detected manifest below a Project is a Module candidate until its membership is evidenced; each Module retains its own stack, runtime requirements, and task working directory. A root manifest never stops authorized discovery.
_Avoid_: Subproject, nested project

**Detected Fact**:
An observed project fact backed by local evidence and carrying its source, confidence, and observation time.
_Avoid_: Guess, metadata

**Unavailable Project**:
A project whose known location cannot currently be accessed while its managed metadata and history remain retained.
_Avoid_: Missing project, deleted project

**Dependency Snapshot**:
An offline observation of a project's declared dependencies, lockfiles, package managers, and evidence time.
_Avoid_: Dependency audit, installed packages

**Facet**:
A system-derived classification used to filter projects, such as language, framework, package manager, or version-control system.
_Avoid_: Tag, label

**Tag**:
A user-owned classification attached to a project for personal organization.
_Avoid_: Facet, detected category

**Project Collection**:
A user-owned named set of Projects used for organization and filtering. Membership never changes Project identity or filesystem location.
_Avoid_: Folder, Scan Root, workspace

**Dashboard**:
The default local overview derived from RepoAtlas records: Project and Project Collection state, recent Task Run results, and current Attention Items. It does not scan Projects or inspect their files when opened.
_Avoid_: Analytics service, activity feed, AI summary

**Recent Activity**:
A user's latest meaningful interaction with a project through RepoAtlas, such as opening it, launching a tool, or running a task.
_Avoid_: File modification, last commit

**Project File View**:
A desktop-only, read-only view of a Project's local directory tree and previewable files. It loads paths on demand, never follows symbolic links, and is not a code editor or MCP file API.
_Avoid_: Workspace, file manager, editor

## Commands and safety

**Task Definition**:
A saved, user-reviewable description of a project operation such as run, test, build, or package.
_Avoid_: Shell command, script

**Task Run**:
One execution of a task definition, including its lifecycle, output, and result.
_Avoid_: Command, process

**Runtime Observation**:
A live, bounded observation of one Task Run's process tree, CPU, memory, listening ports, and local development endpoints.
_Avoid_: Profiler, telemetry upload, background watcher

**Attention Item**:
A derived, actionable local condition such as a Pending Approval, failed Task Run, Unavailable Project, or environment mismatch. Acknowledgement applies only to that condition version.
_Avoid_: Notification feed, audit log

**Task Proposal**:
A non-executable suggestion for a task or operation that requires policy evaluation and, when necessary, user approval.
_Avoid_: Agent command, pending task

**Approval**:
An explicit, scoped user authorization for one operation or a defined class of operations.
_Avoid_: Permission, consent

**Pending Approval**:
A request valid for 15 minutes from creation, awaiting explicit approval before a protected operation may begin. An expired request requires a new request.
_Avoid_: Queued task, permission request

**Audit Event**:
A durable, secret-free record of an external or protected operation's origin, intent, time, and outcome.
_Avoid_: Command log, task output

**Tool Launcher**:
A user-configurable integration that opens a project in an IDE, terminal, or filesystem browser.
_Avoid_: Task, command

## Agent integration

**Project Brief**:
A structured local view of a Project's detected facts, environment, tasks, recent runs, and recent activity.
_Avoid_: AI Summary, README replacement

**External Agent**:
An installed third-party coding agent opened at a Project's location. The Agent owns its model, account, provider configuration, and conversation.
_Avoid_: built-in assistant, RepoAtlas model

**Agent Request**:
A typed request received through RepoAtlas MCP. Record-management requests follow MCP policy; protected execution becomes a Pending Approval in the desktop app.
_Avoid_: direct execution, chat message
