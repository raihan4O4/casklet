# Collection desk UI

Implemented October 21, 2026. This supersedes the earlier collected-workspace presentation. Concept alternatives, audit, composition and reusable visual rules are in [DESIGN.md](../../DESIGN.md).

## Product and implementation

Casklet remains a local Windows lifecycle manager, with independent native Cask windows and typed Rust IPC. The redesign uses the existing Svelte 5, TypeScript, Vite, Tauri, bundled Inter and Lucide. No application dependency, schema, provider, ownership, cleanup authority or native window lifecycle changes are introduced. Supplied logos remain unchanged. No production content is mocked.

| Area              | Result                                                                                                                                                                                                                                                                                                              |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Library           | Activity collection cards with stable identity marks, descriptions, item counts, known bytes and update dates. A compact list view retains column sorting. Search has an inline clear action. The total is named Known Cask size and explicitly includes references.                                                |
| Workspace         | Wrapping titles, segmented section navigation, legible measured storage and direct Files/Applications shortcuts. Real Created/Updated dates fit the ordinary Overview. Long workspaces continue to scroll.                                                                                                          |
| Resource intake   | Purposeful file/folder choices, focused source review, consequence-led ownership/reference radios and unchanged dedicated-data consent. Active tasks compact the header and defer unrelated Cleanup navigation. Closing intake restores the inventory.                                                              |
| Files             | Consistent resource marks, management/shared relationships, expanded paths and factual metadata, category and ownership forms. Existing policy changes and folder renewal use the same core commands.                                                                                                               |
| Applications      | Consistent Owned/References filters, searchable lists, aligned badges and action hierarchy. All four installation/association methods remain functional. Unrelated footer Cleanup is deferred while a method workflow is open.                                                                                      |
| Cleanup           | Clear method/scope panels, compact effect summaries, explicit current-gate instructions and state-specific action emphasis. New plans reveal the review heading. Approved confirmation precedes execution controls. Existing exact hashes, expiry, approval, concurrency and execution guards remain authoritative. |
| Activity/settings | Consistent chronology and metadata, readable backup/privacy guidance, accessible theme preview controls, and a compact category disclosure. Existing diagnostics, recovery, archive and export remain available.                                                                                                    |
| Shared states     | Petrol/neutral Light and charcoal/pale-petrol Dark tokens; consistent control/focus/disabled states; brief settle transitions; zero motion under reduced-motion preferences. Notifications sit above the workspace footer or at the main window's lower right, clear of titles.                                     |

The 1024 × 720 main and 560 × 760 Cask windows remain native desktop targets. Browser previews additionally reflow at 768/390 px; this does not establish a mobile release. The final reduction removes competing Cleanup navigation from active intake rather than adding visual decoration.

## Verification

The existing 73 GUI regressions remain covered; two additional regressions verify Overview navigation without mutation and stable-ID opening after layout/search changes. The intake regression now verifies Cleanup is deferred until intake closes. The contrast regression includes the actual workspace Dark overrides instead of checking only the older global palette.

- All 75 GUI tests pass. Svelte reports zero errors/warnings. ESLint, changed-file formatting and the production frontend build pass.
- Production audit captures 71 states across both themes: cards/list, sort/search, creation/errors/empty states, Overview, menus, resource inspectors, ownership/reference consent, all four application methods, Settings, cleanup planning/review and capable-build confirmation. The audit checks dialog/menu Escape focus restoration, keyboard intake close, Overview navigation, theme synchronization in the fixture, review-only capability gating and explicit execution confirmation.
- Final browser results have no axe WCAG A/AA violations, horizontal-overflow findings, console errors or request failures. All 18 sampled library focus stops are visible and styled. Both reduced-motion samples report zero running animations.
- The Tauri desktop builds both through Cargo and as a bundled debug executable with production frontend assets. An isolated real WebView2 run creates a Cask through the shared core, opens its independent window, follows Overview navigation, opens/closes intake with keyboard, synchronizes Dark across windows and closes normally. Its database is under `target/collection-desk-native`; installed Cask data is not used.

Evidence is local and ignored: `.jury/collection-desk/pass-1`, `pass-2`, `final`, `release-check` and `native`. `scripts/ui-audit.mjs` and `scripts/ui-fixture.mjs` reproduce browser evidence; the latter is never imported into production. The workspace filename now records its actual capped width (560), rather than the main-preview loop's 768 label.

## Independent review and limits

Three fresh critics reviewed screenshots and measurements without source, design notes or prior scores. Their weighted scores were 7.25, 7.16 and 7.35/10. The final independent score is an estimate, not affiliated with Awwwards, and remains below the skill's aspirational 8. No website award-band offset applies to this native utility. The last two presentation refinements move notifications away from titles and defer unrelated Cleanup navigation during intake; subsequent automated/native checks validate them without claiming a newly independent score.

Remaining recommendations concern denser self-contained cleanup context, repeated application-method headings and broader assistive-technology/motion evidence. No unsupported benchmark, native startup speed, screen-reader certification, Windows High Contrast acceptance, logical 125% text scaling or public-release readiness is claimed. Browser fixture performance is not installer/provider/filesystem performance. Real third-party installation/uninstall/UAC and existing independent security/clean-machine release gates remain separate.

The application assets retain the supplied full-resolution logos and offline Inter. This favors exact brand continuity; a separately reviewed asset optimization could reduce the roughly 1.47 MB fixture load. There are no production placeholder facts or imagery to replace.

## Run

With the repository-pinned prerequisites/dependencies installed:

```powershell
corepack pnpm --filter @casklet/desktop tauri dev
corepack pnpm build
node scripts/ui-audit.mjs --tools target/ui-audit-tools --out .jury/collection-desk/recheck
```

The browser helper needs Playwright and axe-core in the supplied isolated tool directory. In restricted environments, use the installed `.cmd` tools directly for frontend checks and point TEMP/TMP at a writable ignored `target/tmp` for Vitest. The pnpm launcher initially attempted an unavailable registry download; no lockfile or application dependency was changed to work around it.

The subsequent user-requested setup uses the existing explicit Alpha NSIS packaging pipeline with `--allow-dirty`, preserving the functional Alpha capability profile and recording this uncommitted source snapshot. Packaging and nine-artifact verification evidence use `target/collection-desk-package.log` and `target/collection-desk-package-verification.log`; output belongs in `release/alpha`. This builds a local unsigned setup, without installing it or publishing a release.

## Slow installed-library startup correction

A five-second polling race could discard every successful library result when a real resource/application scan took longer than the polling interval. Each poll incremented the request sequence while earlier work was still pending; the initial completion cleared Loading with no accepted library and no backend error, leaving a silent Could not load screen. Reinstallation retained the affected data and the same timing condition.

Main-window refreshes now allow only one request in flight. Polls and repeated refresh requests retain pending work instead of invalidating it; the existing removal-event sequence guard still prevents removed Casks from being resurrected by a stale response. A genuine load error stays inline beside Retry, which displays Loading while retrying and clears the error after success. The main window reveals its rendered loading state through the existing font/image/layout/native-theme readiness gate without waiting for the library scan. No database, resource, ownership or cleanup behavior changes.

Regression tests hold the first request pending across repeated poll ticks and verify that its result is accepted, that startup readiness is signaled while loading, and that errors remain visible and retry succeeds. The production browser reproduction holds each library request for 16 seconds (more than three polling intervals) in both themes. Evidence: `.jury/library-loading-fix`. A dedicated local Alpha setup is written to `release/library-loading-fix` using the existing package/verification scripts and dirty-source provenance; this does not replace or reset installed data automatically.
