# Keep AI provider management inside RepoAtlas

Status: Superseded by [ADR 0022](0022-delegate-ai-work-to-external-agents.md)

RepoAtlas will use a data-driven provider registry for its own AI features but will not rewrite Codex, Claude Code, OpenCode, or other agents' provider configurations. Those agents integrate with RepoAtlas through its typed MCP surface; separating these responsibilities preserves the product's code-asset focus and avoids becoming an account-routing or model-proxy manager.


Provider API keys are stored by RepoAtlas itself. Windows uses DPAPI for the current user; macOS uses the login Keychain. The SQLite record keeps only an encrypted blob or a Keychain pointer, never the plaintext key.
