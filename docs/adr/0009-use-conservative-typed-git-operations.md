# Use conservative typed Git operations

RepoAtlas uses the installed system Git through typed operations and leaves credentials to the user's existing Git, SSH, and credential-manager setup. Git writes do not accept arbitrary argument strings, pulls default to fast-forward only, destructive checkout or reset behavior is excluded, and commit staging begins at file granularity to keep the first release predictable and recoverable.

