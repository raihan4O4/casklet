# Milestone 4 — Removal planning and safe cleanup

Status: in progress on `codex/removal-plans-safe-cleanup`. This branch is based on Milestone 3 PR #6 until that PR merges into `develop`.

## First slice: immutable preview

`casklet plan create <cask-id>` creates a durable, expiring, read-only preview for the whole Cask. Repeat `--resource <id>` to select resources, or use `--category <key>`. `casklet plan get <plan-id>` reloads it. The desktop offers the same preview for a Cask, category, or selected resource. Plan IDs, items, revision snapshots, expiry, and a canonical SHA-256 equality hash are stored in SQLite. The hash is an integrity comparison, not a signature or authorization.

The domain policy requires managed, internal, Casklet-created, owned, unshared, present, verified, narrow, fully scanned resources before proposing trash. A reference is proposed for detach only. Ambiguous managed data is preserved. Newly staged managed files record a Windows file identity and content digest. Fully inspected managed directories record a bounded tree fingerprint of entry names, kinds, filesystem identities, and file contents. The planner preserves replaced or changed resources. Resource records from Milestones 1–3 without durable filesystem identity remain preserved.

The default build exposes preview commands only. The Rust application core has an approval operation that checks the exact hash, expiry, Cask revision, resource and association revisions, location, sharing count, scope, and current managed-file identity before changing a plan to `APPROVED`. Reloading a plan recomputes its snapshot hash. Migration 0005 invalidates earlier draft plans because those rows lacked a stored sharing count; users can generate fresh previews.

Approval, execution, and explicit preserved close are implemented behind the disabled `reviewed-removal` feature. The executor journals every item, revalidates each managed resource immediately before action, stages it under the Cask, verifies the moved object, and then uses the Windows Recycle Bin adapter. A changed target is preserved. An uncertain adapter result keeps the run locked for recovery. Whole-Cask completion records `DESTROYED` or `PARTIALLY_DESTROYED` without deleting history. An explicitly selected `--permanent` plan can permanently delete eligible managed files only; directories are shown as preserved and can be handled with a separate Recycle Bin plan. Execution requires `--confirm-permanent` in addition to normal CLI confirmation, or a separate desktop acknowledgment. Migration 0006 records the disposal choice, allows the permanent action, and invalidates old draft plans. See ADRs 0012 and 0013.

## Remaining Milestone 4 work

- Keep legacy items preserved until verified through an explicit recovery path.
- Add live Windows hard-link swap and post-quarantine swap tests for files and directories. Confirm Recycle Bin failure handling on supported filesystems.
- Complete an independent security review of the executor and the path-based trash handoff before enabling `reviewed-removal` in a public build.

## Review status

The current branch is not ready for Milestone 5. Default builds still have no destructive removal entry point. The gated executor has unit and integration coverage for successful file and directory cleanup, replacement and junction swaps after approval, directory tree changes, explicit permanent-file confirmation, uncertain trash, preserved close, abandoned runs, and a hard exit after quarantine. Isolated Windows CLI smoke tests on 2026-09-25 moved a managed file and a managed directory to the Recycle Bin, retained their sources, and completed the Cask lifecycle. Temporary test workspaces were removed; the two generated items remain in the Recycle Bin. Hard-link swap and post-quarantine adversarial checks plus the required independent review remain before the feature can be enabled and the milestone accepted. The focused review checklist is `docs/security/milestone-4-removal-review.md`.
