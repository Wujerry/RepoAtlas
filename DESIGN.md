# Design

## Source of truth
- Status: Active
- Last refreshed: 2026-09-04
- Primary product surfaces: Dashboard, desktop project library, Project Collections, project workspace, global task workbench, attention center, command palette, scanning, settings, and external Agent launch/MCP integration.
- Evidence reviewed: `README.md`, `CONTEXT.md`, `docs/adr/*`, `src/App.tsx`, `src/components/*`, `src/styles.css`, and the Tauri window configuration.

## Brand
- Personality: Quietly technical, cartographic, precise, trustworthy, and crafted for long sessions.
- Trust signals: Local-first language, explicit authorization boundaries, calm status feedback, and reversible actions.
- Avoid: Blue-purple AI gradients, neon cyberpunk styling, ornamental motion, generic glass cards, and destructive wording that implies RepoAtlas owns project directories.

## Product goals
- Goals: Make large local code collections immediately legible; make scanning, read-only file inspection, Agent launching, Git, tasks, and execution history fast and safe; make every action visibly acknowledged.
- Non-goals: A code editor, a filesystem graph, cloud synchronization, or a replacement for the system Git/IDE/terminal.
- Success signals: A new user can add the current Project through MCP, or add a Scan Root, and understand the result without help; a returning user can open a project or command in seconds; every async operation has a visible state and recovery path.

## Personas and jobs
- Primary personas: Developers and technical leads with many local checkouts across multiple languages and years of work.
- User jobs: Rediscover projects, understand current state, launch the right Agent or tool, run known tasks, perform conservative Git work, and retain local notes and execution history.
- Key contexts of use: 1100px to large desktop windows on Windows and macOS, keyboard-heavy use, light and dark system themes, Chinese and English.

## Information architecture
- Primary navigation: The persistent left navigation is removed. Projects, favorites, recent, and archived are scopes in the project tree; settings and help live in the title bar and command palette.
- Core routes/screens: Default Dashboard, path-grouped project collection, project workspace, full-workspace modal settings, help and MCP setup, global command palette, first-launch onboarding, confirmation surfaces, and scanning status. Settings and Help overlay the current screen so closing either preserves the underlying selection, filters, tree expansion, and scroll position.
- The Dashboard is the returning-user home. It combines a compact current-state summary with terminal Task Run results from the previous seven days, puts Project Collections first as workstream entry points, and keeps bounded recent Projects, recent Task Runs, and Attention Items below. Every value comes from the local Core snapshot and shows its update time; the Dashboard never triggers scanning, live Git inspection, Project file reads, or AI-generated interpretation.
- Selecting a Dashboard Project opens its existing workspace. Selecting a Project Collection resets search and facet filters, applies that Collection to the persistent project tree, and opens its first non-archived Project; an empty Collection remains on the Dashboard. The title-bar brand and command palette both return to the Dashboard without remounting the project tree.
- First-launch onboarding is a skippable three-step dialog: welcome, MCP one-step add, then waiting for a shared-database result. Manual Scan Root discovery remains the fallback when MCP is unavailable or the user wants bulk import.
- Shared-database changes continue to refresh the library after onboarding, including Agent edits to existing descriptions, tasks, and Collections. This preserves the current selection and filters where possible. Global Project navigation clears an incompatible Collection filter and loads the matching project list before committing selection.
- Content hierarchy: Project identity and launch actions first; task-oriented workspace tabs second; details, evidence, and destructive actions last. The header action row leads with the primary Open Agent launcher, followed by terminal, IDE, and file browser.
- Open Agent is the AI integration point. RepoAtlas does not host model selection, provider credentials, project chat, generated summaries, or AI memory; the launched Agent owns those concerns and may use RepoAtlas MCP for structured project and task context.
- Overview sections live in a compact tab-bar switcher, not a persistent 184px side index. Continue, Environment, README, AGENTS.md, Profile, and Notes are the six overview chapters.
- Project workspace tabs are Overview, Files, Git, and Tasks; Projects without Git omit Git. Files uses a roughly 320px lazy directory/search pane and a flexible read-only preview pane. Activating Files performs the first root read; leaving it preserves expansion, selection, query, and scroll state.
- Files keeps directory browsing and path search separate. Expanding a directory performs one direct-child read and caches it for the session. Search builds a bounded background index only after the first query. Generated-directory visibility and generated-directory search are separate choices, and Refresh is explicit because RepoAtlas does not watch Project files.
- Project rows behave like compact asset-index entries: the identity mark belongs to the title line, while path, description, stack, and time use the full card width below it. Selection uses only a neutral raised surface and subtle border, with no amber edge marker or full beige card. Project edits stay inside the active row.
- Project row actions live in one context menu. Pointer users open it with right click; keyboard users use `Shift+F10` or the context-menu key, with `F2` reserved for rename. No action buttons appear only on hover.
- Folder context menus include Rescan folder. It refreshes only the selected directory's intersection with existing Scan Roots, using the shared scan progress, cancellation, and completion feedback. It is disabled while scanning or when no Scan Root covers that folder; it never creates new authorization. A partial refresh does not mark sibling Projects unavailable or update a whole Scan Root's scan timestamp.
- MCP task requests surface in the title bar approval center and never execute before explicit desktop approval. MCP may manage RepoAtlas records and Scan Root authorizations directly, but it never deletes, moves, or edits a real project directory.
- Project Collections are a user-owned filter above the virtualized path tree. Creating, renaming, changing membership, or deleting a collection never changes a Project record or directory.
- The title-bar attention center combines Pending Approvals with failed Task Runs, Unavailable Projects, environment mismatches, and system failures. Acknowledgement hides only the current version; a later occurrence returns.
- The command palette search field is an inset neutral surface. Focus uses a low-saturation outer halo and never turns the full dialog edge amber; selection is graphite-first with amber limited to a narrow locator and icon state.
- The first two overview chapters use progressive emphasis: current branch and workspace health first, actionable Start here tasks second, recent activity and evidence provenance third. Runtime comparisons use compact cards with requirements and local versions paired, while mismatch states remain immediately scannable.

