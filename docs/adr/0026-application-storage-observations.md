# ADR 0026: State-derived application storage observations

Status: Accepted, 2026-10-07.

## Decision

Extend the existing shared storage summary and persistence/platform ports. Active associations select installations; persistent ownership classifies them; current read-only Windows observations establish presence and size. Do not store UI deltas, infer ownership from location, hash mutable size into uninstall evidence, or create a parallel application lifecycle.

Exclude typed installer resources from the displayed file/application totals. Preserve their separate registration and planned cleanup. Deduplicate Windows user-view aliases and portable resource-backed installations. Include reference application sizes by default; defer the optional per-Cask exclusion setting.

Measure a specific, unshared installation directory with existing bounded link-aware filesystem inspection. Reject canonical broad roots, incomplete scans and overlapping installation paths as complete measurements. Fall back to labelled Windows EstimatedSize, or expose unknown size. Run desktop measurements outside the native event loop.

## Consequences

Successful installation registration begins accounting; association adds reference accounting; detach and verified uninstall remove accounting through current database state. Missing live registrations contribute no bytes while history remains. Size changes do not revoke deletion evidence. Folder measurements are logical installation-directory bytes, not a guarantee of all shared components or physical allocation. Estimated application bytes never increase automatic reclaimable-space claims.

Details and validation: docs/engineering/application-storage.md.
