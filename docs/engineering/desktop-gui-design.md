# Desktop GUI design

Implemented 2026-09-30. See ADR 0018 for window and application decisions.

The main window uses a narrow navigation rail, page header with search and creation, three compact summaries, and a sortable inventory list. A row opens its dedicated native workspace without replacing inventory. Menus provide secondary actions. Global Installations, Activity, and Settings expose discovery, operation/recovery status, database backup, and diagnostics.

Each Cask workspace has a compact title/description, five tabs, an independently scrolling content area, and persistent Add Items / Create Cleanup Plan actions. Overview summarizes measured logical storage, counts, location, and dates. Files & Folders identifies managed, referenced, and shared associations, with locations and measurement status. Applications separates portable installations, registered associations, WinGet lifecycle, and reviewed installer artifacts. Advanced workflows are revealed deliberately through application-method choices. Activity queries durable state; Settings edits metadata, archives without deleting content, manages categories, and exports.

Resource intake explicitly offers file, folder, and reference selection and requires copy/move/reference review. Drops never imply a move. Cleanup shows the application's plan and warnings before approval; execution remains disabled in distributable builds pending independent review.

The visual system uses local Inter, charcoal controls, white/light gray surfaces, one-pixel neutral borders, 6–8 px radii, and Lucide outline icons. Green/yellow/red are reserved for state. Paths and names truncate with full values available through native browser tooltips. There are no browser responsive breakpoints, fabricated capacities, decorative charts, or permanent right-side inspector.

## Interaction polish

Cask identity accents are assigned deterministically from stable IDs using a restrained palette; they are not health or ownership indicators. Lifecycle state remains separately labeled. Only the active sort column shows its direction, and repeated clicks toggle it. One action menu is active per window, dismissing on outside pointer/focus, Escape, or selection. Keyboard arrow navigation and reduced motion are supported.

Notifications float at the top without affecting layout and expire automatically. Keyed progress updates replace prior notices; durable operation queries remain authoritative. Activity uses chronological date groups and subdued metadata. Resource intake separates source, explicit copy/move/reference choice, category, explanation, and commit action. The existing application picker separates search, results, installation metadata, and association.

Both native windows start hidden with explicit white native and document backgrounds. After initial queries, Svelte rendering, font readiness, and synchronous layout, the invoking WebView requests its own native reveal/focus. Hidden windows do not depend on potentially throttled animation frames. Existing initialized workspaces are focused rather than recreated.

## Polish verification (2026-09-30)

`cargo xtask check` passed Rust formatting, Clippy, workspace tests/build, frontend formatting, ESLint, Svelte diagnostics, UI tests, and the frontend build. Final frontend checks include nine tests covering existing safety workflows plus sort direction, menu replacement/outside dismissal/Escape focus, keyed expiring notifications, window readiness, and stable varied identity accents. The Windows NSIS bundle builds with normal release feature gates.

Native smoke checks used isolated QA storage: inventory, date-grouped Activity, menu placement/outside dismissal, and separate initialized detail-window appearance. Frame-by-frame flicker measurement, clean-machine installation, high-DPI, and full assistive-technology release checks remain manual release gates.
