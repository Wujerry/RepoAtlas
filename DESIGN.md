# Design

## Source of truth
- Status: Active
- Last refreshed: 2026-10-08
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
- Unified work search joins Projects, authorized Session text and commands in Ctrl/Cmd+K. Ctrl/Cmd+Shift+K opens the same search in an independent 760x560 desktop window while RepoAtlas is running. Escape/Close hides that window; closing the main desktop exits normally. Shortcut conflicts remain visible in Settings and the palette, with retry and a pointer-accessible search-window action.
- Empty search leads with recent Sessions; a query puts matching Projects before Sessions so a named Project remains one Enter away. All/Sessions/Projects/Actions scope buttons narrow the same results. Recent rows emphasize a two-line title plus Project, Agent and time; matched excerpts appear only for a query. Full paths stay in the selected preview instead of repeating in every row. Arrow keys and clicks select; incidental pointer movement never changes the preview. Enter focuses a Session preview and Tab reaches its actions. Full-session navigation preserves the hit's message position. IME composition never activates a result. Source authorization/refresh remains in Sessions; cached search never silently expands access.
- The wider search preview starts with the matching message or latest meaningful message; bounded surrounding context is collapsed until requested. Long messages expand on demand, with late search matches visible in the initial excerpt. Copy next to the message always uses its original text; Open Project sits next to the original path. The persistent primary action names the original Agent, with full-session navigation secondary. Known client envelopes are omitted from display excerpts; invalid cached titles can use a literal user line from the first 20 already-indexed messages, without rewriting sources or generating summaries. Continue retains its validated source/ID/cwd launch panel and explicit failure feedback. The main palette fits 1100x720; both windows share light/dark tokens, locale and reduced-motion behavior.
- Sessions is a full-workspace overlay entered through the title bar, command palette, Settings, or Continue Coding. It preserves project-tree state and its own filters, selection and scroll positions. Results aggregate by Session; selecting one previews visible messages, while its explicit Continue action starts the Agent.
- Named Agents in session cards, results, previews, sources, authorization summaries, launch targets, and filters pair their text with the shared local monochrome brand glyph. Agent pickers use Base UI Select so both the selected value and options retain their icons and keyboard access.
- Continue Coding follows actionable attention on the Dashboard with at most three available, unarchived Projects, one resumable Session each. Project Overview shows its latest resumable, non-archived Session. Open Agent remains available when no history can resume.
- Session Sources show absolute directories before authorization and explain local indexing plus MCP access. Refresh shows cache, progress, cancellation and per-source failures. Revocation clears visible excerpts across open search windows.
- Primary navigation: The persistent left navigation is removed. Projects, favorites, recent, and archived are scopes in the project tree; settings and help live in the title bar and command palette.
- Core routes/screens: Default Dashboard, path-grouped project collection, project workspace, full-workspace modal settings, help and MCP setup, global command palette, first-launch onboarding, confirmation surfaces, and scanning status. Settings and Help overlay the current screen so closing either preserves the underlying selection, filters, tree expansion, and scroll position.
- The Dashboard is the returning-user home. It combines a compact current-state summary with terminal Task Run results from the previous seven days, puts actionable Attention Items and bounded recent work first, and keeps Project Collections and seven-day aggregates below. Every value comes from the local Core snapshot and shows its update time; the Dashboard never triggers scanning, live Git inspection, Project file reads, or AI-generated interpretation.
- Selecting a Dashboard Project opens its existing workspace. Selecting a Project Collection resets search and facet filters, applies that Collection to the persistent project tree, and opens its first non-archived Project; an empty Collection remains on the Dashboard. The title-bar brand and command palette both return to the Dashboard without remounting the project tree.
- First-launch onboarding is a skippable three-step dialog: welcome, MCP one-step add, then waiting for a shared-database result. Manual Scan Root discovery remains the fallback when MCP is unavailable or the user wants bulk import.
- Shared-database changes continue to refresh the library after onboarding, including Agent edits to existing descriptions, tasks, and Collections. This preserves the current selection and filters where possible. Global Project navigation clears an incompatible Collection filter and loads the matching project list before committing selection.
- Content hierarchy: Project identity and launch actions first; task-oriented workspace tabs second; details, evidence, and destructive actions last. The header action row leads with the primary Open Agent launcher, followed by terminal, IDE, and file browser.
- Open Agent is the AI integration point. RepoAtlas does not host model selection, provider credentials, project chat, generated summaries, or AI memory; the launched Agent owns those concerns and may use RepoAtlas MCP for structured project and task context.
- Overview sections live in a compact tab-bar switcher, not a persistent 184px side index. Continue, Environment, Modules, README, AGENTS.md, Profile, and Notes are the seven overview chapters; the Modules chapter appears only when the Project has detected modules or directory grouping is enabled.
- Environment evidence files use a flat list with fine dividers, small technology/file glyphs, and trailing categories. Deduplicate paths across manifest, package-manager, lockfile, and runtime evidence; asset files remain outside this text-preview list. File previews use a wide, expandable dialog with the shared theme-aware syntax highlighting and virtualized line numbers, horizontal scrolling, content/path copy, external actions, and a visible truncation notice. Keep controls keyboard-accessible and reject stale reads after selection changes.
- Project workspace tabs are Overview, Files, Git, and Tasks; Projects without Git omit Git. Files uses a roughly 320px lazy directory/search pane and a flexible read-only preview pane. Activating Files performs the first root read; leaving it preserves expansion, selection, query, and scroll state.
- Files keeps directory browsing and path search separate. Expanding a directory performs one direct-child read and caches it for the session. Search builds a bounded background index only after the first query. Generated-directory visibility and generated-directory search are separate choices, and Refresh is explicit because RepoAtlas does not watch Project files.
- Project rows behave like compact asset-index entries: the identity mark belongs to the title line, while path, description, stack, and time use the full card width below it. Selection uses only a neutral raised surface and subtle border, with no amber edge marker or full beige card. Project edits stay inside the active row.
- Project row actions live in one context menu. Pointer users open it with right click; keyboard users use `Shift+F10` or the context-menu key, with `F2` reserved for rename. No action buttons appear only on hover.
- Folder context menus include Rescan folder. It refreshes only the selected directory's intersection with existing Scan Roots, using the shared scan progress, cancellation, and completion feedback. It is disabled while scanning or when no Scan Root covers that folder; it never creates new authorization. A partial refresh does not mark sibling Projects unavailable or update a whole Scan Root's scan timestamp.
- MCP task requests surface in the title bar approval center and never execute before explicit desktop approval. MCP may manage RepoAtlas records and Scan Root authorizations directly, but it never deletes, moves, or edits a real project directory.
- Project Collections are a user-owned filter above the virtualized path tree. Creating, renaming, changing membership, or deleting a collection never changes a Project record or directory.
- The title-bar attention center combines Pending Approvals with failed Task Runs, Unavailable Projects, environment mismatches, and system failures. Acknowledgement hides only the current version; a later occurrence returns.
- Once a signed update is discovered, downloading, ready to install, or fails after discovery, the title bar shows an update entry with an accent dot. Its panel carries the release notes plus manual check, download, install, restart, and postpone actions. A completed check that finds nothing shows no title-bar entry.
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
- Title-bar navigation glyphs play one 220ms selection animation: Sessions pulse, Tasks nudge forward, Ports connect diagonally, Footprints step, Attention rings, Help tilts, Settings turns, and Usage rotates into place. Labels and hit areas stay fixed; count refreshes do not replay motion. Pointer, keyboard and command navigation share the selected state. Reduced motion keeps the glyphs static with the existing selection colors.
- Startup: the main window stays hidden until the first brand frame is ready. The splash draws the RepoAtlas mark and wordmark in 1200ms, then yields to the library once bootstrap has finished. Reduced motion shows the final static mark immediately.
- Task workbench: the global task surface fills the content below the persistent 48px title bar, with the run list on the left and up to four live terminals on the right. xterm owns typed input; there is no second command field. Terminal output itself is not animated.
- The task workbench keeps its title, layout switch, stop-all action and close control in one toolbar across the page. The independently scrolling run list supports Project, task kind and command search; filtering never changes the displayed terminal. Run rows show Project, command, status and elapsed time or recorded time. Empty output offers a return to Projects and, when available, recent output.
- Runtime observation sits between each Task Run header and terminal. Current CPU, memory, listening ports, conflicts and safe localhost links stay visible; peak metrics and process count expand on demand. Metrics update without animating or replacing terminal output. Output fills the remaining pane even when no runtime observations exist. Two terminals stack vertically at the minimum window width.
- Project Tasks uses compact definition cards and a history count. History moves below definitions when the workspace itself is narrow; card actions wrap only when needed. Search/filter/sort and task editing retain their existing behavior. The collapsible output dock keeps accessible theme, clear and copy controls compact and lets its toolbar wrap without pushing output outside the dock.
- Task card Edit and Remove actions appear on card hover or focus within on fine-pointer devices. Reserve their space to avoid layout shifts, keep Run visible, and show all actions continuously on devices without hover.
- Imagery/iconography: The application mark is a strong graphite tile containing three amber index bars and a continuous negative-space path, legible at favicon size. Phosphor icons use one optical weight. Outside the approved raster mark, do not use contour lines, map pins, gradients, stock imagery, or Unicode icons.
- Project and IDE marks may use restrained brand color only at 14-22px. Detected project icons are sanitized local thumbnails. No CDN icon fonts or broad filesystem image access.
- Project marks sit directly beside the project name without a universal tile, border, or button-like background. Real thumbnails may keep a small intrinsic corner radius; language fallbacks remain monochrome and visually secondary.

