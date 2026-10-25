# Resource atlas desktop redesign

Implemented 2026-10-24. Design system: ../../DESIGN.md. Architectural decision: ../adr/0041-resource-atlas-desktop-shell.md.

## Product and implementation

The existing Svelte 5 / TypeScript / Vite / Tauri 2 application now uses a shared NavigationRail for the main library and independent Cask windows. Main destinations remain Casks, Installations, Activity and Settings. Cask destinations remain Overview, Files & Folders, Applications and Settings. Cleanup is a deliberate task reached through the persistent action footer, with its context retained in the header while content scrolls.

The library separates activity identity, known logical size, search/sort/view controls and actual collections. Spine-shaped collection marks derive identity colors from stable Cask IDs. A compact statistic band and shorter cards expose four collections at the ordinary desktop height. List view supports dense scanning. Empty, search-empty, loading and retained-error retry states use the same system.

Cask windows use a two-column grid with fixed navigation and separately sized identity, content and action regions. Inventory, intake, application methods, global/Cask settings, resource editors, creation/removal/Explorer dialogs, menus, progress feedback and recovery inherit semantic shared roles. Light uses neutral surfaces and cobalt actions; Dark uses layered charcoal and pale cobalt. Existing arbitrary custom accent fills retain independently tested readable ink and focus. ThemeSettings displays the new defaults correctly in both HEX and native color controls.

Navigation includes visible active state, named destinations, Alt+1 through Alt+4 tooltips/aria-keyshortcuts, and skip links for both window types. Modal dialogs suspend shortcuts. Menus and management editors retain Escape and origin-focus restoration. At 560px native workspace width the rail is 76px; narrower logical previews reduce it to 68px without removing navigation labels.

Cleanup Review shows noninteractive Review / Approve / Execute stages. Only the current transition action is present. Review-only builds expose no approval or execution button; capable builds reveal approval only after review, and execution only after approval with the existing confirmation guard. Expired plans expose no transition controls. Completed stages use durable status and local exact-hash acknowledgement. Plan-ready feedback is inline instead of obscuring reviewed items. Resource details keep the cleanup row visible while long folder contents scroll.

All existing application methods remain connected: installer MSI/EXE/elevation/recovery, existing-app association, portable ZIP, WinGet, launch, detach and reviewed application cleanup. File intake retains batch consent, partial results and retry. Storage remains known logical size, including references, and never promises reclaimability. Opening and navigation remain metadata-only.

No schema, generated contract, Rust policy, native window lifecycle or capability-profile changes. JavaScript does not acquire filesystem or deletion authority. No new production dependencies. No user data or installed applications were used for QA.

## Verification

- Frontend: 105 tests across seven suites pass. New tests verify navigation callback parity, shortcut range/modifiers and suppression behind modal dialogs. Existing destructive-flow regressions now assert absent unavailable transitions, exact-plan review before approval and explicit confirmation before execution. One saved-plan test required awaiting the newly visible review action after asynchronous loading; its command-target assertions remain intact.
- Svelte: zero errors/warnings. ESLint: passes. Production Vite build: passes. Changed-file Prettier and git whitespace checks: pass.
- Rust regression: `cargo test --workspace --all-features --offline` passes, including CLI and isolated Windows fixture suites. The opt-in benchmark and installed Explorer fixture remain ignored as designed.
- Native desktop: `cargo build -p casklet-desktop --offline` passes. The filesystem does not support incremental hard links; Cargo falls back to copying, with warnings only.
- Production browser: four full 94-state passes, with the final pass recording zero axe violations, console errors, failed requests or horizontal overflow. Pass 1 caught one 4.47:1 intake contrast failure; the muted role was darkened and subsequent passes are clear. Coverage includes both themes, library card/list/sort/search, creation, all application-add methods, resource details/category/ownership, folder content, settings, cleanup review and confirmation, recovery, narrow logical windows, long text, 125% scale, keyboard restoration and reduced motion.
- Native WebView smoke: two isolated profiles verify real SQLite Cask creation, independent Cask window, real reference registration, resource modal/Escape, Light/Dark synchronization in both directions, real reviewed preservation plan, unavailable default-profile approval and unchanged reference bytes. Windows closed normally through their own controls. Production assets were served to the debug host; this is not an installed-package test.

Evidence is under `.jury/resource-atlas/pass-1`, `pass-2`, `final`, `verified`, `native` and `native-final`. Browser data uses test-only contract-shaped IPC fixtures, never a production mock transport. Native fixture profiles remain under ignored `target/resource-atlas-native*` for inspection.

## Independent review and limits

Separate critics reviewed screenshots without code or design notes. Pass 1 scored 7.05/10, pass 2 scored 7.73/10 and the final independent review scored 7.76/10 (Design 7.8, Usability 8.0, Creativity 7.2, Content 8.0). The aspirational all-categories-at-8 craft target is not met; remaining review suggestions concern density, verification-copy simplicity and asset efficiency. These are desktop presentation estimates, not affiliated with Awwwards; public-web award offsets do not apply. Their findings drove density, duplicate-heading, toast, resource action and cleanup progression corrections. Scores are not release approval.

Existing manual gates remain open: real third-party installer/uninstaller and UAC behavior, clean-machine/upgrade/installed package validation, full screen-reader/high-DPI acceptance and independent security review. No new installer was built or installed, and no real application uninstall or user-data cleanup was executed. Large original logo/font assets remain bundled; optimization is a future efficiency improvement. No full motion/FPS claim is made from still screenshots. The application is ready for continued release-readiness review, not declared publicly release-ready by this work.
