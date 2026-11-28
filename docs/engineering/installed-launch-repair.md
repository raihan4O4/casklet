# Installed launch repair — October 7, 2026

## Confirmed cause

The reported `release/Casklet_0.0.0_x64-setup.exe` was a September 28 packaging spike, not the current application. Its original SHA-256 was `9bb0c4161b6143e87c4ab57a4d62a4633ce3827b70c276fc3842e1924059b479`.

An isolated install returned zero and rendered the old GUI with empty data. Seeding the same profile using the current CLI (schema 16) and reopening the installed 0.0.0 executable reproduced exit code 101 before a window appeared:

```text
failed to open Casklet state: "unsupported database schema version: 16"
```

A read-only inspection confirmed that the existing user database also has schema 16. The obsolete binary cannot read this newer database; its startup `expect` turns the compatibility rejection into an immediate panic. This is a stale installer/downgrade problem, not a missing WebView2 runtime on this host. Evidence remains under `target/legacy-launch-probe`.

## Repair and prevention

- Rebuild the current 0.1.0-alpha.1 package; keep data in CaskletData and content outside program files.
- Current startup already reports data errors in a native dialog instead of silently panicking. Improve newer-schema guidance and reject compatibility before changing SQLite journal mode; a regression test checks byte-for-byte preservation.
- Disable installer downgrades for newly built packages. This cannot alter the behavior of previously distributed setup binaries.
- `refresh-local-installer.ps1` first verifies package provenance, source and checksums, archives obsolete local setups, and refreshes the canonical root setup and the exact historical setup path. The 0.0.0 filename is only a local compatibility alias; distribute the correctly versioned setup. `release/local-installer.json` records their shared hash and revision.
- The installed smoke check accepts an explicit setup path, checks its actual installed version against source, and records the installer hash. It launches against current-schema data before and after reinstall, then verifies that uninstall preserves both database and managed content.

## Verification scope

Verification passed on this Windows 11 host before opening the PR:

- `cargo xtask check`: 118 all-feature Rust tests (one opt-in benchmark ignored), separate default/Alpha suites, strict Clippy, formatting, generated contracts, 37 GUI tests, ESLint, Svelte checks with zero errors/warnings, and production builds. Log: `target/installed-launch-quality.log`.
- Clean-source Alpha packaging and nine-artifact verification: `target/installed-launch-package.log` and `target/installed-launch-promotion.log`.
- The exact historical setup path passed isolated install/visible launch/reinstall/uninstall. Current-schema database and sentinel content survived; existing unregistered program files were preserved. Log: `target/installed-launch-smoke.log`.
- The installed GUI reopened the same schema-16 profile that crashed 0.0.0, created a Cask through the UI, and rendered its separate native workspace.
- Normal per-user installation launched with the existing Cask. A read-only SQLite backup was taken first, and Cask/resource/application/association metadata and integrity were unchanged after repair. Local evidence: `target/installed-user-data-evidence`.
- Installed application inventory and Settings loaded. Both installer-created Desktop and Start-menu shortcuts launched visible Casklet windows after reinstall. Evidence: `target/installed-launch-shortcuts.json`.
- Release CLI cleanup smoke passed: unconfirmed execution blocked, owned fixtures recycled, external permanent deletion refused, referenced bytes preserved, durable history and metadata backup available. Log: `target/installed-launch-alpha-smoke.log`.

Local Windows evidence does not substitute for an isolated clean machine, independent security review, signing, or arbitrary third-party MSI/EXE/UAC tests. Native secondary-window input is limited by the automation helper; workspace rendering was inspected, while detailed resource/provider behavior was covered by the automated suites and isolated CLI smoke.
