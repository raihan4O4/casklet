# Main window improvements and Cask identity

Implemented 2026-10-25 under the focused user request. Decision: ../adr/0043-library-identity-and-association.md. This extends the current collection-workbench design rather than replacing it.

## Changes

- A restrained dashboard panel gives Cask count and known logical storage large tabular numerals, a larger New Cask action and readable secondary item/reference notes. Both themes use existing surface, type, border and action roles. Responsive layouts preserve the primary metrics without relying on fixed widths.
- The Your library label is removed. Search begins on the content alignment line; view/sort controls and existing filtering remain.
- Installation rows and Cask application lists use consistent 16px horizontal insets within bordered list boundaries. Narrow installation actions wrap to their own row.
- Global installations use Associate instead of Present. The modal offers searchable active Casks, disabled existing/unverified links, explicit confirmation, retained errors/retry, and Create a Cask when none are eligible. The existing versioned list/association commands remain the only backend path. The selected installation ID is preserved even for equal names/registry views. Filtering clears hidden selection. Success updates the row, emits library invalidation and refreshes open target Cask applications. No ownership or installer behavior changes.
- Main/workspace navigation hover uses a subtle surface change with no inset bottom shadow. The separate active-section marker and visible keyboard focus remain.
- Automatic colors (Option A) use a balanced 12-color palette. Saved stable-ID assignments survive restart and synchronize across windows. Whole icon tiles carry the color; glyph/text contrast uses the current theme. Existing Casks are assigned defaults, and palette exhaustion reuses least-used colors. Colors are local appearance preferences, excluded from database backup and portable manifests; lifecycle metadata stays in SQLite. Invalid/blocked storage has safe defaults/session behavior and a persistence warning.

The initial dirty worktree and unrelated .agents/brag-output/skills files are preserved. No new runtime dependency, database schema, generated contract or Rust policy changes.

## Validation

Frontend regression tests cover association command arguments, active/archived eligibility, existing links, unavailable destinations/retry, duplicate clicks, retained failures, hidden-selection reset, empty/all-linked lists and the integrated main-row/focus update. Color tests cover palette variation, stable saved mappings, restart reads, reuse balance, legacy Casks, corrupt preferences and blocked storage. Existing domain/UI/cleanup tests remain part of the full frontend suite.

The production browser audit covers both themes, populated/empty libraries, search/sort, card/list layouts, secondary workflows, narrow windows, long text, reduced motion and keyboard/modal behavior. Added captures exercise destination selection, filtered confirmation, updated links, duplicate prevention, empty destinations and narrow installation/chooser layouts. The test-only transport now rejects an unversioned ListCasks call; the native smoke test caught this mismatch in the first implementation and it was corrected before completion.

Existing Rust windows_inventory regressions run against the unchanged core. Sandbox access initially denied their native registry fixtures; a rerun with fixture access passed all six tests. Existing hard-link fallback and an unrelated unused-import warning are retained.

Final frontend checks pass: 115 tests across nine suites, Svelte with zero errors/warnings, ESLint, production build and changed-file formatting/whitespace checks. The strict full production audit captures 107 states with zero axe violations, console errors, failed requests or horizontal overflow. Four additional final theme/viewport cases (eight captures) verify dashboard/chooser presentation, color persistence after reload, absence of the hover shadow and hidden-selection reset. Evidence: `.jury/main-window-improvements/final`, `scoped-final` and `native`. Browser fixtures cannot establish native correctness. The native check uses an isolated development database and a uniquely named disposable registry metadata fixture, never a real installer/uninstaller.

## Release limits

Hosted CI, a refreshed installer, clean-machine packaging, physical Explorer/picker interaction and full screen-reader testing are not claimed. Existing independent security and public-release gates remain. Appearance preferences are per local desktop profile and are not a cloud or portable customization system.

## Native acceptance evidence

