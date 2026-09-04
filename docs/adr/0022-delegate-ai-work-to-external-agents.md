# Delegate AI work to external agents

Status: Accepted

## Context

RepoAtlas already launches installed coding agents and exposes typed local project and task capabilities over MCP. Its built-in provider profiles, summaries, project chat, and AI Memory duplicated the stronger model, context, and conversation workflows those agents already provide. Maintaining both paths blurred the product boundary and required RepoAtlas to own credentials, model protocols, and project-data disclosure without improving its core project-management workflow.

## Decision

RepoAtlas does not host model providers, model credentials, project chat, generated summaries, or AI Memory. It does not send project evidence to model endpoints. An installed external Agent owns its model, account, provider configuration, conversation, and any decision to read project files.

RepoAtlas remains the local control plane around that Agent: it opens the Agent at a Project location, exposes structured Projects, Scan Roots, detected facts, task definitions, Task Runs, and reports through MCP, and turns protected execution requests into desktop Pending Approvals. Existing MCP prohibitions on Git writes, shell evaluation, arbitrary command execution, and filesystem deletion remain unchanged.

The stdio adapter keeps database requests ordered on one bounded worker. Its input loop remains responsive while that worker runs, so MCP cancellation notifications can stop long Scan Root operations without creating a parallel persistence or policy path.

Legacy AI tables and records remain readable through database upgrades, backup, import, and export paths so removing the feature does not destroy user data. They are not exposed in the desktop interface or MCP, and RepoAtlas performs no new external model requests.

This decision supersedes ADRs 0005, 0006, 0008, and 0013.

## Consequences

- The Project workspace has Overview, Files, Git, and Tasks. Files is a desktop-only, read-only local inspection surface as defined by ADR 0023; it is not an Agent file API or editor.
- Settings no longer contains AI provider configuration, and RepoAtlas no longer stores new model API keys.
- Agent integrations stay interchangeable because model-specific behavior remains outside RepoAtlas.
- RepoAtlas can focus on reliable local inventory, task execution, runtime observation, approvals, and audit.
- A future built-in AI feature requires a new ADR with a concrete workflow that external agents cannot already satisfy.
