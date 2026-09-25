# ADR 0009: Staged resource intake and same-volume moves

Status: Accepted for Milestone 2 on 2026-09-25.

## Context

Copy and move change filesystem state outside a SQLite transaction. Milestone 2 must preserve external sources when an intake fails and leave enough evidence for Milestone 3 recovery. A cross-volume move would require copying followed by source removal, which needs the stronger operation and recovery machinery planned for Milestone 3.

## Decision

- Record an intake intent with source, staging, and final paths before touching managed storage.
- Copy into operation-specific staging, verify the staged size, and rename it into managed storage.
- Move through same-volume renames into staging and final storage. Reject moves that the filesystem cannot perform atomically; never fall back to deleting the source.
- Recheck source identity and reject links, junctions, reparse points, unknown directory entries, and destinations contained by the source.
- Preserve an incomplete staging path and its intake intent for later reconciliation. A failed operation is never reported as complete.
- Keep references external with detach-only removal policy.

## Consequences

Cross-volume moves are not supported in this slice. Milestone 3 must expose and reconcile unfinished intents, and implement retryable manifest projection. The Milestone 4 removal work must retain stronger component-by-component path checks before any deletion. The intake UI remains functional and explicit; visual redesign is deferred.
