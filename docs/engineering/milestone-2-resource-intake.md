# Milestone 2 — Resource intake and inspection

Status: implementation complete on `codex/resource-intake-inspection`; awaiting PR review against `develop`.

The CLI follows the approved `casklet resource add|list|get|categorize`, `casklet category list`, and `casklet storage summary` routes. Adding requires an explicit `--mode copy|move|reference`. The desktop picker and drop handler stage one path for review; the user must choose a mode and press Add resource. Both entry points call the same application service.

SQLite stores resource identity, location, Cask association, category, provenance, size measurement, and a persistent intake intent separately. A reference remains external with a detach-only policy. A copy writes to private staging, verifies the staged size, then renames into managed storage. A move uses filesystem rename into staging and final storage; it only succeeds when the filesystem supports those renames. An unfinished intent retains its source, staging, and final paths for Milestone 3 recovery work. No resource deletion is exposed here.

The directory inspector bounds traversal to 100,000 entries and depth 64. It retains a filesystem handle identity for the source and rechecks it before materialization; move also checks the staged identity. It reports unknown entries when links, reparse points, inaccessible entries, or limits prevent exact measurement. Managed copy and move reject a source with unknown entries. Queries spot-check current availability, so a missing reference is not shown as present or counted as current storage. Physical size remains unknown; reclaimable size is a range with zero as the lower bound. These values must not be presented as exact disk savings.

The tests cover copy, move, reference, category changes, collision preservation, symlinks and Windows junctions, inaccessible-path classification, simulated disk-full writes, an interrupted copy with a surviving intent, missing-reference availability, source/destination recursion, and migration from a Milestone 1 database. The full Windows workspace quality gate passed locally.

Milestone 3 will implement reconciliation of unfinished intents and retryable manifest projections. A failed intake currently requires manual inspection of the recorded staging paths. Concurrent changes inside a directory tree can invalidate a scan; managed copy rejects links and rechecks roots, and the removal milestone must strengthen path validation before any deletion. See [ADR 0009](../adr/0009-staged-resource-intake.md).