## Components
- Page chrome stays compact: Home, Settings, Help, Sessions, Footprints and the task workbench use one small identity/action row without decorative eyebrows or stacked introductory copy. Usage combines its view switcher, connection count, refresh and close in one fixed toolbar. Keep snapshot/cache status, observation times, authorization explanations, errors and recovery actions visible in their relevant working areas. Project identity, source paths and the Footprints date navigator remain functional content, not decorative headings. Continue Coding uses a title/count/action row without an icon tile or repeated instruction.
- Existing components to reuse: Base UI primitives, TanStack virtual list, Framer Motion, and the current typed Tauri API boundary.
- New/changed components: App mark, Dashboard recent work, status rail and entry lists, path tree, Project Collection selector/editor, scope selector, fields, tabs, badges, skeletons, empty state, toast stack, scan status, attention center, project-description editor, structured Project Brief, Task Run monitor, virtualized Project File View, unified rendered Markdown, help/MCP setup, and page headers.
- Variants and states: Default, hover, pressed, focus-visible, selected, loading, success, warning, error, disabled, and unavailable.
- Token/component ownership: Global tokens and layout live in `src/styles.css`; behavior and accessible semantics live in `src/components/ui`.

## Accessibility
- Target standard: WCAG 2.2 AA for text, controls, focus, and keyboard operation.
- Keyboard/focus behavior: Visible focus rings, logical tab order, arrow-key command navigation, Escape dismissal, and no keyboard traps.
- Contrast/readability: Theme-specific semantic colors, readable muted text, tabular numerals, and wrapping/copy support for long paths.
- Screen-reader semantics: Labels for every field, `aria-current`/`aria-selected`, named dialogs, and live regions for status and errors.
- Reduced motion and sensory considerations: Framer Motion follows user preference; continuous decorative animation stops under reduced motion.

