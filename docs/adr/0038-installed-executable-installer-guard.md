# ADR 0038: Reject installed application executables as installers

Status: accepted by the user's executable/locked-Cask bug report, 2026-10-22.

## Problem

MSI/EXE selection previously treated an ordinary installed application's executable as a potential installer. Copying and running it can fail or launch a long-lived application instead of completing installation. An uncertain launch deliberately retains its installer mutation lock. The desktop no longer exposed the existing preserved-closure command, leaving users without a recovery path.

## Decision

Add an installer-source capability that checks whether an EXE belongs to an installed application without executing or hashing it. The Windows adapter uses current uninstall registrations' specific installation folders, exact DisplayIcon executable paths, and App Paths launch registrations. Canonical paths and native file identity cover normal aliases; broad installation roots such as Program Files, Windows, profiles and common user-content roots are not treated as one application's directory. File names, signatures and product-name matches alone do not classify a file as an installed application.

Shared-core installer inspection rejects a positive match with `INSTALLED_APPLICATION_EXECUTABLE`, before signature/content inspection, registration, copying or lock creation. Registration and one-action install share that check; launch rechecks the original source and managed path before taking its lock, covering previously registered artifacts. This protects GUI, Explorer and CLI callers through the same application core. Genuine setup files outside matched installation paths remain supported, including installers with the same product name as an existing installation. Unknown executables still require explicit execution approval; absence of a match does not prove a file is an installer.

The desktop shows a persistent explanation and **Find installed application** route to association. It does not silently run or associate the executable, and detection never grants ownership or uninstall authority.

Restore the existing explicit `close_installer_operation` workflow for REVIEW_REQUIRED and RECOVERY_REQUIRED attempts. **Close attempt and keep files** remains disabled until the user confirms the installer/application and its child processes have finished. Closure durably preserves the outcome, releases the exact installation lock, and refreshes the Cask. No resources, applications or user files are deleted; no installation is claimed. Running or uncertain workers are not automatically declared safe, and process errors never imply rollback. Existing CLI/API recovery semantics and removal revalidation remain unchanged. No schema migration is required.

## Validation

Tests verify rejection through inspect/register/install with zero executions, resources, artifacts, operations and locks; launch revalidation of stored artifacts; acceptance of separate setup files; and native registry/DisplayIcon/hard-link detection using disposable Windows fixtures. Existing installer failure, ownership, cancellation and preserved-closure tests remain passing. UI tests cover persistent warning, association routing and confirmation-gated closure of an existing uncertain attempt.

The full infrastructure suite passes 136 tests (one opt-in ignored); 90 frontend tests, strict Rust/frontend checks, production and native builds pass. Eight Light/Dark production captures have no axe violations, overflow or console errors. A real native check rejects the host's registered Google Chrome executable through inspection and install requests, leaving zero resources/operations and `canRemove=true` in isolated QA state. No Chrome process was launched or installation changed. Evidence: `target/installed-executable-guard-tests.log` and `.jury/installed-executable-guard`.
