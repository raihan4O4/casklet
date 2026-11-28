# Management details and Settings refinement

October 21, 2026. User-requested scope: the Files & Folders expanded management panel, its dropdown and action controls, and both Cask/global Settings. The existing collection-desk palette, Svelte/Tauri stack and typed core boundary remain. See [DESIGN.md](../../DESIGN.md) for alternatives and the scoped resource-sheet concept.

## Resource editing

The expanded row now reads as a resource sheet: location, measured size/availability/management, Category, Ownership, folder descendants and a separate cleanup preview. Opening a row brings its identity into view and compacts the workspace header. Category and Ownership each open one focused editor; other details step aside until it closes.

Category uses a full-width styled native select with a clear chevron and explanatory text. It keeps native keyboard/platform behavior. Ownership uses labelled radio cards with the consequences visible beside each choice. Opening the editor retains the current policy, rather than preselecting its opposite. Changing choices clears consent; an unchanged reference cannot submit, and owned grants still require the existing dedicated-data acknowledgement. Renewal retains the existing core command and safeguards.

Cancel and Save remain in the workspace's persistent footer while editing and submit their real HTML forms through the `form` attribute. They replace unrelated Add Items/Cleanup actions during the edit. Escape cancels without mutation and restores focus to the initiating control; closing the disclosure returns focus to its row. Commit handlers, IDs, optimistic ownership checks, category commands and cleanup plans remain shared-core behavior.

## Settings

Cask Settings now has distinct identity/archive, category and export sections. Description is a multiline UI-font field. Archive remains reversible and explicitly preserves resources. A persistent Reset/Save footer applies only to name, description and archive changes; unchanged metadata cannot submit or bump revisions. Reset restores the current saved values.

Available categories are passive text labels. Creation is explicitly immediate, guarded against duplicate pending requests, disabled for empty input, and preserves the name after failure. Success/error feedback stays in the category section, preventing its notification from covering export controls. Readable report and portable JSON describe their contents; machine-specific paths require an explicit export disclosure.

Global Settings groups appearance, metadata backup, diagnostics and actual build cleanup capability. Action rows explain their effects and privacy limits. Existing theme persistence and cross-window synchronization remain intact.

## Verification and review

- Existing ownership/category/cleanup regressions remain covered. The suite now contains 76 tests, including category duplicate-submit/failure/retry behavior. Ownership tests exercise explicit policy choice and Escape focus restoration. Intake retention is tested through the Files & Folders tab while the Settings footer serves its own task.
- Production captures cover both themes, resource/category/ownership/folder panels, Cask/global Settings, categories and exports, plus the wider application's regression states. The final run adds 390 px resource editors. All reports check overflow, console/request errors, axe A/AA rules and reduced-motion behavior; editor keyboard cancellation is asserted directly.
- Svelte checks, ESLint, formatting, production build and bundled native debug build are required. The isolated real WebView2 smoke creates a reference, changes category, verifies ownership consent/cancel, saves and resets Settings, closes intake with the keyboard and synchronizes theme across windows. Test state is confined to `target/management-settings-native`; no user installation, ownership grant or cleanup execution is used.

Local evidence: `.jury/management-settings/pass-1`, `pass-2`, `final`, `release-check` and `native`. The browser transport is test-only and never imported into the app. Fixture ownership strings now match the actual OWNED/REFERENCED contract instead of legacy presentation values.

Two fresh independent scoped critics inspected 17 affected screenshots apiece without source, concepts or prior scores. First weighted score: 6.87/10. Second: 7.35/10 (Design 7.5, Usability 7.3, Creativity 6.8, Content 8.0). These are estimates, not affiliated with Awwwards; website award offsets do not apply. Their recommendations drove the persistent action footer, focused resource context, current-policy default, lighter passive categories, UI-font description and inline creation feedback. The last copy/typography/feedback corrections have automated/native evidence, without claiming a new independent score.

Remaining limits: complete screen-reader/forced-colors acceptance, measured transition FPS, actual Windows logical text enlargement and broader native performance are not certified. No backend/schema/provider policy changes or public-release approval are implied. The installed application is not replaced during QA; the local setup is refreshed separately using the previously authorized Alpha packaging workflow described below.

## Final evidence