The isolated Windows WebView test passes real creation of two Casks with distinct saved colors, real registry-fixture association through the new modal, disabled duplicate destination, an updated open Cask application list, cross-window Light/Dark behavior, persistence of both colors after process restart, exactly one durable reference and normal last-window process exit. Backend query confirms `DETACH_ONLY`. The disposable HKCU registration is removed by the fixture cleanup; no installer/uninstaller or actual application executable is run. `native/result.json` records the checks and isolated data root. Initial harness fixture-path/registry-view/startup-navigation issues were corrected; the native test also caught the missing versioned ListCasks query in product source, now fixed and covered by stricter mocks.

## Library viewport correction

The follow-up layout request keeps the visual system and card design while reclaiming vertical space. The original listing already had an internal overflow container; its viewport was constrained by the header's stacked New Cask row, nested panel/statistics padding and separate spacing blocks. It was not intentionally limited to half the window by a percentage height.

MainWindow now marks the Casks page with `library-page`. Its grid uses intrinsic header and toolbar rows followed by `minmax(0, 1fr)` for the collection section. The collection section likewise reserves its sorting row and gives the remainder to the existing scroll owner, with zero automatic minimums and contained overscroll. Other main destinations retain their existing layout. No fixed library height, viewport-height subtraction, absolute positioning or card-height changes are introduced.

The header places title and New Cask on one grid row and description below, reduces repeated vertical padding and lets secondary measurement notes share space beside totals when they fit. The summary remains side by side with the identity at ordinary widths and stacks naturally on narrow windows. Fonts, colors, card styling, appearance preferences, search/sort and list controls remain.

Production evidence at 1024x720 shows a 337px populated listing viewport versus the previous approximately 267px, with all four ordinary cards fully visible in two rows. `scripts/library-layout-audit.mjs` asserts the underlying geometry rather than comparing CSS strings. It covers 60 states: both themes; 1024x720, 1024x600, 1440x900, 768x720 and 390x844; 0/4/40 Casks; grid/list. It verifies control separation, full remaining-space allocation, bounded outer content, internal scrolling with a stationary header, accessibility, final-card reachability and an exact 200px viewport gain when a 1024x720 window grows to 1024x920. All cases pass without axe violations, horizontal overflow or console/request failures. Evidence: `.jury/library-layout/verified/report.json` and captures.

The production build, 121 frontend tests, Svelte check (zero errors/warnings), ESLint, changed-file formatting and whitespace checks pass. The test-only fixture adds an optional library count for repeatable large/empty collections; it remains excluded from production. No native lifecycle, data, provider, ownership or cleanup changes, and no installed-package or hosted-CI claim.

The subsequent placement request moves New Cask directly beside Search Casks, inside one responsive toolbar group with aligned 40px controls. The header now contains identity/description and summary only. View controls remain on the toolbar right, and narrow layouts shrink the search field while retaining the adjacent action. The geometry audit also asserts search/button adjacency, vertical alignment and equal height.

The October 26 Associate-button refinement uses the bundled diagonal Lucide Link icon from the supplied reference and a 650-weight label, scoped to global Installations association controls. Other link glyphs and association behavior remain unchanged. Production build and rendered browser inspection pass.

## October 26 Settings and installation search

Installations shares the Casks search geometry (38px height, 10px radius, inset Ctrl K badge, icon and type sizing). Ctrl+K focuses its search outside dialogs; explicit Search and Refresh remain available beside the field. The main Settings surface now uses a library-style intro panel, consistent rounded cards, neutral icon tiles, a contained theme selection row and balanced accent/local-data spacing. Responsive layout preserves all controls. Accent Reset to default is removed as requested; presets/custom HEX still save immediately and persist across reloads.

Production Light/Dark checks at 390/1024px compare search geometry directly, verify shortcut/search and saved accent persistence, confirm reset is absent and check accessibility/overflow. Screenshots: `.jury/main-settings-search`. All 128 frontend tests, Svelte checks, lint and production build pass. The accent UI test now expects the requested absence of reset; underlying preference synchronization/reset support is unchanged.
