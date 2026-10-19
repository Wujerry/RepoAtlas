# Target Windows and macOS from the first release

The first supported release targets both Windows and macOS rather than treating macOS as a later port. Platform-specific process control, path normalization, credential storage, terminal and IDE launching, signing, and packaging therefore belong behind explicit platform abstractions and must be validated on both operating systems.

Release installers may ship without platform developer certificates when clearly labeled: Windows installers use the UNSIGNED filename label; macOS builds without Developer ID are ad-hoc signed and not notarized. Release notes and download guidance must state these limits. Tauri updater signatures remain mandatory, independently of platform code signing. macOS packages remain experimental until validated on real hardware.

The updater accepts signed raw Windows installers (`.exe`/`.msi`) and macOS
`.app.tar.gz` bundles. Its optional ZIP extraction feature is disabled: RepoAtlas
does not publish legacy `.nsis.zip`/`.msi.zip` updates, and the released updater's
ZIP dependency has unresolved RustSec advisories. Manifest generation rejects
ZIP-only Windows artifacts. TLS validation, system proxy support, signature
verification, and macOS tar extraction remain enabled.
