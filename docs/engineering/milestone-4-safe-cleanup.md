# Milestone 4 — Removal planning and safe cleanup

Status: in progress on `codex/removal-plans-safe-cleanup`. This branch is based on Milestone 3 PR #6 until that PR merges into `develop`.

## First slice: immutable preview

`casklet plan create <cask-id>` creates a durable, expiring, read-only preview for the whole Cask. Repeat `--resource <id>` to select resources, or use `--category <key>`. `casklet plan get <plan-id>` reloads it. The desktop offers the same preview for a Cask, category, or selected resource. Plan IDs, items, revision snapshots, expiry, and a canonical SHA-256 equality hash are stored in SQLite. The hash is an integrity comparison, not a signature or authorization.

The domain policy requires managed, internal, Casklet-created, owned, unshared, present, verified, narrow, fully scanned resources before proposing trash. A reference is proposed for detach only. Ambiguous managed data is preserved. Resource records from Milestones 1–3 have no durable filesystem identity baseline, so the current planner preserves them even if their paths and sizes still look correct. A current inspection cannot establish that a path still names the original object. The new nullable `identity_key` column reserves this evidence for a later implementation; it is intentionally unpopulated for legacy resources.

This slice has no approve, execute, trash, or permanent-delete command or Tauri endpoint. A plan preview does not modify resources. The desktop explicitly labels it as a preview.

## Remaining Milestone 4 work

- Record strong platform file identity at managed registration, with migration behavior that preserves legacy items until verified through an explicit recovery path.
- Make the plan hash cover every executable precondition and invalidate plans when relevant state changes. Add approval tied to that exact hash and expiry.
- Implement a Windows Recycle Bin port and a journaled executor that revalidates identity, paths, associations, revisions, and plan integrity immediately before each action. Explicit permanent deletion needs a separate narrow action and stronger confirmation.
- Handle interrupted and partial execution with resumable, truthful outcomes; retain Cask lifecycle history.
- Add symlink/junction and path-replacement fault tests, CLI and GUI approval workflows, and an independent security review before destructive endpoints are enabled in a public build.
