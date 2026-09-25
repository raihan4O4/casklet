# ADR 0012 — Journaled Recycle Bin cleanup

Status: implemented behind the disabled `reviewed-removal` feature; pending independent security review (2026-09-25).

## Decision

Execution accepts only an approved, unexpired plan with the exact stored hash. The executor claims a per-Cask mutation lock and writes a removal run before acting. Each item is checked against its database snapshot and marked started before an external step. Managed resources are checked against their recorded Windows identity again immediately before the action. Referenced items are detached. Unverified or changed items are preserved. Fully inspected new managed directories have recorded tree evidence; legacy directories without it remain preserved.

For an eligible resource, the Windows adapter renames it to a unique path under the Cask's `.casklet/removal-staging` directory, verifies the moved object's identity, checks it again, then asks the OS Recycle Bin adapter to trash it. The run records `UNKNOWN` and retains its lock if the adapter returns an uncertain result or a process dies after an item starts. No uncertain item is automatically retried. Explicit closure releases the lock but retains `UNKNOWN` and `CLOSED_UNRESOLVED` in the item journal; a possible quarantine location is shown for inspection. If the original is restored there, a fresh plan can verify it and retry. Cask and association history remain in SQLite.

The removal journal uses `removal_runs` and `removal_plan_items` alongside the existing intake operation journal. This keeps the external trash boundary and per-item uncertainty explicit without retroactively changing the intake operation schema. Legacy draft plans are invalidated by migration 0005 because they did not store the sharing count needed to recompute the full snapshot hash.

## Review gate

The default CLI and desktop builds do not enable execution. The path-based Recycle Bin call still has a small time-of-check/time-of-use window after quarantine verification. Before enabling `reviewed-removal` in a public build, an independent security review must examine this window, Windows reparse and hard-link behavior, quarantine permissions, rollback/recovery handling, and the CLI and desktop confirmation flows. The Milestone 4 adversarial filesystem tests and live Windows smoke tests must also pass.
