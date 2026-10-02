# Changelog

## Unreleased — Simple cleanup and permanent Cask removal

- Add explicit 0.1.0-alpha.1 file-cleanup builds without enabling provider uninstall.
- Persist and rediscover cleanup history; reconcile failed workers and require explicit uncertain-run preservation.
- Extract cleanup presentation, consolidate migrations and system executable resolution, and report GUI startup errors.
- Patch frontend security dependencies; validate feature profiles separately and replace the integration task placeholder.
- Add source-snapshot/capability provenance, complete artifact verification, and isolated real Windows cleanup smoke tooling.

- Simplify cleanup to two footer actions, a compact reviewed state, visible deletion choices, and clear review-only limits; expand resource details inline with reduced-motion support.
- Add confirmed permanent removal of truly empty Casks with atomic dependency checks and audit tombstones.
- Show Review, Approve, and Execute controls; hide diagnostics under Advanced details.
- Make bottom Cleanup navigation-only; keep Archive in Settings and remove Storage Location there.
- Temporarily disable generic user Copy/Move intake in GUI and CLI, preserving legacy/provider code and recovery.

## Unreleased — Ownership and cleanup UX

- Expose reversible Cask archive/restore in GUI and CLI independently of resource cleanup.
- Reuse equivalent cleanup plans and explain actions, reasons, and approval/execution steps.
- Add explicit ownership at the current location while retaining copy, move, and reference intake.
- Preserve changed, shared, missing, linked, and unverified data; journal external quarantine recovery.
- Upgrade to schema 12 without relocating data or granting ownership to existing references.

## Unreleased — Milestone 10 release hardening

- Separate installed per-user state from development data.
- Reject corrupt database startup and back up existing databases before schema upgrades.
- Add visible keyboard focus, skip navigation, and reduced-motion handling.
- Add per-user NSIS packaging, CLI ZIP, checksum, provenance, and SPDX SBOM generation tooling.
- Document user safety, recovery, privacy, release evidence, and pending manual gates.
- License Casklet under Apache-2.0 and include its license in candidate artifacts.
- Add a shared database backup action to CLI and desktop Settings, plus an operation history view.

All notable changes will be documented here.

## Unreleased

- Began Milestone 0 reproducible repository foundation.
