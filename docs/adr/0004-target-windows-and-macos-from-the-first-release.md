# Target Windows and macOS from the first release

The first supported release targets both Windows and macOS rather than treating macOS as a later port. Platform-specific process control, path normalization, credential storage, terminal and IDE launching, signing, and packaging therefore belong behind explicit platform abstractions and must be validated on both operating systems.