## Responsive behavior
- Switching Projects keeps the previously active workspace tab; an unavailable tab (such as Git for a non-Git Project) falls back to Overview. Project-specific file and task state resets independently of the active tab. Task Definitions initially sort by latest Task Run start time descending, with unrun tasks last and original order for ties.
- Supported breakpoints/devices: Tauri desktop at 1100x720 minimum; reference viewport 1440x920.
- Layout adaptations: At 1280px and above use a 360px project tree plus a flexible detail workspace; between 1100px and 1279px the tree is 320px. Settings and help use the full content width.
- Touch/hover differences: Mouse and keyboard are primary; controls keep at least 32px height and never rely on hover alone.

## Interaction states
- Correctness: non-Git Projects show a neutral VCS explanation without Git counters. Missing Git observations and nullable counters are unknown, never clean or zero by default. Pending revalidation and failed updates explicitly label retained observations; only a successfully read known state uses success styling. Show observation time and refresh/retry actions.
- Recent Sessions distinguish initial loading, empty, failed reads and failed index refresh. Empty cards offer source management and an existing Project/Agent entry. Search success must not clear unrelated source or refresh failures. Chinese navigation consistently says 会话.
- An absent optional AGENTS.md shows the neutral no-guide state; unreadable files and unavailable Project directories retain explicit failure and retry feedback.
- Navigation and density: task, port, attention and Footprints entries retain visible text at 1100px. Project scopes use icons plus text. Compress directory chains only when there is one child and no direct Project; preserve branch points, real action-target paths and keyboard parent navigation, and offer the full hierarchy in the existing tree-view button's dropdown, with a checkmark for the current mode; this preference never occupies a separate toolbar row. For five or fewer Task Definitions, reveal advanced filters/sorting on demand; keep active filters visible and empty history compact.
- Restart: restore the main window's size, position and maximized state while keeping the splash visibility flow. Restore the last full-page surface, selected Project, Project tab, Settings category, list scope, Collection, search, filters and sorting. Keep the existing sidebar-width preference. Deleted selections return Home; invalid tabs use their default. Do not reopen confirmation dialogs or resume operations. Title-bar window controls and window focus changes do not dismiss the full-page surface before its state is saved.
- Loading: App-shell and project skeletons preserve layout; pending controls are disabled and keep their label context.
- Empty: Every collection and workspace tab explains why it is empty and offers the next valid action. A new empty library opens first-launch onboarding once; completing or skipping it writes a local UI preference, and Help can reopen the same guide without resetting that completed state.
 - First launch: A new empty library opens the onboarding dialog once. Completing or skipping it writes a local UI preference; Help can reopen the same guide without resetting that completed state.
