# Treat scan roots as authorization boundaries

RepoAtlas only discovers projects beneath directories the user explicitly registers as Scan Roots. It does not automatically crawl whole disks, does not follow symbolic links by default, excludes secret-prone and generated content, and treats network locations as separately controlled sources so asset discovery remains predictable and privacy-preserving.

The MCP management surface may add, scan, or remove Scan Root records because an MCP call is explicit authorization to change RepoAtlas management data. Removing a root only revokes that record (and may orphan or remove managed records according to the explicit policy); it never deletes, moves, or edits the root or any project directory. These mutations are written to the local audit trail.

