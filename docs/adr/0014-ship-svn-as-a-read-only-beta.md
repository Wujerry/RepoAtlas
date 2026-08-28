# Ship SVN as a read-only beta

The first release discovers SVN working copies and exposes basic metadata, status, and log through the shared version-control abstraction, but excludes SVN write operations. This validates the abstraction without pretending Git write semantics map safely onto SVN update, switch, merge, and commit workflows.