- Error: Inline or toast feedback states what failed and offers retry/copy/dismiss when applicable.
- Success: Quiet status toasts confirm completion without exclamation marks.
- Disabled: Disabled controls explain prerequisites through adjacent copy or tooltips.
- Offline/slow network, if applicable: Core project management remains local. Agent network state and model requests stay in the external Agent client and never block RepoAtlas startup.

## Content voice
- Task history identifies runs by the associated Task Definition name and recorded command, with status and time. Runs associate by Task ID, never task kind. Selecting history opens its output, clears filters hiding the associated task, scrolls it into view, focuses the card and highlights it for 1.8 seconds. Reduced motion uses immediate scrolling. Removed or unassociated tasks retain their recorded command/output and show an unavailable-definition notice instead of selecting another task.
- A user-ended Task Run is labeled 已停止 / Stopped in the task workspace, workbench, Project Overview, history and Dashboard totals. It is a neutral terminal state, distinct from success/failure; canceled scans and refreshes retain their cancellation wording.
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

## Usage and session readability

- Usage is a full-page, nonmodal surface below the persistent title bar. Subscription
  windows and remaining quota lead; local recorded tokens have a separate history
  view. Never equate quota percentages with tokens.
- The title bar shows every connected provider whose navigation switch is enabled
  immediately after the RepoAtlas brand on the left. Sessions and Tasks share one
  navigation group after global search, separated from the remaining work tools.
  Each visible provider shows its most constrained remaining quota. There is no count cap or overflow badge;
  the quota rail scrolls horizontally by wheel, trackpad or arrow keys when space
  is insufficient, while the work tools and window controls stay fixed.
  Its popover exposes all periods, reset
  times, cached/error status, refresh and account management. Monetary balances
  keep their currency; uncapped spend is explicitly labeled used.
  Only the full Usage page provides per-service visibility switches with
  pending/save/error feedback. The popover focuses on quota, reset times and
  per-period meters, without switches or navigation-setting helper text.
  The popover has an inset thin scrollbar and a fixed footer with a rounded, quiet
  hover target for account management; scrolling never moves the footer.
  Hiding affects only the inline readout; the complete Usage page and connections
  remain available. All-hidden falls back to the compact Usage entry.
