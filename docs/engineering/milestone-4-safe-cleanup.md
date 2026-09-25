# Milestone 4 — Removal planning and safe cleanup

Status: in progress on `codex/removal-plans-safe-cleanup`. This branch is based on Milestone 3 PR #6 until that PR merges into `develop`.

## First slice: immutable preview

`casklet plan create <cask-id>` creates a durable, expiring, read-only preview for the whole Cask. Repeat `--resource <id>` to select resources, or use `--category <key>`. `casklet plan get <plan-id>` reloads it. The desktop offers the same preview for a Cask, category, or selected resource. Plan IDs, items, revision snapshots, expiry, and a canonical SHA-256 equality hash are stored in SQLite. The hash is an integrity comparison, not a signature or authorization.

The domain policy requires managed, internal, Casklet-created, owned, unshared, present, verified, narrow, fully scanned resources before proposing trash. A reference is proposed for detach only. Ambiguous managed data is preserved. Newly staged managed files record a Windows file identity and content digest, so they can receive a `TRASH` proposal after current verification. The planner preserves a replaced file even when its bytes are unchanged. Resource records from Milestones 1–3 have no durable filesystem identity baseline and remain preserved. Directory identity is not yet recorded, so managed directories are also preserved.

The default build exposes preview commands only. The Rust application core has an approval operation that checks the exact hash, expiry, Cask revision, resource and association revisions, location, sharing count, scope, and current managed-file identity before changing a plan to `APPROVED`. Reloading a plan recomputes its snapshot hash. Migration 0005 invalidates earlier draft plans because those rows lacked a stored sharing count; users can generate fresh previews.

Approval, execution, and explicit preserved close are implemented behind the disabled `reviewed-removal` feature. The executor journals every item, revalidates each managed file immediately before action, stages it under the Cask, verifies the moved object, and then uses the Windows Recycle Bin adapter. A changed target is preserved. An uncertain adapter result keeps the run locked for recovery. Whole-Cask completion records `DESTROYED` or `PARTIALLY_DESTROYED` without deleting history. The CLI requires a typed confirmation or `--yes` for execution and preserved close; `--no-input` refuses to proceed without `--yes`. The desktop requires the exact plan or run ID and shows warnings when the feature is enabled. See ADR 0012.

## Remaining Milestone 4 work

- Complete directory identity and provenance validation; keep legacy items preserved until verified through an explicit recovery path.
- Validate current filesystem evidence again during execution; a prior approval never authorizes a destructive step by itself.
- Complete directory tree identity before any directory cleanup is considered. Permanent deletion remains outside this feature and would require a separate narrow action and confirmation.
- Add live Windows restart, hard-link, and post-quarantine swap tests. Confirm recovery staging and restore behavior on real filesystems.
- Complete an independent security review of the executor and the path-based trash handoff before enabling `reviewed-removal` in a public build.

## Review status

The current branch is not ready for Milestone 5. Default builds still have no destructive removal entry point. The gated executor has unit and integration coverage for successful cleanup, replacement and junction swaps after approval, uncertain trash, preserved close, and abandoned runs. An isolated Windows CLI smoke test on 2026-09-25 moved a managed file to the Recycle Bin, retained its source, marked the item succeeded, and marked the Cask destroyed. The temporary test workspace was removed; one test file remains in the Recycle Bin. Restart and post-quarantine adversarial checks plus the required independent review remain before the feature can be enabled and the milestone accepted.
