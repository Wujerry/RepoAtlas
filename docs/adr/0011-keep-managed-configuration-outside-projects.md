# Keep managed configuration outside projects

RepoAtlas stores names, tags, task definitions, launcher choices, and other manual management data in its own local store rather than silently creating files inside discovered projects. Users may later export an explicit portable project configuration, but ordinary discovery and editing must remain non-invasive for old, archived, or externally owned source trees.