- Running tasks, Footprints, attention center and help remain directly visible in
  the title bar at the minimum width; never move these four actions into overflow.
  Search flexes to available width and provider readouts remain compact.
- Settings use a stable category rail: appearance/interaction, Projects/Sessions,
  data/backup and updates. Content scrolls independently and switching categories
  preserves form state. Navigation visibility belongs with the corresponding quota.
- Compact horizontal account rows align provider identity with up to three quota
  columns. Disconnected services live in an expandable catalog. Avoid a promotional
  hero and unequal card heights. Monochrome brand glyphs, tabular counts and slim meters
  show used quota; the large number explicitly says remaining. Amber marks high
  consumption and semantic danger marks near exhaustion. Reset countdowns expose
  absolute timestamps on hover. Expired/failed observations stay labeled as pending
  or cached instead of assuming replenishment.
- Each connection has an explicit action disclosing its credential source and
  destination. Unknown plans/counters are not fabricated. Keyboard actions and
  pending/failure feedback remain visible; title-bar navigation stays available.
- Rows, previews and Continue Coding share compact token notation and API-equivalent
  USD estimates, with partial pricing labeled. Expandable breakdowns expose coverage,
  pricing date and assumptions; unknown costs never look like a free session.
  Sessions devote more width to the reader, keep Open Project and Continue visible,
  and collapse advanced filters, technical metadata and consecutive client notices.
  Usage details and Session information open in bounded, keyboard-accessible portal
  popovers. They must not resize the header, displace messages or reset reading position.
  All message readers use bounded Markdown with headings, lists, tables
  and code. Known transport envelopes are folded; original-view and verbatim copy
  remain available. Never automatically fetch message images.
- All session surfaces share the reader: full history, selected search results and
  surrounding messages. Home/Project Continue cards and both search result lists use
  bounded inline Markdown excerpts with emphasis/code and no nested links or controls.
  Code blocks have language, wrap and copy actions; highlighting runs in the existing
  worker only when visible. Tables scroll within their message, task lists are read-only,
  and safe disclosure HTML is sanitized. Explicit double-dollar math is rendered;
  single-dollar amounts remain text. Search highlighting preserves Markdown structure.
  Initial messages render at most 4,000 characters (1,200 in search previews); expanded
  messages page through 12,000-character sections without a total reading cutoff.
  Source offsets preserve code fences and every character; original copy is unmodified.
- Usage and the title bar read cached metadata first and query enabled providers only
  while visible, with five-minute background refresh and Core throttling. Countdown
  updates do not query providers. Summary queries read
  metadata without decoding transcripts or probing Agent installations.

## Module discovery and guidance

- Overview starts with an expandable Modules and directory structure panel when cached constituents exist. Each entry shows its relative path, own stack, candidate/Module/independent Project status, evidence time, runtime requirements, and scoped task actions.
- Module launch actions use its validated directory. Independent entries navigate to their Project; explicit promotion refreshes the shared project library. Actions remain keyboard accessible with visible pending and result feedback.
- Directory grouping retains the root entry and metadata, promotes constituent Modules, and persists across refresh. Explain that restoring Project mode does not merge independent Projects. Group entries carry a Directory group badge.
- Help begins with two copyable Agent prompts: Initialize MCP, then Scan directories. Both preserve explicit absolute Scan Root authorization, per-Module task cwd, user-controlled promotion, and desktop execution approvals.

## Workspace visual hierarchy

- Rapid project selection highlights the selected row immediately and settles for 120ms before replacing the workspace or starting its requests. Keep the previous workspace visible during the wait; cancel pending selection on return to Home. Cached details appear after selection settles, without waiting for revalidation. Project Continue Coding reads cached indexed sessions rather than initiating an index refresh on each switch.

- Selecting a Project inside already expanded folders preserves tree rows and virtual-list measurements. Icon loads share in-flight requests by Project and revision, keep completed offscreen results, and reject outdated results after a revision or manual icon change. Each request stays capped at 48 Projects.

