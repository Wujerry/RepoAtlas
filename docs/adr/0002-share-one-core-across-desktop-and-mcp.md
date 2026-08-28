# Share one core across desktop and MCP

The Tauri desktop application and the local MCP adapter will call the same Rust application core instead of accessing SQLite or spawning commands independently. This keeps discovery, metadata changes, auditing, and approval rules consistent while allowing external tools such as Codex to manage safe project metadata without bypassing RepoAtlas safety boundaries.

