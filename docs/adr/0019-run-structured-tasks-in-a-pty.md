# Run structured tasks in a PTY

RepoAtlas keeps the Command Broker's structured executable-plus-argv boundary, but live Task Runs now spawn through a portable PTY so commands can detect a terminal, emit ANSI color, and update progress in place. The desktop workbench owns xterm rendering, resize, and input focus; MCP still cannot start, write, resize, or stop a live process without desktop approval. Shell evaluation remains an explicit higher-risk exception, logs stay capped, and Windows Job Objects plus Unix process-group cleanup remain the stop guarantee.