- Revisited Project Overviews show retained details immediately, with a small loading spinner while revalidation runs. The title hides the normal Ready badge and retains the Unavailable warning; the spinner keeps an accessible status label and respects reduced motion. Keep cached README/environment content mounted during background updates and retain it alongside a retryable error if updating fails. Do not replay Project-switch motion for same-Project revalidation. Manual refresh and library mutations invalidate overview resources; ordinary selection reuses fresh Git, environment, document and installed-tool results.

- Selecting a Project reveals its title with a 220ms fade and 6px upward settling motion, accompanied by a single 320ms amber line sweep at the header edge. Only Project identity changes replay these effects; refreshes and log updates do not. While loading, dim only the header. Preserve the workspace, scroll containers, action controls and Markdown image nodes; reduced motion removes both entry effects. No exit animation delays data loading or input.

- Home leads with a compact heading and current-state strip, followed by actionable attention, recent Sessions, Projects and Task Runs. Empty recent work offers Add Project and, when Projects exist, Find and open a Project; never invent activity.
- Four cached status metrics form an unboxed rail. Attention precedes recent work; recent Projects and Task Runs precede Collections; seven-day aggregates stay below actionable entries. Open entries retain their typed navigation callbacks.
- Use larger Project names, restrained section labels, whitespace and dividing rules instead of repeated nested cards. Avoid decorative gradients and continuous animation. Long names and paths truncate with full text available in tooltips; secondary text remains readable in both themes.
- Project headers use an ultra-compact, high-density split layout: project identity, branch/stack chips, and path line on the left paired directly with primary Agent/Terminal/IDE action buttons on the right, keeping vertical header height below 70px to preserve maximum viewport height for workspace content. Left sidebar adopts refined control heights, muted search containers, and distinct active project indicators.

## Footprints time navigator

Explicit scrolling at the timeline boundaries navigates adjacent local dates: down to the previous day after its current pages finish, up to the next day no later than today. Wheel input also works on empty or short days; Page Up/Down provides keyboard access. Keep the old records visible during the request, then update the date and list together without entry animation. Empty days are not skipped. A newer day with a single page lands at the bottom; a paginated newer day opens at its first page to preserve bounded loading and access to all its records. Repeated wheel events are throttled and requests remain single-flight.

Footprints occupies the content area below the 48px title bar and returns focus to its entry when closed. A large local date, continuous 30-day activity track, fixed-height virtual timeline, and 340px inspector form its hierarchy (300px inspector at the minimum window). Zero-activity dates remain visible; choosing a day never silently selects another day. Search applies to the entire selected local day, not just loaded rows. Project filters are keyboard-searchable.

This surface deliberately extends the visual contract: subtle static amber radial light and a local date-track gradient are allowed in Footprints only. Graphite/white theme tokens, monochrome category icons and semantic result colors remain shared. No particles, full-screen blur, cursor lighting or continuous decorative motion. Phosphor regular/duotone layers crossfade without changing geometry.

Opening uses opacity and 8px translation over 220ms, date selection uses a 180ms track cursor, category selection 160ms, and detail selection 140ms with 6px translation. List scrolling and pagination never replay entry animation. Reduced motion removes spatial animations; hidden windows pause animation and history polling. Virtual rows are 68px, hour separators 32px, with eight rows of overscan in each direction.

Cache results are shown before bounded automatic Git updates. Progress, cancellation, partial failure, coverage and a deliberate new-records action prevent background activity from moving the current reading position. Full commit bodies load only on selection. The timeline retains focus at its listbox container while arrow keys update the active row.

Sessions title-bar navigation uses a labeled Brain icon with pressed/fill and amber border states. Source management supports one batch authorization confirmation listing every absolute path, with partial failures retained for retry. Continue opens a launch panel showing cwd and copyable command, pending/failure feedback and App/CLI selection when both are detected; unsupported App resume is explicitly unavailable.

The Continue a session region uses two columns (three on wide desktops) of bordered, equal-height cards. Project identity and Agent form the header, a two-line session title and two-line plain user excerpt form the body, and time plus Preview/Continue actions occupy a fixed footer. Preview targets the exact session, not just its Project. Source transcript text stays unmodified; only display excerpts decode export entities and remove Markdown decoration.

