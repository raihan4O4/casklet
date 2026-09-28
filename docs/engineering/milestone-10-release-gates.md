# Milestone 10 release gates

Status: implementation in review; public alpha gate remains closed.

The packaging spike produced `Casklet_0.0.0_x64-setup.exe` with Tauri's NSIS bundler on this Windows host. The supported installer choice is per-user NSIS. The installer has not yet been run or uninstalled on a clean Windows 11 x64 machine.

## Build and traceability

Run `cargo xtask check` and, from a clean Windows 11 x64 checkout with a checksum-verified Syft binary, set `SYFT_PATH` to its absolute executable path and run `cargo xtask package`. This builds the CLI ZIP and per-user NSIS installer and writes `SHA256SUMS.txt`, `provenance.json` (Git revision and both dependency-lock hashes), and an SPDX JSON SBOM. Run `cargo xtask verify-release` again after upload. Release candidate files are not signed by this script; authenticated publisher/signing must be configured before public distribution.

## Automated evidence

- Workspace Rust tests, lint, formatting, CLI and desktop builds: `cargo xtask check`.
- Every milestone schema 1–10 upgrades to current schema with Cask data intact; versions 1–9 produce a readable pre-migration SQLite backup.
- Corrupt database startup rejects the file without overwriting it.
- Frontend smoke tests cover typed commands and explicit resource intake.

## Manual Windows 11 x64 matrix

Record OS build, filesystem, volume layout, Git revision, tester, date, screenshots/logs, and result for each row before changing this document's status.

| Area            | Required observation                                                                                                         | Result  |
| --------------- | ---------------------------------------------------------------------------------------------------------------------------- | ------- |
| Fresh install   | Per-user NSIS installs without elevation; CLI ZIP runs; development data is not opened                                       | Pending |
| Upgrade         | Install candidate over a prior published build; existing Casks and managed content remain available                          | Pending |
| Cask workflow   | Create, reopen, categorize, copy, move, reference, export, recover manifest                                                  | Pending |
| App workflow    | Inspect and install portable ZIP, search and associate Windows apps, review WinGet/installer outcome                         | Pending |
| Recovery        | Interrupt resource, WinGet, and installer operations; inspect and preserve uncertain outcomes                                | Pending |
| File edge cases | Low disk, long path, non-ASCII names, multi-volume move, inaccessible path, reparse point                                    | Pending |
| Accessibility   | Keyboard only: create/open Cask, review resource, inspect operation/recovery, export; visible focus and screen reader labels | Pending |
| Performance     | Profile cold startup, 1,000-resource inventory, scan, and storage aggregation on supported hardware                          | Pending |
| Uninstall       | Remove Casklet through Windows Settings; confirm `%USERPROFILE%\\Casklets` and `%LOCALAPPDATA%\\Casklet` remain intact       | Pending |
| Security        | Independent review of removal, elevation, CSP, path identity, providers, dependency policy                                   | Pending |

Destructive cleanup and WinGet uninstall stay disabled in distributable builds until the independent security review approves them. No public release should be labeled ready while any required manual row remains pending or a critical/high defect is open.

The project's own license is still undecided in `LICENSES/README.md`. The package script emits dependency-license evidence, but a public release also needs a chosen project license and review of third-party notice/source obligations.

## Performance budget proposal

Cold startup to usable dashboard: 3 seconds on baseline hardware with warm filesystem cache; 1,000-resource inventory and storage summary: 5 seconds; progress reporting for scans: first update within 2 seconds. Measure on a Windows 11 x64 reference machine and revise budgets from profiles before public alpha.
