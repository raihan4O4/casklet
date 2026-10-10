# Desktop GUI design

Implemented 2026-09-30. See ADR 0018 for window and application decisions.

The main window uses a narrow navigation rail, page header with search and creation, three compact summaries, and a sortable inventory list. A row opens its dedicated native workspace without replacing inventory. Menus provide secondary actions. Global Installations, Activity, and Settings expose discovery, operation/recovery status, database backup, and diagnostics.

Each Cask workspace has a compact title/description, five tabs, an independently scrolling content area, and persistent Add Items / red Cleanup navigation actions. Overview summarizes measured logical storage, counts, location, and dates. Files & Folders identifies managed, referenced, and shared associations, with compact name/size rows and expandable inline location, ownership, availability, measurement, and category details. Applications separates portable installations, registered associations, WinGet lifecycle, and reviewed installer artifacts. Advanced workflows are revealed deliberately through application-method choices. Activity queries durable state; Settings edits metadata, archives without deleting content, manages categories, and exports.

Resource intake explicitly offers file, folder, and reference selection and requires explicit reference or in-place ownership choice and consent; generic Copy/Move intake is temporarily disabled under ADR 0020. Drops never imply a move. Cleanup shows the application's plan and warnings before approval; execution remains disabled in distributable builds pending independent review.

The visual system uses local Inter, charcoal controls, white/light gray surfaces, one-pixel neutral borders, 6–8 px radii, and Lucide outline icons. Green/yellow/red are reserved for state. Paths and names truncate with full values available through native browser tooltips. There are no browser responsive breakpoints, fabricated capacities, decorative charts, or permanent right-side inspector.

## Interaction polish

Cask identity accents are assigned deterministically from stable IDs using a restrained palette; they are not health or ownership indicators. Lifecycle state remains separately labeled. Only the active sort column shows its direction, and repeated clicks toggle it. One action menu is active per window, dismissing on outside pointer/focus, Escape, or selection. Keyboard arrow navigation and reduced motion are supported.

Notifications float at the top without affecting layout and expire automatically. Keyed progress updates replace prior notices; durable operation queries remain authoritative. Activity uses chronological date groups and subdued metadata. Resource intake separates source, explicit reference/ownership choice, category, explanation, and commit action. The existing application picker separates search, results, installation metadata, and association.

Both native windows start hidden with explicit white native and document backgrounds. After initial queries, Svelte rendering, font readiness, and synchronous layout, the invoking WebView requests its own native reveal/focus. Hidden windows do not depend on potentially throttled animation frames. Existing initialized workspaces are focused rather than recreated.

## Polish verification (2026-09-30)

`cargo xtask check` passed Rust formatting, Clippy, workspace tests/build, frontend formatting, ESLint, Svelte diagnostics, UI tests, and the frontend build. Final frontend checks include nine tests covering existing safety workflows plus sort direction, menu replacement/outside dismissal/Escape focus, keyed expiring notifications, window readiness, and stable varied identity accents. The Windows NSIS bundle builds with normal release feature gates.

Native smoke checks used isolated QA storage: inventory, date-grouped Activity, menu placement/outside dismissal, and separate initialized detail-window appearance. Frame-by-frame flicker measurement, clean-machine installation, high-DPI, and full assistive-technology release checks remain manual release gates.

## Cleanup refinement

Cleanup is the single footer entry for resource cleanup and confirmed empty-Cask removal; navigation creates no plan. Back to Overview uses a bordered button with a back icon and existing focus states. Main deletion-method radios choose Recycle Bin or eligible permanent deletion. Changing method or scope clears the preview and review acknowledgement. Permanent mode explains preservation of folders/external data and requires a separate irreversible-deletion confirmation at execution.

Cleanup Review shows a count/effect summary, compact named items, a visual review state, and actions. A Review only notice appears before interaction when the capability is closed. Approval/execution remain visibly disabled with accessible descriptions and hints; the independent security-review rationale stays in Advanced details. A local hash-bound Reviewed state is distinct from durable core approval. Resource disclosures use brief motion, keyboard controls, wrapped long paths, and reduced-motion support. Adding a resource leaves details collapsed.

## Alpha readiness

CleanupReview is a controlled presentation component; pure text/status helpers live in cleanup.ts. Workspace retains IPC orchestration; Rust remains the lifecycle/ownership authority. Alpha capability enables file review/approval/execution, while WinGet uses its independent provider capability. Durable cleanup history and interrupted recovery are discoverable after restart. Worker errors reload the persisted plan and reset confirmations. Startup database errors produce a native message instead of an invisible failure. Full manual screen-reader/high-DPI checks remain pending.

## Light and Dark appearance

Global Settings offers keyboard-accessible Light/Dark radios. Light remains the default. The shared dark palette distinguishes application background, navigation, panels, secondary surfaces, fields, floating surfaces, hover and selection. Typography, icons, status treatments, control borders, focus rings, disabled states, scrollbars, storage bars, dialogs, notifications, application badges and cleanup workflows use semantic roles. Original light colors are retained as role fallbacks. Native tooltips and select controls inherit the explicit color scheme.

The supplied transparent black and white logos are bundled unchanged and switch in both sidebar and caption. Both assets are preloaded. Saved appearance is applied before mounting; native theme and background are set before window reveal. Open windows synchronize through validated Tauri events and storage changes. Palette changes apply atomically without layout movement or low-contrast intermediate colors; existing hover and interaction motion respects reduced motion. See ADR 0027 for persistence and boundary decisions.

Appearance verification (2026-10-07): `cargo xtask check` passed with 117 all-feature Rust tests (one opt-in benchmark ignored), default/Alpha profile checks, 37 GUI tests, strict linting, formatting, zero Svelte diagnostics, contract verification and production frontend build. Theme tests cover persistence, invalid values, logos, native-before-reveal ordering, event-name validity, cross-window changes, rapid toggles, storage failures and contrast. Supplied logo assets match their originals byte-for-byte. Native isolated-QA checks confirmed light/dark inventory, Settings, dark startup restoration, creation dialog/focus/disabled states and bidirectional synchronization of an open Cask workspace. Final frontend checks and the normal native debug build passed after the atomic-switch refinement. Evidence: target/dark-theme-quality.log. Full assistive-technology and high-DPI release gates remain separate.

## Native Cask drop feedback (October 10)

Individual Cask windows use their existing Tauri drag/drop subscription for feedback. A single file or folder entering a loaded Cask window displays a dashed theme-colored inset border, subtle tint, and a compact centered drop card. The former bottom strip read as a footer instruction; the transient 260px-wide, approximately square card establishes a clearer target with a folder icon, named primary instruction and concise review guidance. Its dimensions are bounded by the available window space, and long Cask names wrap. Native over events retain that state without restarting animation or changing target geometry. Leave, drop (including rejected multi-item drops), Escape, focus loss and unmount clear it. The overlay has no pointer hit area and does not shift the layout. Light/dark colors use the existing semantic surface, text and focus tokens.

Drop processing remains the existing one-item Add Items review, with explicit Reference/Managed choice and ownership consent. No resource is registered on hover or drop alone. Scope is individual Cask windows only; main-window Cask rows do not accept drops.

Drag-feedback verification: cargo xtask check passed full Rust/default/Alpha checks, strict Clippy, formatting, frontend lint/types/contracts, 46 GUI tests and builds. Tests cover file/folder hover, stable native over events, leave/Escape/blur, rejected multi-item drops, and unchanged explicit intake consent. Evidence: target/drag-feedback-quality.log.
