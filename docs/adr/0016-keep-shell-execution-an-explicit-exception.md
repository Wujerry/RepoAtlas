# Keep shell execution an explicit exception

Normal tasks execute a structured program and argument vector through the Rust Command Broker, while PowerShell, cmd, bash, or zsh evaluation requires an explicitly marked higher-risk Shell Mode. Sensitive environment values live in the operating-system credential store and task records contain only names and references, preventing routine task configuration from becoming an unreviewable shell or secret channel.

