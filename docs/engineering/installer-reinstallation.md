# Installer reinstallation and completed-review recovery

## October 11 diagnosis

The real-test journal recorded PowerToysUserSetup-0.101.2362.0-x64.exe exiting zero with one new matching Windows registration (MEDIUM confidence). Detection succeeded. The same stable installation ID already existed in managed_msi_installations in REMOVED state from an earlier installation. Ownership creation used an unconditional INSERT against that primary key, so reinstall confirmation failed. The current operation remained REVIEW_REQUIRED. Its installer lock was already released, but the GUI treated completed review as a mutation blocker. Captured registry-view snapshots differ; that observation alone does not establish the cause of this incident. Live PowerToys registration/files were not available to the inspection process, so current Windows presence is not claimed.

## Repair

The existing ownership transaction conditionally replaces a REMOVED or RELEASED owner with the newly verified installation operation, current registration snapshot and executable digest. All absent-before/correlation/current-evidence/unshared/active checks remain required; active owners cannot be replaced. Historical installer operations and cleanup plans remain journaled. No schema change or existing-app association workflow is used.

On workspace refresh, one saved successful REVIEW_REQUIRED result with a single HIGH/MEDIUM candidate is retried through the existing shared ownership command. No installer is executed by this retry; changed/missing/shared evidence rejects it. Attempts are bounded to once per operation per mounted workspace. Successful recovery refreshes applications/storage through the existing refresh sequence and emits the normal library invalidation event. A completed review does not disable new installer selection; actual running or uncertain recovery operations retain their safety block.

This is not an exit-code bypass. Timeout, reboot, incomplete inventory, process-tree uncertainty and ambiguity still cannot grant ownership. Application installation success and ownership-registration failure are distinct.

## Verification

Regression coverage exercises EXE and MSI installation, approved sequential cleanup and verified retirement, repeated reinstall using the same identity, and recovery of saved successful operations without additional installer launches. Existing tests continue to protect references, ambiguity, changed evidence, cancellation and uncertain process outcomes. GUI coverage checks saved-result recovery and availability of installer selection afterward. Native third-party install/uninstall is not performed on the user's applications by this change.

Validation evidence: target/reinstall-quality.log records successful Rust formatting, strict workspace Clippy, all-feature and default/Alpha test suites and builds. After correcting frontend-only Svelte lint/type requirements, target/reinstall-frontend-final.log records formatting, lint, zero Svelte diagnostics, 47 GUI tests and production build passing. target/reinstall-alpha-smoke.log records isolated managed/reference cleanup and storage preservation. Package evidence is target/reinstall-package.log and target/reinstall-verification.log.