## Design principles
- Evidence before decoration: Project state, path, provenance, and safety boundaries stay more prominent than visual effects.
- Calm density: The library remains compact enough for many projects while the workspace uses whitespace and clear task groupings.
- Motion explains state: Every action has micro-feedback; expressive motion is reserved for discovery, the command palette, and project identity.
- Tradeoffs: Desktop clarity takes priority over mobile layouts; local/offline reliability takes priority over remote font or image assets.

## Visual language
- Color: Dark mode uses neutral graphite layers (`#0D0D0D`, `#151515`, `#1C1C1C`). Light mode uses only white, neutral gray, and charcoal (`#F6F6F6`, `#EEEEEE`, `#FFFFFF`, `#171717`). Brand actions use bright amber `#FFA31A`; light mode contains no blue, beige, or chromatic gray. Green, amber, and red are semantic only.
- Typography: Bundled Geist Variable for Latin and numerals, Geist Mono for paths/logs/code, and system CJK fallbacks.
- Spacing/layout rhythm: 4px base rhythm; 12/16px dense controls; 20/24/32px workspace grouping.
- Shape/radius/elevation: 8px controls, 12px rows, 16-20px elevated surfaces; tinted shadows with a consistent top-left light source.
- Motion: 120-160ms press/hover, 180-220ms selection/tab changes, 220-280ms overlays; transform and opacity only.
- Startup: the main window stays hidden until the first brand frame is ready. The splash draws the RepoAtlas mark and wordmark in 1200ms, then yields to the library once bootstrap has finished. Reduced motion shows the final static mark immediately.
- Task workbench: the global task surface is a full-window split, with the run list on the left and up to four live terminals on the right. xterm owns typed input; there is no second command field. Terminal output itself is not animated.
- Runtime observation sits between each Task Run header and terminal. It shows process-tree CPU, memory, process count, listening ports, and safe localhost links; metrics update without animating or replacing terminal output.
- Imagery/iconography: The application mark is a strong graphite tile containing three amber index bars and a continuous negative-space path, legible at favicon size. Phosphor icons use one optical weight. Outside the approved raster mark, do not use contour lines, map pins, gradients, stock imagery, or Unicode icons.
- Project and IDE marks may use restrained brand color only at 14-22px. Detected project icons are sanitized local thumbnails. No CDN icon fonts or broad filesystem image access.
- Project marks sit directly beside the project name without a universal tile, border, or button-like background. Real thumbnails may keep a small intrinsic corner radius; language fallbacks remain monochrome and visually secondary.

## Components
- Existing components to reuse: Base UI primitives, TanStack virtual list, Framer Motion, and the current typed Tauri API boundary.
- New/changed components: App mark, Dashboard featured Project, status rail and entry lists, path tree, Project Collection selector/editor, scope selector, fields, tabs, badges, skeletons, empty state, toast stack, scan status, attention center, project-description editor, structured Project Brief, Task Run monitor, virtualized Project File View, unified rendered Markdown, help/MCP setup, and page headers.
- Variants and states: Default, hover, pressed, focus-visible, selected, loading, success, warning, error, disabled, and unavailable.
- Token/component ownership: Global tokens and layout live in `src/styles.css`; behavior and accessible semantics live in `src/components/ui`.

## Accessibility
- Target standard: WCAG 2.2 AA for text, controls, focus, and keyboard operation.
- Keyboard/focus behavior: Visible focus rings, logical tab order, arrow-key command navigation, Escape dismissal, and no keyboard traps.
- Contrast/readability: Theme-specific semantic colors, readable muted text, tabular numerals, and wrapping/copy support for long paths.
- Screen-reader semantics: Labels for every field, `aria-current`/`aria-selected`, named dialogs, and live regions for status and errors.
- Reduced motion and sensory considerations: Framer Motion follows user preference; continuous decorative animation stops under reduced motion.

