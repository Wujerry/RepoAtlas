# Use conservative typed Git operations

RepoAtlas uses the installed system Git through typed operations and leaves credentials to the user's existing Git, SSH, and credential-manager setup. Git writes do not accept arbitrary argument strings, pulls default to fast-forward only, destructive checkout or reset behavior is excluded, and commit staging begins at file granularity to keep the first release predictable and recoverable.

Detected remote identities omit URL user information, query strings, and fragments. Database upgrades clean legacy lineage facts and keys; Project reads and backup copies also sanitize legacy values. This does not alter the actual Git remote or the user's credential configuration.

Git status paths and all pathspec operations use the checkout root, including for explicitly promoted Modules. Index and working-tree changes are represented separately when both exist for one file.

Desktop Git writes take a short Core lock to prepare an immutable operation, execute Git and capture its status outside that lock, and reacquire it only to persist state plus audit. The Project location is validated before execution and checked again before updating its snapshot. If another client moves or removes the record during execution, the actual outcome is still returned and audited, with a notice that cached state was not updated. A per-checkout mutex serializes writes without blocking other Projects or task stops. Git completion refreshes the Git snapshot; full Project detection remains an explicit refresh.

