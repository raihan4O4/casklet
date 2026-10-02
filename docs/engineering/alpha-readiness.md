# Alpha readiness — 0.1.0-alpha.1

Date: 2026-10-02. Target: Windows 11 x64. Status: local/limited Alpha candidate; public-distribution clearance is not complete.

## What was incomplete

- File cleanup implementation existed but shared its build gate with independently unreviewed provider uninstall.
- Durable cleanup plans/runs could not be rediscovered in the GUI after restart; worker errors could leave stale displayed approval.
- Migration execution repeated thirteen transaction blocks; the large workspace mixed cleanup rendering and orchestration.
- GUI database startup errors lacked a useful visible message; recovery process discovery used PATH lookup.
- The integration xtask was a placeholder; all-features-only checking hid build-profile errors.
- Version 0.0.0 and package provenance did not identify Alpha capability or uncommitted source.
- The current npm advisory service reported devalue serialization and Vitest tooling advisories.

## Changes and review scope

- Explicit Alpha file-cleanup feature uses existing exact-plan approval, identity/ownership/dependency revalidation, durable locks/journal, Recycle Bin quarantine, and permanent confirmation. Provider uninstall remains separately gated.
- Typed shared plan history, GUI recent-plan loading, interrupted-run rediscovery, failure reconciliation, and explicit preserved closure. No filesystem restoration/deletion occurs during preserved closure. Expired plans have a visible expiry state and disabled review/approval/execution controls requiring a new preview. Unknown outcomes remain visibly unresolved.
- CleanupReview presentation and pure wording helpers extracted; shared-core state remains authoritative. Blocking planning/approval workers keep filesystem scans out of synchronous desktop handlers.
- Central ordered migrations preserve per-version transactions and pre-upgrade backup. Startup errors are shown natively. Windows system-binary resolution is shared and used for recovery-owner checks. Path-free cleanup events supplement durable run/item records.
- Integration task executes actual suites; CI gate tests default, Alpha, and full reviewed feature profiles separately. Frontend audit includes development tools. Security fixes: devalue 5.9.3 and Vitest 4.1.11; see [devalue advisory](https://github.com/advisories/GHSA-j22f-vq7h-c4qm) and [Vitest advisory](https://github.com/advisories/GHSA-82fw-gwwq-j7x9).
- Alpha version consistency, feature provenance, complete source checksum snapshot, strict artifact inventory/checksum verification, and isolated real-Windows smoke tooling.
- Reviewed domain/application separation, typed contracts, persistence/recovery, removal/ownership, provider process/installer boundaries, portable archive bounds, frontend state/control flows, CSP/capabilities, configuration, diagnostics, and packaging. This is targeted internal review and does not claim every line or every runtime scenario is proven correct.

## Validation evidence

| Check                                       | Result                                                                                                                                                                                                                                                  |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Full local CI equivalent: cargo xtask check | Passed Rust formatting, strict Clippy, all-feature tests/builds, separate default/Alpha profiles, frontend formatting/lint/Svelte/tests/build, contract consistency                                                                                     |
| All-feature Rust tests                      | 87 passed; one performance test run separately                                                                                                                                                                                                          |
| GUI interaction tests                       | 19 passed, including interrupted-worker reconciliation and reopen/preservation recovery and expired saved-plan controls                                                                                                                                 |
| Real integration command                    | Passed SQLite/filesystem/recovery and CLI suites                                                                                                                                                                                                        |
| Rust dependency policy                      | Passed advisories, bans, licenses, sources; transitive duplicate-version/unmatched-allowance warnings retained                                                                                                                                          |
| Full pnpm audit                             | No known vulnerabilities reported after security fixes                                                                                                                                                                                                  |
| Release-mode 1,000-resource profile         | Passed: inventory 122 ms; storage aggregation 111 ms on this host                                                                                                                                                                                       |
| Real Windows fixture cleanup                | Passed owned file/directory platform Recycle Bin handoff, reference preservation, unconfirmed execution refusal, external permanent preservation, history, confirmed Cask removal, and database backup                                                  |
| Local desktop startup                       | Passed: visible Casklet window at 989 ms with a fresh isolated database; startup smoke only                                                                                                                                                             |
| Local silent install/reinstall/uninstall    | Passed: exits zero, program removed, Cask database reopened and managed sentinel content unchanged; prior install-location setting restored. Does not cover clean-machine upgrade or Windows Settings                                                   |
| CLI initial startup                         | Optimized release fixture first-open 1,534 ms; this does not measure desktop/WebView startup                                                                                                                                                            |
| Packaging                                   | Passed: Windows x64 Alpha CLI ZIP, NSIS installer, SPDX SBOM, notices, source snapshot, capability provenance, and nine verified artifact checksums. Explicitly dirty local candidate; unsigned. Evidence in release/alpha and target/alpha-package.log |
| Hosted GitHub Actions                       | Not dispatched in this task; local equivalent ran. Updated workflow includes full frontend advisory audit                                                                                                                                               |

The Windows host reports build 26200. Existing migration tests cover versions 1–12 upgrading to schema 13, corruption rejection, backup readability, and resource preservation. Existing adversarial tests cover links, identity replacement, quarantine swaps, overlap/sharing, interrupted journal boundaries, and protected paths. Automated/fake-provider evidence does not replace real installer/UAC tests.

## Remaining limitations and publication requirements

Acceptable for explicitly limited developer Alpha testing: Copy/Move intake temporarily disabled; folder/external permanent deletion unsupported; ambiguous/shared/missing data preserved; no sandbox, arbitrary installer tracing, updates, cloud, or other-OS release; WinGet uninstall disabled.

Not closed or represented as successful: independent destructive/elevation review and final path-based Recycle Bin handoff judgment; clean-machine installer/upgrade/Windows Settings uninstall; real UAC/provider interruption matrix; low-disk/multi-volume and screen-reader/high-DPI matrix; desktop cold startup/scan progress budget; publisher signing; exact third-party notice/MPL-source distribution review. These are prerequisites before claiming broad public Alpha release readiness.

The generated local candidate explicitly records uncommitted source. Public publication requires clean/tagged source, final candidate checksums, completed remaining gates, and an authenticated publisher path. No release has been published or tagged by this task.