All 76 GUI tests pass. Svelte reports zero errors/warnings; ESLint, changed-file formatting and production/native debug builds pass. The final `release-check` run contains 83 screenshots, zero axe violations, zero console errors/request failures, no horizontal overflow, and visible/styled sampled focus. The 4× CPU browser fixture recorded LCP 1036 ms and CLS ~0.00085; maximum capture CLS ~0.03255. Reduced-motion samples contain zero running animations. These are fixture metrics, not native startup or installer performance.

The final bundled desktop passed the isolated real-core smoke again: reference registration, category save, ownership consent/cancel, metadata save/reset, keyboard intake close, cross-window theme synchronization and ordinary shutdown. No real user data or destructive execution was used.

The earlier user-authorized setup is refreshed with these refinements using the existing explicit Alpha pipeline and dirty-source provenance. Evidence: `target/management-settings-package.log` and `target/management-settings-package-verification.log`; output remains `release/alpha`. This does not install or publish the package, change its capability profile, or close existing release gates.

## Location and association follow-up

The resource sheet replaces the raw Windows path with **Open location**. Its typed `GetResource` request carries only the stable resource ID. The application core resolves the registered path through `ResourceRepository`, and the existing storage-browser port opens the directory itself for folders or the containing directory for files. The Windows adapter verifies existence, normalizes verbatim drive/UNC prefixes for Explorer and invokes the system Explorer executable with a separate directory argument. It never executes the resource or accepts a frontend-supplied path. Missing locations return a visible error. The cleanup action's trash icon now uses the semantic danger token in both themes, overriding the generic muted resource-icon rule.

Application association/disassociation now releases the picker after the relationship refresh, independently of potentially slow storage-size measurement. Measurement continues in the background with Cask/revision guards and explicit failure feedback. A link no longer presents as an active search or hides the result list. Duplicate pending submissions are guarded; inventory refresh retains one busy lifetime through its subsequent query. The regression test holds size measurement unresolved and verifies that Search and Refresh work while the linked result remains correctly disabled as Associated.

Validation: 84 GUI tests, strict production Rust Clippy, native/frontend builds, frontend lint/type/format checks, a native registered-resource error-boundary test and Light/Dark rendered resource checks. Local browser evidence is under `.jury/details-refinement`; no ownership or cleanup rules change.

## October 22 resource overlay

Selecting a Files & Folders item now opens its management sheet in a native modal dialog instead of expanding the list. The unchanged inventory remains behind a dimmed backdrop; the modal has a resource-name heading, scrollable body and persistent in-dialog category/ownership Cancel/Save actions. Native dialog behavior traps focus and makes the background inert. Escape cancels the active editor first, then closes the sheet; Close returns focus to the original item. Pending policy/category saves cannot be dismissed. Read-only details can close during unrelated background work. Cleanup closes the sheet before navigating to the existing plan workflow.

Location, size/status, category, ownership, folder inspection and cleanup retain their existing typed commands and safety gates. Success/error feedback appears inside the overlay rather than behind its backdrop. Both appearance modes, narrow reflow and reduced-motion preferences use the existing tokens. The presentation change does not add automatic filesystem scans or alter cleanup authority.

Verification covers modal/editor containment, cancellation and focus return, existing mutation regressions, Light/Dark and narrow production captures, and native WebView focus trapping/editing. Local evidence: `.jury/resource-overlay`.

The final overlay regression suite contains 88 passing GUI tests, with zero Svelte errors/warnings and passing ESLint, formatting and frontend/native builds. Native WebView2 verifies `:modal`, 14 forward and 14 reverse Tab steps, real category save, ownership consent/cancel and ordinary shutdown using `target/resource-overlay-native-v2`. Explicit Tab wrapping prevents WebView focus from reaching window chrome at the modal's final control. Browser QA repeats focus wrapping, Escape restoration and the existing full route/state audit; captures are under `.jury/resource-overlay/verified`.

Cask Settings now uses native collapsible sections for Cask settings, Categories and Export Cask. All three sections now start closed whenever Settings opens (October 24 refinement; ADR 0040). Keyboard-operable headers retain their descriptions and show chevrons, while form drafts, immediate category creation, export disclosures and the existing metadata Save/Reset footer retain their behavior. A focused Light/Dark audit covers collapsed/open sections, keyboard toggles, category creation and 390 px reflow (`.jury/collapsible-settings`).
