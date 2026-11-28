# ADR 0021 — Explicit Alpha cleanup build and durable review history

Status: Accepted for local/limited Alpha validation on 2026-10-02 at the user's request. Independent broad-distribution security approval is not claimed.

## Context

Cleanup execution already exists, but reviewed-removal also authorizes provider uninstall. The user explicitly requested a functional Alpha after testing the review-only build. Reopening the GUI lost access to interrupted cleanup, although SQLite retained its journal and lock.

## Decision

Add alpha-cleanup as an explicit CLI/desktop build feature that authorizes the existing file planner/approval/executor and preserved resolution. It does not enable WinGet uninstall or alter filesystem/domain policy. Default builds remain review-only; reviewed-removal remains the independent-review profile. Alpha packaging explicitly selects alpha-cleanup for both interfaces. No runtime toggle or GUI filesystem privilege is introduced.

Add a versioned ListRemovalPlans query to the shared core. History prioritizes interrupted/running plans, excludes removed Casks, and is bounded to 50 plans. A reopened Cask rediscovers incomplete cleanup. Recent cleanups can reload the exact durable plan; local review/confirmations always reset. Closing uncertain work requires explicit preservation confirmation and never restores or deletes filesystem data. Unresolved outcomes remain visible as needing inspection even when the run is closed.

Move cleanup presentation into a controlled CleanupReview component and pure wording helpers; command/state orchestration stays in Workspace and shared Rust core. Run filesystem-heavy planning and approval on blocking workers. Use a centralized ordered migration sequence with integrity checking, backup before upgrade, and a short transaction per version.

Record Alpha version and capabilities in package provenance. Clean source is required by default. An explicit allow-dirty option is limited to local Alpha candidates and records a complete source-file checksum snapshot; these candidates are not tagged public releases. Artifact checksums include provenance and the snapshot, with exact inventory verification.

## Consequences

The developer can validate real file cleanup without claiming provider or independent review certification. Remaining same-user path-based Recycle Bin handoff risk, clean-machine/upgrade/UAC/accessibility checks, signing, and notice/source obligations remain publication gates. Alpha testing should use backed-up dedicated data. Public release requires a clean/tagged candidate and recorded outstanding validation results.
