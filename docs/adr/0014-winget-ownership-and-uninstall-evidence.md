# ADR 0014: WinGet ownership requires independent instance evidence

Status: Implemented for Milestone 8; uninstall is gated by `reviewed-removal` pending independent security review.

## Context

WinGet exit status does not prove that a newly installed package belongs to a Cask. A package may already exist, installations may be shared, and provider or registry observations may be incomplete.

## Decision

Keep the provider behind application ports and call a verified WinGet executable with typed arguments. Record intent and a Cask lock before process execution. Claim an installation only when a prior WinGet export did not contain its package ID, a later export contains one exact ID and version, and Windows inventory contains exactly one new registration matching the reviewed package name. Retain provider binary and registry evidence hashes. Failed, cancelled, timed-out, or ambiguous work enters a durable recovery state without ownership and retains its lock until explicitly closed after inspection.

Uninstall uses a separate inspectable plan tied to the owned record revision and evidence. Approval and execution require current provider, package, and registry evidence, plus no active associations. Execution requires the `reviewed-removal` feature and verifies absence before marking the record removed. WinGet operations and plans have dedicated SQLite tables because the existing resource intake journal restricts its operation type.

## Consequences

Some legitimate installs cannot be claimed automatically, particularly when WinGet export or registry correlation is incomplete. Closing an uncertain operation preserves the external installation and never grants ownership. A ten-minute stale heartbeat marks an interrupted operation for recovery when inspected. Real WinGet mutation still needs isolated manual validation before public release.
