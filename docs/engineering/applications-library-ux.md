# Applications library refinement

## Layout

Applications now uses a pinned toolbar with one prominent Add Application action, live search by app name/publisher/provider, and All/Managed/Associated filters. The add-method picker and selected workflow open above the library, scroll into view, receive keyboard focus, and return focus to Add Application when closed. Smooth navigation respects reduced motion. All four methods fit a two-column picker at the actual 560 × 760 Cask window size; narrower layouts stack the cards.

The library uses compact alphabetically sorted rows, concise versions/publishers, provider labels, ownership badges and explicit Open/Cleanup/Remove link/Manage actions where the existing provider supports them. Empty and no-match states explain the next action. Long names are truncated without changing their identities.

`ApplicationLibrary.svelte` owns presentation/search/navigation. `applications.ts` projects typed provider records and deduplicates by stable installation identity. WinGet and Windows metadata for one installation share one row; distinct installations with the same name remain distinct. Missing WinGet registry IDs use distinct provider identities. Presentation badges never grant cleanup authority, and recovered/unverified records are not promoted by this projection. Existing application commands, approval gates and journals remain in the shared core.

## Installer disclosures and recovery

Advanced Installer History and Technical Details are removed from Application Install. Paths, hashes, certificates, candidate evidence, operation IDs and internal error/state codes are not rendered there. Low-level CLI/API inspection and durable journals remain available for troubleshooting.

The GUI still reads installer history internally to restore unfinished installation status after reopening the Cask. A short attention notice opens the installation workflow. Confirming that the installer and its opened apps have finished enables Keep installed changes, which invokes the existing preserved-closure command. It never reruns an installer, deletes software or grants ownership. New installs remain blocked while an unresolved operation exists. Successful installation remains automatic under ADR 0023; arbitrary EXE automatic uninstall remains unsupported.

## Validation

Interaction tests cover a ten-app library, top-level action placement, search/ownership filters, no-match reset, focus movement/restoration, provider deduplication without name-based ownership, removed installer disclosures and recovery without exposing diagnostic data. Visual fixture checks covered the native 560 × 760 window, the four-method picker, installation panel and toolbar while scrolling twelve applications. The fixture is disposable and removed after checking; no real installer or user data was used.

Frontend lint, Svelte checks (zero errors/warnings), all 28 GUI tests and the production build passed. Logs are retained under `target/application-library-*-final.log`. Rust domain/application behavior was not changed in this UI refinement.

Design reference: [Carbon data-table toolbar guidance](https://www.carbondesignsystem.com/building-blocks/core/components/data-table/guidelines).