## Responsive behavior
- Supported breakpoints/devices: Tauri desktop at 1100x720 minimum; reference viewport 1440x920.
- Layout adaptations: At 1280px and above use a 360px project tree plus a flexible detail workspace; between 1100px and 1279px the tree is 320px. Settings and help use the full content width.
- Touch/hover differences: Mouse and keyboard are primary; controls keep at least 32px height and never rely on hover alone.

## Interaction states
- Loading: App-shell and project skeletons preserve layout; pending controls are disabled and keep their label context.
- Empty: Every collection and workspace tab explains why it is empty and offers the next valid action. A new empty library opens first-launch onboarding once; completing or skipping it writes a local UI preference, and Help can reopen the same guide without resetting that completed state.
 - First launch: A new empty library opens the onboarding dialog once. Completing or skipping it writes a local UI preference; Help can reopen the same guide without resetting that completed state.
- Error: Inline or toast feedback states what failed and offers retry/copy/dismiss when applicable.
- Success: Quiet status toasts confirm completion without exclamation marks.
- Disabled: Disabled controls explain prerequisites through adjacent copy or tooltips.
- Offline/slow network, if applicable: Core project management remains local. Agent network state and model requests stay in the external Agent client and never block RepoAtlas startup.

## Content voice
- Tone: Direct, calm, specific, and operational.
- Terminology: Follow `CONTEXT.md`; use Project, Scan Root, External Agent, Agent Request, and Task Run precisely.
- Microcopy rules: Describe consequences before confirmation, avoid jargon in recovery messages, and never imply project directories will be deleted.

## Implementation constraints
- Framework/styling system: React 19, TypeScript, Tailwind CSS 4, Base UI, Framer Motion, and Tauri 2.
- Design-token constraints: Theme values must come from CSS custom properties; no page-local color systems or external CDN assets.
- Performance constraints: Keep project and file-tree virtualization, load directories only when expanded, build the path index only on search, keep syntax highlighting off the UI thread, animate compositor properties only, and avoid unnecessary reloads or layout measurement.
- Compatibility constraints: Windows and macOS title bars, paths, system theme changes, Chinese/English, and offline startup.
- MCP boundary: project metadata, tasks, icons, location, existing bounded project evidence, reports, and Scan Root records are directly manageable over MCP. The Files directory, preview, image, and path-index commands are desktop-only and are not MCP tools. Model/provider configuration stays in the Agent client. Shell mode, Git writes, arbitrary command execution, and filesystem deletion are not exposed over MCP. Every MCP or desktop mutation is recorded in a local audit trail without secrets.
- Test/screenshot expectations: Verify every changed layer with focused checks. Visual changes require live Tauri inspection at 1440x920 and 1100x720 in light and dark themes; typecheck and component tests alone do not establish visual correctness.

## Open questions
- None for the current scope. Dependency graphs, SVG project-icon ingestion, SVN write operations, and configurable external launchers remain future work.

## Module discovery and guidance

- Overview starts with an expandable Modules and directory structure panel when cached constituents exist. Each entry shows its relative path, own stack, candidate/Module/independent Project status, evidence time, runtime requirements, and scoped task actions.
- Module launch actions use its validated directory. Independent entries navigate to their Project; explicit promotion refreshes the shared project library. Actions remain keyboard accessible with visible pending and result feedback.
- Directory grouping retains the root entry and metadata, promotes constituent Modules, and persists across refresh. Explain that restoring Project mode does not merge independent Projects. Group entries carry a Directory group badge.
- Help begins with two copyable Agent prompts: Initialize MCP, then Scan directories. Both preserve explicit absolute Scan Root authorization, per-Module task cwd, user-controlled promotion, and desktop execution approvals.

## Workspace visual hierarchy

- Home leads with a featured, available, unarchived recent Project and an explicit Open Project action. The neutral graphite surface is shared by both themes; amber is reserved for the entry action. Empty libraries show onboarding guidance, never invented project activity.
- Four cached status metrics form an unboxed rail. Recent Projects and Task Runs precede Collections and Attention; seven-day aggregates stay below actionable entries. Open entries retain their typed navigation callbacks.
- Use larger Project names, restrained section labels, whitespace and dividing rules instead of repeated nested cards. Avoid decorative gradients and continuous animation. Long names and paths truncate with full text available in tooltips; secondary text remains readable in both themes.
- Project headers use an ultra-compact, high-density split layout: project identity, branch/stack chips, and path line on the left paired directly with primary Agent/Terminal/IDE action buttons on the right, keeping vertical header height below 70px to preserve maximum viewport height for workspace content. Left sidebar adopts refined control heights, muted search containers, and distinct active project indicators.
