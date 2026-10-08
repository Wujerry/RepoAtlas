# Restore local desktop view state

RepoAtlas restores the last desktop view after a normal exit. This is a device-local UI preference, separate from shared Project records and external Agent Sessions.

- The Tauri window-state plugin owns the main window's normal size, position and maximized state in the application configuration directory. Only `main` is tracked. Visibility, decorations and the auxiliary quick-search window retain their existing lifecycle. Missing or malformed state uses defaults; a saved position on a disconnected monitor is not restored.
- Versioned WebView local storage holds the current page, Home/Project surface, selected Project ID, list scope, Collection ID, search, filters, sorting, Project tab and Settings category. This follows the existing onboarding/sidebar preference storage and never writes into managed Projects. These preferences are not part of the portable Project database backup.
- Restore cached library data first, validate selected records (including archived Projects), clear deleted Collections and return Home if the Project cannot be restored. Restoration does not record a new Recent Activity or run any task. Do not overwrite saved navigation during failed or pending bootstrap.
- Confirmations, approvals, executable actions, unsaved form drafts and transient dialogs are not restored. Unknown preference values fall back to defaults. Storage failures must not block startup or normal navigation.

Core remains authoritative for product records and policy. These desktop-only presentation preferences do not add an MCP persistence or execution path.
