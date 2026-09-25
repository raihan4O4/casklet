# ADR 0010: Explicit intake recovery and preserved close

Status: Accepted for Milestone 3 on 2026-09-25.

## Context

SQLite cannot atomically commit a filesystem rename. A process may stop after creating an intent, during staging, after the final rename, or after resource registration. Automatically claiming success from a path's existence could adopt unrelated or changed data. An abandoned operation also retains a Cask mutation lock.

## Decision

- Persist operation and step states, an intake snapshot, progress, cancellation, a process owner, and a lock before managed filesystem changes.
- Treat a missing or uncertain process-liveness result conservatively: only a confirmed dead owner is marked abandoned automatically; the lock is not stolen.
- Require an explicit recovery command to accept an observed staged or final item. Verify recorded allocation, source state for moves, kind, size, links, and bounded directory inspection before registering it.
- Offer a separate preserved close action for ambiguous work. It releases the lock only when the operation is recovery-required and its owner is gone or the current process has already recorded failure. It never removes or moves candidate files.
- Keep manifest projection as a retryable job derived from committed SQLite state. A failed projection does not roll back committed resource state.
- Keep diagnostic events to fixed codes and operation IDs; private filesystem paths appear only in an explicitly requested recovery inspection.

## Consequences

Legacy Milestone 2 intents lack enough snapshot fields for automatic completion and require inspection or preserved close. A complete stage can be accepted after explicit verification, but size and kind are not a cryptographic content identity; ambiguous evidence remains preserved. Cross-volume moves remain unsupported. Removal still requires its own plan and stronger path revalidation in Milestone 4.
