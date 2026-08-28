# RepoAtlas

RepoAtlas is a local code asset control center. Its language distinguishes a local project asset from its repository lineage, discovered evidence, runnable tasks, and AI-maintained knowledge.

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
A constituent package or component within a project, especially inside a monorepo. A module is not a standalone project unless the user promotes it.
_Avoid_: Subproject, nested project

**Detected Fact**:
Project knowledge backed by local evidence and carrying its source, confidence, and observation time.
_Avoid_: Guess, metadata

**Unavailable Project**:
A project whose known location cannot currently be accessed while its managed knowledge and history remain retained.
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

**Recent Activity**:
A user's latest meaningful interaction with a project through RepoAtlas, such as opening it, launching a tool, or running a task.
_Avoid_: File modification, last commit

## Commands and safety

**Task Definition**:
A saved, user-reviewable description of a project operation such as run, test, build, or package.
_Avoid_: Shell command, script

**Task Run**:
One execution of a task definition, including its lifecycle, output, and result.
_Avoid_: Command, process

**Task Proposal**:
A non-executable suggestion for a task or operation that requires policy evaluation and, when necessary, user approval.
_Avoid_: AI command, pending task

**Approval**:
An explicit, scoped user authorization for one operation or a defined class of operations.
_Avoid_: Permission, consent

**Pending Approval**:
A time-bounded request awaiting explicit approval before a protected operation may begin.
_Avoid_: Queued task, permission request

**Audit Event**:
A durable, secret-free record of an external or protected operation's origin, intent, time, and outcome.
_Avoid_: Command log, task output

**Tool Launcher**:
A user-configurable integration that opens a project in an IDE, terminal, or filesystem browser.
_Avoid_: Task, command

## AI knowledge

**AI Summary**:
A regenerable description derived from a particular snapshot of a project's evidence.
_Avoid_: AI Memory, project description

**AI Memory**:
User-controlled, persistent project knowledge that remains independent of regenerated summaries and detected facts.
_Avoid_: AI Summary, chat history

**Analysis Plan**:
A reviewable selection of project evidence proposed for an AI analysis request.
_Avoid_: Prompt, context dump

**Provider Profile**:
A reusable connection to one AI service, including its protocol, endpoint, credential reference, and available models.
_Avoid_: Model, API key

**Provider Preset**:
A maintained template that supplies known defaults for creating a provider profile without owning the resulting profile.
_Avoid_: Provider, hard-coded provider

**Analysis Snapshot**:
A reproducible AI result associated with the selected evidence, provider profile, model, and project state from which it was generated.
_Avoid_: AI Memory, live project state

**Project Conversation**:
A locally retained AI question-and-answer history associated with one project and independently removable from AI Memory.
_Avoid_: AI Memory, global chat
