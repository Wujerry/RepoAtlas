# Never fail over AI providers silently

Status: Superseded by [ADR 0022](0022-delegate-ai-work-to-external-agents.md)

RepoAtlas does not automatically resend project evidence through a different AI provider when a request fails. Switching providers is an explicit user action because endpoints can have different privacy, retention, cost, and trust properties even when they expose compatible protocols.

