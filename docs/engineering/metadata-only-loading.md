# Lightweight Cask loading

## Root causes and boundaries

The expensive work followed three paths:

1. `cask_library` → `list_resources` → `refresh_availability` → directory measurement and persisted updates, plus `add_application_storage` → `application_sizes` → installation-folder inspection.
2. Workspace initialization awaited `refreshResources`, including the same live resource and storage queries. Focus and three-second `syncFiles` polling repeated them and refreshed expanded folder contents.
3. Add Resource in Reference mode called the general deep inspection adapter. Windows identity evidence could read full file contents or hash directory descendants, even though a reference grants no deletion authority.

ADR 0037 separates saved presentation queries from explicit inspection. Both native GUI and CLI retain the same Rust core. No frontend filesystem access, new services, third-party dependencies, cloud state or domain ownership shortcuts are introduced.

## User behavior

Library and Overview display last-known logical sizes. References contribute to inventory size, not physical Cask metadata size or reclaimable ownership. Opening/focusing a Cask or polling saved changes performs no content scan. New directory references show unknown size until measured; individual files use filesystem length metadata without reading bytes. Available Windows registry estimates are saved when inventory/association is explicitly updated.

**Refresh sizes** in Overview runs an explicit live measurement without disabling navigation. **Refresh folder contents** updates an expanded folder inspection. Measurements can be slow for large or inaccessible trees; this work remains outside loading and automatic polling. The CLI's existing live inspection and destructive safety checks retain their behavior. Saved sizes are never cleanup evidence.

## Verification

- A filesystem-free integration adapter panics if library/resource/summary loading touches content or writes projections. The test passes after reopening SQLite with an offline folder path and saved 1 TiB folder plus 1 TiB application sizes; measured combined query time was about 22–29 ms locally. No large data allocation is needed for this structural proof.
- The same test validates registration-evidence invalidation and rejects a stale measurement write. Schema upgrade tests cover every prior schema through 18 with the existing consistent backup process; schema 19 is created on fresh databases.
- The complete all-feature infrastructure suite passed: 133 tests, one opt-in test ignored. This includes resource ownership, replaced-file/directory preservation, Recycle Bin restore, approval/execution/recovery and installer/provider behavior. Evidence: `target/metadata-loading-tests-final.log`.
- Frontend regressions cover explicit measurement while navigation stays usable, polling without live refresh, folder inspection only on request, and existing application-link behavior. Strict Rust Clippy, Svelte checks, ESLint, formatting and production/native debug builds are required.
- A real debug desktop using isolated metadata fixtures measured five library IPC queries at 16.9–19.4 ms and opening a new independent Cask workspace at 411 ms. The fixture contains a saved offline 1 TiB directory and 1 TiB application; these are logical snapshot values, not actual allocated terabytes. No user installation/data or cleanup execution was involved. Evidence: `.jury/metadata-loading/native/result.json` and screenshots.

Production browser checks use the existing test-only IPC fixture, separate from native/core evidence. Host filesystem speed, database size, number of associations and native-window creation can still affect total latency; timing samples are not a universal startup guarantee or a benchmark of actual terabyte tree enumeration.
