# Keep AI provider management inside RepoAtlas

RepoAtlas will use a data-driven provider registry for its own AI features but will not rewrite Codex, Claude Code, OpenCode, or other agents' provider configurations. Those agents integrate with RepoAtlas through its typed MCP surface; separating these responsibilities preserves the product's code-asset focus and avoids becoming an account-routing or model-proxy manager.

