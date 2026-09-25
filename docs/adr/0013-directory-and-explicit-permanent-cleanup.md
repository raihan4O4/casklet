# ADR 0013: Verified directories and explicit permanent file cleanup

Status: Implemented behind `reviewed-removal`; independent security review pending.

## Context

Milestone 4 includes managed-directory cleanup and an explicitly chosen permanent-delete option. A directory path alone is insufficient evidence of its contents, and permanent recursive deletion would expand the destructive surface.

## Decision

For a newly managed directory, record a bounded Windows tree fingerprint containing each relative entry name, entry kind, filesystem identity, and file content digest. Reject links, reparse points, partial scans, and trees exceeding the depth or entry limit. Recompute the fingerprint at planning, approval, execution, and after moving the directory into quarantine. Eligible directories can be moved to the Recycle Bin. Legacy directories without recorded evidence remain preserved.

The default plan uses the Recycle Bin. A plan created with an explicit permanent choice stores that choice in SQLite and includes it in the plan hash. Only eligible managed files receive `DELETE_PERMANENTLY`; directories are shown as preserved in that plan. Execution requires a second permanent confirmation and uses `remove_file` on a verified quarantined file. Both modes use the same durable run journal and preserve uncertain outcomes. Migration 0006 invalidates older draft and approved plans.

## Consequences

Large or uninspectable trees cannot be automatically removed. A user must make a separate Recycle Bin plan for directories when choosing permanent disposal for files. The path-based handoff after quarantine still has a race window; independent security review and adversarial Windows testing are required before enabling the feature in a public build.
