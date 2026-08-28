# Design

## Source of truth
- Status: Active
- Last refreshed: 2026-08-26
- Primary product surfaces: Desktop project library, project workspace, command palette, scanning, settings, AI knowledge.
- Evidence reviewed: `README.md`, `CONTEXT.md`, `docs/adr/*`, `src/App.tsx`, `src/components/*`, `src/styles.css`, and the Tauri window configuration.

## Brand
- Personality: Quietly technical, cartographic, precise, trustworthy, and crafted for long sessions.
- Trust signals: Local-first language, explicit authorization boundaries, calm status feedback, and reversible actions.
- Avoid: Blue-purple AI gradients, neon cyberpunk styling, ornamental motion, generic glass cards, and destructive wording that implies RepoAtlas owns project directories.

## Product goals
- Goals: Make large local code collections immediately legible; make scanning, launching, Git, tasks, and project knowledge fast and safe; make every action visibly acknowledged.
- Non-goals: A code editor, a filesystem graph, cloud synchronization, or a replacement for the system Git/IDE/terminal.
- Success signals: A new user can add a root and understand the result without help; a returning user can open a project or command in seconds; every async operation has a visible state and recovery path.

## Personas and jobs
- Primary personas: Developers and technical leads with many local checkouts across multiple languages and years of work.
- User jobs: Rediscover projects, understand current state, launch the right tool, run known tasks, perform conservative Git work, and retain project-specific knowledge.
- Key contexts of use: 1100px to large desktop windows on Windows and macOS, keyboard-heavy use, light and dark system themes, Chinese and English.

## Information architecture
- Primary navigation: The persistent left navigation is removed. Projects, favorites, recent, and archived are scopes in the project tree; settings and help live in the title bar and command palette.
- Core routes/screens: Path-grouped project collection, project workspace, full-width settings, help and MCP setup, global command palette, confirmation surfaces, and scanning status.
- Content hierarchy: Project identity and launch actions first; task-oriented workspace tabs second; details, evidence, and destructive actions last.
- Overview sections live in a compact tab-bar switcher, not a persistent 184px side index. Continue, Environment, README, AGENTS.md, Profile, and Notes are the six overview chapters.
- Project rows behave like compact asset-index entries: the identity mark belongs to the title line, while path, description, stack, and time use the full card width below it. Selection uses only a neutral raised surface and subtle border, with no amber edge marker or full beige card. Project edits stay inside the active row.
- Project row actions live in one context menu. Pointer users open it with right click; keyboard users use `Shift+F10` or the context-menu key, with `F2` reserved for rename. No action buttons appear only on hover.
- MCP task requests surface in the title bar approval center and never execute before explicit desktop approval. MCP may manage RepoAtlas records and Scan Root authorizations directly, but it never deletes, moves, or edits a real project directory.
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
- Imagery/iconography: The PNG application mark is a strong black tile containing one amber route and three nodes, legible at favicon size. Phosphor icons use one optical weight. No contour lines, map pins, gradients, stock imagery, or Unicode icons.
- Project and IDE marks may use restrained brand color only at 14-22px. Detected project icons are sanitized local thumbnails. No CDN icon fonts or broad filesystem image access.
- Project marks sit directly beside the project name without a universal tile, border, or button-like background. Real thumbnails may keep a small intrinsic corner radius; language fallbacks remain monochrome and visually secondary.

## Components
- Existing components to reuse: Base UI primitives, TanStack virtual list, Framer Motion, and the current typed Tauri API boundary.
- New/changed components: App mark, path tree, scope selector, fields, tabs, badges, skeletons, empty state, toast stack, scan status, approval center, project-description editor, rendered Markdown README and Atlas Report, help/MCP setup, and page headers.
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
- Empty: Every collection and workspace tab explains why it is empty and offers the next valid action.
- Error: Inline or toast feedback states what failed and offers retry/copy/dismiss when applicable.
- Success: Quiet status toasts confirm completion without exclamation marks.
- Disabled: Disabled controls explain prerequisites through adjacent copy or tooltips.
- Offline/slow network, if applicable: Core project management remains local; external AI operations retain explicit provider context and a visible busy state.

## Content voice
- Tone: Direct, calm, specific, and operational.
- Terminology: Follow `CONTEXT.md`; use Project, Scan Root, AI Summary, AI Memory, and Task Run precisely.
- Microcopy rules: Describe consequences before confirmation, avoid jargon in recovery messages, and never imply project directories will be deleted.

## Implementation constraints
- Framework/styling system: React 19, TypeScript, Tailwind CSS 4, Base UI, Framer Motion, and Tauri 2.
- Design-token constraints: Theme values must come from CSS custom properties; no page-local color systems or external CDN assets.
- Performance constraints: Keep project virtualization, animate compositor properties only, and avoid unnecessary reloads or layout measurement.
- Compatibility constraints: Windows and macOS title bars, paths, system theme changes, Chinese/English, and offline startup.
- MCP boundary: project metadata, tasks, icons, location, knowledge, reports, and Scan Root records are directly manageable over MCP. Provider credentials, external AI requests, shell mode, Git writes, and filesystem deletion remain desktop-only. Every MCP or desktop mutation is recorded in a local audit trail without secrets.
- Test/screenshot expectations: Typecheck, production build, Rust Core/MCP checks, and interaction tests. The current scope explicitly omits full-application manual screenshot review; only the generated Logo is checked for silhouette, transparency, and small-size recognition.

## Open questions
- None for the current scope. Dependency graphs, SVG project-icon ingestion, SVN write operations, and configurable external launchers remain future work.
