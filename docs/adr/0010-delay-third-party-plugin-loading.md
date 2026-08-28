# Delay third-party plugin loading

The first release defines internal extension boundaries for version control, metadata detection, AI, and search but does not load third-party code into RepoAtlas. A safe plugin runtime requires a separate trust, compatibility, permission, distribution, and crash-isolation design; baking an ad hoc loader into the initial desktop process would make that future boundary harder to secure.
