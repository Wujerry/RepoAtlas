# Model local checkouts as projects

RepoAtlas identifies each canonical local checkout as a distinct Project and may relate Projects through a Repository Lineage. Remote URLs cannot be the primary identity because separate clones and worktrees can have different state, tasks, paths, tags, and operational meaning; monorepo packages remain Modules unless the user explicitly promotes one.