Home opens with a compact workspace heading and status strip, followed by actionable attention, up to three recent sessions and bounded recent projects/tasks. The oversized featured-project block is removed. Session rows show stored project icons and a distinct project navigation action. Full-page Sessions, Footprints, Help and Settings overlays are nonmodal relative to the persistent title bar; switching closes the previous page in one action while the covered project workspace is inert. Confirmation dialogs remain modal. Search focus is a single subtle outer field ring, and selection uses a narrow amber rail with a low-contrast fill.

The home workspace fills the entire detail pane, including its scroll area, with 22–32px edge padding and no centered maximum-width column. At 1100px of available session-section width, the three recent sessions form equal-width cards with aligned action footers; narrower sections retain compact rows.

Project titles expose the rename action on title-row hover or keyboard focus, keeping its layout space reserved. Devices without hover show it continuously. The dialog initially selects the current display name, trims saved names, retains failed drafts, and returns focus to the title action. Renaming changes the Project record only; it never renames the directory.

The Collection editor scales with the main window at 92% viewport width and 90% viewport height, retaining at least 16px edge clearance. The details column grows from 280px to 400px while membership takes the remaining width.

The Collection editor uses a two-column layout: name and optional description on the left, searchable membership on the right. Keep the header and save/cancel footer fixed while the project list scrolls. Show selected counts, All/Selected views, and actions scoped to current results. Empty Collections are allowed. Membership survives search and parent refreshes; load failures disable saving and failed saves preserve drafts. Focus starts on the name and returns to the collection trigger. Search uses one outer focus ring, rows use an inset keyboard focus outline, and the virtualized checkbox list uses one Tab stop with Up/Down/Home/End navigation and Space to toggle.

## Windows ports and processes

The Ports surface starts with a single compact row of attribution filters, search, Refresh and Close. Page identity comes from the active navigation entry and accessible heading rather than a repeated hero title, decorative icon or subtitle. Result count, snapshot context and last-read time share a secondary metadata line above the table; the Windows listener scope remains available through the page description and metadata tooltip.

The listener table sorts through keyboard-accessible column headers for port, process name, PID and Project. The active header exposes its direction with `aria-sort` and an arrow; repeated activation reverses direction. Default order is ascending by each process's lowest listening port. Names use natural ordering, unknown Projects stay last in either direction, and equal values use ascending PID for stability. Search, attribution filters and manual refresh retain the chosen sort while the surface is open. PID has its own compact secondary-text column.

The listener table uses compact rows with small neutral port labels, medium-weight process names, secondary PIDs and Project links. Full executable paths remain visible below process names, wrap naturally and allow text selection; unavailable paths are explicitly labeled. Each row identifies a managed Task Run versus an external process. Project checkout paths and attribution evidence remain visible so same-name Projects can be distinguished. Restricted processes show one right-aligned explanation that combines cause and outcome, such as Cannot stop: permission denied. Never split a generic unavailable label and its reason into separate lines or alignments. These essential facts require no disclosure or hover. TCP and snapshot timing appear once above the table. Normal stop is offered for eligible managed runs, with the external-terminal requirement stated once in the footer. All services, Linked to Projects and Unknown Project filters show snapshot counts and combine with search. Destructive actions retain their semantic danger color and identity-bound confirmations.

The top navigation bar provides an independent Ports & processes entry next to Active Tasks. It opens a full-page, nonmodal surface below the persistent title bar, also reachable from the command palette and port-conflict review; it is not nested inside the task workbench. A searchable compact table shows TCP ports, PID, process and Project, with explicit attribution evidence and Unknown state. Refresh is manual. Single-instance Project association has searchable choices and visible pending/result/error feedback. Normal stop and force termination are distinct actions. A modal confirmation lists the target executable, Project, affected processes and all observed listening ports, plus process-tree scope for managed tasks or single-process scope for external processes. Confirmations expire after 60 seconds and never survive closing the surface. External normal stop explains the original terminal/IDE requirement; no automatic escalation or restart is implied. Shared neutral theme tokens, Base UI confirmation dialogs, keyboard access and reduced motion apply.
