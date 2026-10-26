# Applications library refinement

## Layout

Applications now uses a pinned toolbar with one prominent Add Application action, live search by app name/publisher/provider, and All/Managed/Associated filters. The add-method picker and selected workflow open above the library, scroll into view, receive keyboard focus, and return focus to Add Application when closed. Smooth navigation respects reduced motion. The add-method picker follows the reference with three stacked rows: Install Application, Associate existing app, and Install portable ZIP. Each row has a centered accent-colored icon tile, title/description, and accent-colored chevron. WinGet remains available under More install options. Shared theme tokens control surfaces, accents, hover feedback, and borders; narrow layouts retain the same hierarchy.

The library uses compact alphabetically sorted rows, concise versions/publishers, provider labels, ownership badges and explicit Open/Cleanup/Remove link/Manage actions where the existing provider supports them. Empty and no-match states explain the next action. Long names are truncated without changing their identities.

`ApplicationLibrary.svelte` owns presentation/search/navigation. `applications.ts` projects typed provider records and deduplicates by stable installation identity. WinGet and Windows metadata for one installation share one row; distinct installations with the same name remain distinct. Missing WinGet registry IDs use distinct provider identities. Presentation badges never grant cleanup authority, and recovered/unverified records are not promoted by this projection. Existing application commands, approval gates and journals remain in the shared core.

## Installer disclosures and recovery

Advanced Installer History and Technical Details are removed from Application Install. Paths, hashes, certificates, candidate evidence, operation IDs and internal error/state codes are not rendered there. Low-level CLI/API inspection and durable journals remain available for troubleshooting.

The GUI still reads installer history internally to restore unfinished installation status after reopening the Cask. A short attention notice opens the installation workflow. Confirming that the installer and its opened apps have finished enables Keep installed changes, which invokes the existing preserved-closure command. It never reruns an installer, deletes software or grants ownership. New installs remain blocked while an unresolved operation exists. Successful installation remains automatic under ADR 0023; arbitrary EXE automatic uninstall remains unsupported.

## Validation

Interaction tests cover a ten-app library, top-level action placement, search/ownership filters, no-match reset, focus movement/restoration, provider deduplication without name-based ownership, removed installer disclosures and recovery without exposing diagnostic data. Visual fixture checks covered the native 560 × 760 window, the four-method picker, installation panel and toolbar while scrolling twelve applications. The fixture is disposable and removed after checking; no real installer or user data was used.

Frontend lint, Svelte checks (zero errors/warnings), all 28 GUI tests and the production build passed. Logs are retained under `target/application-library-*-final.log`. Rust domain/application behavior was not changed in this UI refinement.

Design reference: [Carbon data-table toolbar guidance](https://www.carbondesignsystem.com/building-blocks/core/components/data-table/guidelines).

## October 26 chooser verification

The reference-based chooser passes 12 production-browser combinations (Light/Dark, 390/560px widths, black/purple/white accent preferences), including centered glyphs, resolved accent colors, no horizontal overflow, accessibility checks, and navigation through all four workflows. Isolated native WebView checks cover both themes and opening/closing the installer workflow. All 128 frontend tests, lint, Svelte checks, and production build pass. Evidence: `.jury/application-methods`.

## October 26 reference cards

Applications now uses a bordered rounded container, neutral grid-icon tile, heading/count and primary Add action, full-width search with a segmented ownership filter, and individual application cards. Real application icons are retained with the existing fallback when unavailable; version/publisher/provider, saved size and ownership badge are grouped together. Existing Open/Remove link/Manage/Remove handlers and capability guards remain unchanged. Application cards keep their actions aligned to the right at all supported window widths; narrow layouts wrap metadata safely. Singular totals read 1 app.

The companion Settings cards use matched neutral icon tiles, typography, rounded panels, inset chevrons and an edit icon on Change appearance. Shared Reset changes/Save changes behavior remains. Production screenshots and accessibility/interaction checks cover both sections in Light/Dark at 390/560/1024px; all 128 frontend tests, Svelte checks, lint and build pass. Evidence: `.jury/settings-applications`.

October 26 follow-up keeps Remove link on the right, combines file search Ctrl K into one badge, replaces category hover fill with a softly transitioned shadow, reduces Settings card padding/gaps to 12px vertical/8px between cards, and removes persistent selected Cask fill/elevation. Keyboard focus and actual pointer hover remain. Production theme/width screenshots, search/disclosure/reset checks, explicit selection/hover comparisons, lint, Svelte checks and build pass.
