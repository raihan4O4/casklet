# ADR 0020 — Permanent Cask removal and simple cleanup

Status: Accepted for implementation under the October 2 follow-up request. Supersedes ADR 0019's archive-only Cask removal presentation.

## Decision

Archive is reversible and remains in Settings. Remove permanently retires a Cask after explicit confirmation, expected-revision checking, and an atomic database check that no active resource/application associations, provider-owned installations, or unfinished operations remain. This shared command works in public builds because it never deletes resource bytes. Retain a durable removal tombstone and historical journals for audit, foreign-key integrity, and protection against stale-manifest resurrection. The Cask disappears from normal queries and cannot be restored. Do not recursively delete its storage directory; retained metadata is recovery/audit evidence, not an archived Cask.

Whole Cask cleanup considers resource plans and reports remaining application dependencies separately. Existing provider-specific uninstall/association workflows stay authoritative. The default screen shows item names, effects, counts, Cask consequences, and compact review/approval/execution actions and a hash-bound visual Reviewed acknowledgement. Public builds show Review only before interaction, rather than implying that execution is available. Gate-disabled actions remain visible and explained. Approval does not execute. Execution requires a plain confirmation, not typing UUIDs. Advanced details hold paths, IDs, hashes, expiry, reasons, and recovery evidence. After successful cleanup, Remove Cask is offered only when the shared eligibility query passes; preserved dependencies require further review.

The footer contains only Add Items and red Cleanup; Cleanup only navigates. Confirmed empty-Cask removal is offered inside Cleanup. Deletion-method radios are visible in the main screen; changing method or scope clears the preview and local review state. Permanent execution requires an extra irreversible-deletion confirmation. The Cleanup section has the sole plan-creation action. Settings retains Archive but loses Storage Location. Generic Copy/Move resource intake is temporarily disabled at the shared user-command boundary and hidden in both interfaces. Preserve legacy intake, recovery, provider artifact copying, and managed data cleanup; Reference and explicit Manage at current location remain available. Restoring Copy/Move later requires changing the documented user policy and interface choices.

## Schema and acceptance

Schema 13 adds removal tombstones without relocating or purging existing data. Test confirmation/revision rejection, true emptiness across resources and applications, unfinished work, non-restoration/recovery, post-cleanup removal, user intake policy, navigation without plan creation, visible gated controls, plain execution confirmation, and advanced-only technical details. Independent destructive review remains required.
