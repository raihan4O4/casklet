# Milestone 8: WinGet-managed lifecycle

## User flow

Search WinGet, choose an exact package ID and version, review its current provider details and warning, accept package agreements, and install it for an active Cask. CLI and desktop call the same Rust application service. The operation ID is emitted when durable intent is recorded; the user can inspect or cancel a running operation and inspect recovery state after failure.

## Provider boundary

The Windows adapter resolves the Desktop App Installer WinGet executable and verifies its binary hash around calls. It uses typed arguments for `search`, exact `show`, `export`, `install`, and `uninstall`; it never passes package data to a shell. Process output is bounded and sanitized. Search, show, and export have two-minute limits; mutations have a thirty-minute limit and cooperative cancellation. Export is parsed as structured JSON for the `winget` source and pinned versions. Missing provider capability is reported in the UI.

## Ownership and recovery

SQLite records WinGet intent and locks before external work. Installation ownership needs a package absent before, one exact ID/version after, and one new Windows registration matching the reviewed name. Existing registry IDs or uncertain outcomes preserve the operation for inspection without granting ownership. An uncertain operation retains its lock; closing it explicitly preserves any external result. Stale running operations become recovery-required when queried after ten minutes without a heartbeat. No database transaction spans a provider call.

## Uninstall

A plan captures the Casklet-owned installation, package identity, provider hash, registry evidence, record revision, expiry, and warnings. Plan creation, approval, and execution reject changed evidence or active associations. Execution revalidates immediately before a durable uninstall operation, checks provider and registry absence afterward, and only then marks the installation removed. The execution command is compiled only with `reviewed-removal`, which remains disabled for public builds pending independent security review.

## Validation

Unit and SQLite tests cover exact correlation, ambiguity, provider failure, lock preservation, association blockers, plan approval, and gated uninstall. A fake executable exercises argument construction, bounded execution, timeout, cancellation, sanitization, and export parsing. Real WinGet mutation remains a manual isolated validation step because it changes the host.
