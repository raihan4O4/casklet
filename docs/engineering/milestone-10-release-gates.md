# Milestone 10 release gates

Status: local Alpha candidate validation; broad public-distribution gate remains open. Current evidence: [Alpha readiness](alpha-readiness.md).

The packaging spike produced `Casklet_0.0.0_x64-setup.exe` with Tauri's NSIS bundler on this Windows host. The supported installer choice is per-user NSIS. A local-host install exposed a collision between its `%LOCALAPPDATA%\Casklet` program directory and the former database path. The empty test install was removed; release state now uses `%LOCALAPPDATA%\CaskletData`. The corrected package passed the local preservation test below. A clean Windows 11 x64 machine has not been tested.

## Build and traceability

Run `cargo xtask check` and, from a clean Windows 11 x64 checkout with a checksum-verified Syft binary, set `SYFT_PATH` to its absolute executable path and run `cargo xtask package`. This builds the CLI ZIP and per-user NSIS installer and writes `SHA256SUMS.txt`, `provenance.json` (Git revision and both dependency-lock hashes), and an SPDX JSON SBOM. Run `cargo xtask verify-release` again after upload. Release candidate files are not signed by this script; authenticated publisher/signing must be configured before public distribution.

## Automated evidence

- Workspace Rust tests, lint, formatting, CLI and desktop builds: `cargo xtask check`.
- Every milestone schema 1–13 upgrades to current schema with Cask data intact; versions 1–12 produce a readable pre-migration SQLite backup.
- Corrupt database startup rejects the file without overwriting it.
- Frontend smoke tests cover typed commands and explicit resource intake.

## Manual Windows 11 x64 matrix

Record OS build, filesystem, volume layout, Git revision, tester, date, screenshots/logs, and result for each row before changing this document's status.

| Area            | Required observation                                                                                                         | Result                                                                           |
| --------------- | ---------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| Fresh install   | Per-user NSIS installs without elevation; CLI ZIP runs; development data is not opened                                       | Pending                                                                          |
| Upgrade         | Install candidate over a prior published build; existing Casks and managed content remain available                          | Pending                                                                          |
| Cask workflow   | Create, reopen, categorize, reference/adopt, export, recover manifest (Copy/Move user intake temporarily disabled)           | Pending                                                                          |
| App workflow    | Inspect and install portable ZIP, search and associate Windows apps, review WinGet/installer outcome                         | Pending                                                                          |
| Recovery        | Interrupt resource, WinGet, and installer operations; inspect and preserve uncertain outcomes                                | Pending                                                                          |
| File edge cases | Low disk, long path, non-ASCII names, multi-volume move, inaccessible path, reparse point                                    | Pending                                                                          |
| Accessibility   | Keyboard only: create/open Cask, review resource, inspect operation/recovery, export; visible focus and screen reader labels | Pending                                                                          |
| Performance     | Profile cold startup, 1,000-resource inventory, scan, and storage aggregation on supported hardware                          | Partial: inventory and storage profile passed locally                            |
| Uninstall       | Remove Casklet through Windows Settings; confirm `%USERPROFILE%\\Casklets` and `%LOCALAPPDATA%\\CaskletData` remain intact   | Partial: silent local uninstall passed; Settings and clean-machine paths pending |
| Security        | Independent review of removal, elevation, CSP, path identity, providers, dependency policy                                   | Pending                                                                          |

Default packages remain review-only. The explicitly authorized local/limited Alpha profile enables journaled file cleanup under ADR 0021; WinGet uninstall remains disabled. This does not close independent review or certify public distribution. No public release should be labeled ready while any required manual row remains pending or a critical/high defect is open.

## Local host evidence (2026-09-28)

Solo developer host: Windows 11 x64 build 26200.9457, NTFS per-user profile. The corrected NSIS candidate was installed silently without elevation into `%LOCALAPPDATA%\Casklet`. The release CLI created Cask `1de3a4aa-0358-4ae6-9c0e-74a33ca9dce5`; its database existed in `%LOCALAPPDATA%\CaskletData\db`, and its manifest existed in `%USERPROFILE%\Casklets\<id>\.casklet`. Silent uninstall returned zero and removed the program directory. Both data locations remained, and the CLI listed the Cask after uninstall. The test-only data was then removed after exact-path and content checks. This validates the local silent path only; Windows Settings, upgrade, and clean-machine behavior remain pending. The packaging script's final revision and checksum evidence must still be captured for the reviewed candidate.

On this host, the explicitly invoked release-mode 1,000-resource test measured inventory at 60 ms and storage aggregation at 62 ms. Cold startup and scan progress have not been profiled.

The project is licensed under Apache-2.0 (`LICENSE` and `NOTICE`). The package script includes those files and emits dependency-license evidence. Third-party notice and source obligations still need review before public distribution.

## GUI implementation evidence (2026-09-30)

`cargo xtask check` passed: all-feature Rust lint/tests/builds, formatting, frontend lint, Svelte checks, five interaction tests, production frontend build, and generated-contract consistency. PR #14 CI detected yanked transitive `yoke-derive 0.8.3`; the lockfile now uses compatible `0.8.4`. The schema 11 sharing/metadata test verifies deduplicated library totals, shared association presentation, description persistence/projection, archive preservation, and stale-revision rejection. The standard Tauri release build produced the x64 NSIS installer with destructive execution disabled. Its logo source matches the supplied PNG byte-for-byte; Inter and Lucide notices are bundled.

Native visual inspection used isolated development state under `target/gui-qa`, five Casks, and a real copied notes file. Main inventory and an owned secondary workspace rendered with real totals, separate content scrolling, and visible minimize/close controls without a maximize button. Caption controls replace Windows' otherwise visible disabled maximize control. Full keyboard/screen-reader, high-DPI, repeated-window-focus, clean-machine, and installer upgrade/uninstall checks are still pending; this evidence does not close the manual release matrix.

## Performance budget proposal

Cold startup to usable dashboard: 3 seconds on baseline hardware with warm filesystem cache; 1,000-resource inventory and storage summary: 5 seconds; progress reporting for scans: first update within 2 seconds. Measure on a Windows 11 x64 reference machine and revise budgets from profiles before public alpha.

## Alpha candidate follow-up (2026-10-02)

`cargo xtask package-alpha` explicitly selects file cleanup for CLI and desktop. Clean source is required normally. Local validation may use `--allow-dirty`; provenance identifies it as dirty and includes a complete source-file checksum snapshot. The verifier checks that snapshot, lock hashes, artifact inventory, and checksums including provenance. Source changes during packaging invalidate the candidate. The public signing/notice/source and manual matrix gates above remain open; a local package is not a published release.

The 0.1.0-alpha.1 NSIS candidate passed local silent install/reinstall/uninstall in a verified workspace program directory with isolated release data: all exit codes zero, program executable removed, database/Cask reopened, managed sentinel bytes unchanged. The prior per-user install-location setting was restored. The desktop executable reached a visible Casklet window at 989 ms with isolated data. These observations do not close clean-machine, prior-version upgrade, Windows Settings uninstall, visual/accessibility, or signing gates. Candidate packaging verified nine checksum-bound artifacts, with file cleanup enabled and provider uninstall disabled in provenance.
