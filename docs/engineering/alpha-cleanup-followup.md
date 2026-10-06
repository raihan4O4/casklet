# Alpha cleanup and application-management follow-up

Date: 2026-10-02. Design: ADR 0022. Target: Windows 11 x64, limited Alpha testing.

## Changes

| Request                     | Implementation                                                                                                                                                                                                                       |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Cask removal warning        | Native modal, Cancel receives initial focus, destructive Remove action, busy/cancel handling                                                                                                                                         |
| Recent Cleanups containment | Selected historical preview and its controls remain inside the disclosure                                                                                                                                                            |
| Installer simplification    | Type, signature, supported-management limit, trust warning, and actions; paths/hashes/provider diagnostics/history stay collapsed                                                                                                    |
| Managed application cleanup | Explicit MSI management before launch or after verified detection; durable pre-launch absence evidence, exact ProductCode, current identity, sharing checks, expiring preview, approval, journaled uninstall, and preserved recovery |
| Console suppression         | Common Windows process option used for portable launches and background installer/provider/recovery helpers; installer UI and UAC remain visible                                                                                     |
| Cask row menu               | Hidden for Alpha; underlying typed handlers retained; four-column layout                                                                                                                                                             |
| Activity Clear              | Persisted display filter for completed entries; active/recovery work and durable journals are retained                                                                                                                               |
| Storage retirement          | Tombstone first, empty-directory pruning, audit metadata moved to retired-casks; unknown/recovery bytes preserved with a separately persisted storage warning                                                                        |
| Immediate list update       | Dedicated removal event invalidates the Cask cache; query generation prevents an older asynchronous result from resurrecting it                                                                                                      |
| Add Reference               | Redundant button removed; File/Folder still offer Reference/Managed behavior                                                                                                                                                         |
| Overview detail             | Collapsed Details contains storage location and dates; physical-size placeholder removed                                                                                                                                             |

## Supported ownership boundary

The managed MSI choice is explicit permission to include a new application in approved cleanup. Registration alone never grants ownership. A successful MSI launch must have a durable absent-before baseline, one exact product candidate, Windows Installer registration evidence, and unchanged current registration. A missing baseline, preexisting product, EXE, reboot requirement, failure, ambiguity, or interrupted launch cannot gain ownership. Legacy records remain association-only.

The Windows adapter launches only system msiexec with typed `/x`, exact GUID ProductCode, and `/norestart`. It never parses or executes registry uninstall command strings. Uninstall requires an approved current hash-bound plan. Changed/missing/shared products are preserved. Intent and result are durable before and after every external action. A nonzero exit, timeout, changed evidence, failed verification, or process interruption retains uncertain state and its lock. Closing never retries an uninstall or assumes that it succeeded. Explicit Keep releases Cask ownership/association while leaving software installed.

Application cleanup precedes file cleanup in Whole Cask UI. Each stage has its own reviewed approval, so the file plan uses the post-uninstall resource state. A remaining reference or unsupported installation must be explicitly detached/preserved or removed through its supported provider before empty-Cask removal.

## Shared interfaces and migration

Schema 14 adds MSI baselines, ownership records, cleanup plans/steps/locks, and persisted storage-retirement outcomes. No existing launch is promoted to ownership. Existing schema 1–13 data upgrades through the ordered migration registry with pre-upgrade backup. Database metadata backup includes ownership and journals; recovery manifests deliberately reconstruct references without uninstall authority. MSI association mutations schedule fresh manifest projections.

CLI:

- `installer launch <artifact-id> --hash <hash> --accept-risk --manage-msi`
- `msi manage <operation-id> <installation-id> --yes`
- `msi preview <cask-id>` / `msi latest <cask-id>`
- `msi approve <plan-id> --hash <hash>`
- `msi execute <plan-id> --hash <hash> --yes`
- `msi close <plan-id> --yes` preserves interrupted outcomes
- `msi keep <cask-id> <installation-id> --yes` keeps software and detaches ownership

Alpha builds expose MSI approval/execution; default GUI/CLI builds remain review-only. WinGet uninstall remains under its independent gate. Arbitrary EXE automatic uninstall remains unsupported: correlated names/publishers and raw uninstall strings are insufficient authority. The UI explains this before launch. This limitation is intentional, not a promised EXE cleanup implementation.

## Validation and remaining release gates

Automated tests cover integrated managed MSI launch/baseline/claim, association-only legacy and uncertain outcomes, shared preservation, current evidence, tampered/expired plan refusal, approval before execution, verified disappearance, durable interruption, no replay, explicit preserved release, storage retirement and unknown/quarantine preservation, native removal modal, immediate cache invalidation under stale query results, Activity persistence, collapsed installer diagnostics, and the approval/execution GUI sequence.

No real third-party MSI/EXE is installed or uninstalled on the developer's machine by these tests. Fake adapter evidence and real file-cleanup fixtures do not replace isolated Windows Installer/UAC/reboot tests, independent destructive review, signing, accessibility, or distribution-license clearance. Public-release gates remain open. See alpha-readiness.md for the earlier candidate evidence; its earlier artifact snapshot predates this follow-up.

Windows Installer command semantics: [Microsoft msiexec documentation](https://learn.microsoft.com/windows-server/administration/windows-commands/msiexec).

### Follow-up validation results

- `cargo xtask check`: passed; 96 Rust tests and 24 GUI tests, plus default/Alpha profile checks, formatting, strict Clippy, frontend lint/type checks, generated contracts, and production builds.
- `cargo deny check` and complete frontend dependency audit: passed; no known frontend vulnerabilities.
- Windows Alpha packaging: passed; NSIS installer, CLI archive, notices, SPDX SBOM, source snapshot, provenance, and nine artifact checksums verified. The candidate is unsigned and records its dirty working-source snapshot.
- Optimized Alpha fixture smoke: passed, including Recycle Bin cleanup, reference preservation, permanent-deletion safeguards, history, backup, and complete empty Cask storage retirement. Evidence: `target/alpha-smoke-c6d38692-00a3-42b0-b30e-0686c8b9e67f/result.json`.

Logs are retained under `target/alpha-followup-*`. These are local validation results; hosted GitHub CI and real MSI/UAC/reboot validation are not claimed.
