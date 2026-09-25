# Milestone 4 — Removal planning and safe cleanup

Status: in progress on `codex/removal-plans-safe-cleanup`. This branch is based on Milestone 3 PR #6 until that PR merges into `develop`.

## First slice: immutable preview

`casklet plan create <cask-id>` creates a durable, expiring, read-only preview for the whole Cask. Repeat `--resource <id>` to select resources, or use `--category <key>`. `casklet plan get <plan-id>` reloads it. The desktop offers the same preview for a Cask, category, or selected resource. Plan IDs, items, revision snapshots, expiry, and a canonical SHA-256 equality hash are stored in SQLite. The hash is an integrity comparison, not a signature or authorization.

The domain policy requires managed, internal, Casklet-created, owned, unshared, present, verified, narrow, fully scanned resources before proposing trash. A reference is proposed for detach only. Ambiguous managed data is preserved. Newly staged managed files record a Windows file identity and content digest, so they can receive a `TRASH` proposal after current verification. The planner preserves a replaced file even when its bytes are unchanged. Resource records from Milestones 1–3 have no durable filesystem identity baseline and remain preserved. Directory identity is not yet recorded, so managed directories are also preserved.

This slice has no approve, execute, trash, or permanent-delete command or Tauri endpoint. A plan preview does not modify resources. The desktop explicitly labels it as a preview.

## Remaining Milestone 4 work

- Complete directory identity and provenance validation; keep legacy items preserved until verified through an explicit recovery path.
- Make the plan hash cover every executable precondition and invalidate plans when relevant state changes. Add approval tied to that exact hash and expiry.
- Implement a Windows Recycle Bin port and a journaled executor that revalidates identity, paths, associations, revisions, and plan integrity immediately before each action. Explicit permanent deletion needs a separate narrow action and stronger confirmation.
- Handle interrupted and partial execution with resumable, truthful outcomes; retain Cask lifecycle history.
- Add symlink/junction and path-replacement fault tests, CLI and GUI approval workflows, and an independent security review before destructive endpoints are enabled in a public build.

## Review status

The current branch is not ready for Milestone 5. Approval, execution, retry, Cask lifecycle completion, and the required independent security review are outstanding. Migration 0005 reserves a durable removal run and lock journal, and startup marks abandoned runs for recovery while retaining their locks. No code creates a removal run or calls the Recycle Bin adapter yet; these tables are preparatory, not evidence that execution is complete. The default CLI and desktop build have no destructive removal entry point.
