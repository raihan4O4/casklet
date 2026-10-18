# Ownership, Cask lifecycle, and cleanup UX

Status: implemented under ADR 0019; destructive execution remains behind independent security review.

## Product review and recommendations

1. Empty Cask: permanent Remove is a confirmed shared command. It checks all resource/application associations and unfinished operations atomically, retires the Cask from normal queries, and prevents restore or stale-manifest recovery with a durable tombstone. Archive remains a separate reversible Settings option.
2. Cleanup: generated plans retain independent review, approval, and execution. Default UI shows names, counts, effects, and the Cask consequence. IDs, paths, expiry, hash, evidence, and recovery locations are under Advanced details. Disabled gate controls stay visible. Equivalent plans are reused; bottom Cleanup only navigates. After successful cleanup, a separate confirmed Remove is offered if eligibility passes; remaining apps use their provider-specific workflow.
3. Storage: Reference and Manage at current location remain available. Generic user Copy/Move intake is temporarily disabled at the shared command boundary under ADR 0020, and hidden in GUI/CLI. Preserve physical intake implementation for legacy recovery and existing provider artifact registration. No existing data is relocated or granted new authority.

## Shared model and commands

`SetCaskArchived` invokes the existing revision-checked metadata use case. SQLite rejects archive/restore while intake, removal, or installer work holds the Cask. Resources and installed applications remain associated.

`AdoptResource` requires `confirmOwnership`, an active Cask, available category, canonical absolute local path, complete inspection, and stable identity. It records `MANAGED / EXTERNAL / OWNED / USER_ADOPTED` and a separate durable grant containing exact path, identity, Cask ID, and consent timestamp. Registration is a short database transaction and does not write into the selected resource. Its operation history records successful registration. The directory inspector and ownership validator retain bounded traversal. Profile roots, common broad user folders, system/application/state locations, Casklet storage, quarantine, links, hard-linked data, incomplete scans, and overlapping registered resources are rejected.

Known dependencies include active associations using the same resource ID, and independently registered resources at the same or overlapping paths. Adding a reference later does not transfer ownership, but it blocks deletion. Unregistered dependencies of other applications cannot be determined automatically; explicit consent states that this data is dedicated to this Cask and is not needed elsewhere. If that statement cannot be made, use Reference.

Plans verify current identity and full directory contents. Missing, offline, changed, inaccessible, recovered, or ungranted objects are preserved. External owned data supports Recycle Bin disposal only; a permanent plan preserves it. Execution rechecks exact grant, identity, scope, revisions, sharing, and mutation locks. Quarantine is a unique sibling on the original volume and has a deterministic recovery location in the durable run. Concurrent intake cannot move or register data overlapping a running or uncertain cleanup, including recovery quarantine.

## Interfaces

GUI: Archive in Settings; Add Items and red navigation-only Cleanup in the footer; confirmed Remove inside Cleanup; compact cleanup review; ownership consent beside Manage at current location; collapsed resource rows with inline expandable location, authority, availability, size, and category; human action/reason descriptions; plan details secondary; explicit execution confirmation; an upfront Review only notice and disabled-action explanations when execution is unavailable. Deletion method is visible outside Advanced details. Changing method or scope clears the current preview and its local review acknowledgement; it never alters an existing plan. A hash-bound Reviewed badge acknowledges inspection without changing core approval state. Approved permanent plans require both cleanup and irreversible-deletion confirmations. Short disclosure motion respects reduced-motion preferences.

CLI:

```text
casklet cask archive <id>
casklet cask restore <id>
casklet cask removal-status <id>
casklet cask remove <id> --yes
casklet resource add <id> <path> --mode reference
casklet resource adopt <id> <path> --confirm-ownership
casklet cleanup create <id> [--resource <id>] [--category <key>]
casklet cleanup get <plan-id>
casklet cleanup approve <plan-id> --hash <hash>
casklet cleanup execute <plan-id> --hash <hash> --yes
```

`plan` remains a compatibility alias for `cleanup`. Approval and execution commands remain available only in reviewed-removal builds. JSON uses stable action/state codes; human output describes effects and the next step. Empty cleanup scopes direct users to confirmed Cask removal; eligibility includes applications and unfinished work.

## Migration, portability, and backup

Schema 13 adds `cask_removals`, an irreversible lifecycle tombstone separate from Archive. History and metadata directories remain for audit/recovery; resource bytes are never deleted by Remove. Normal get/list/library and restore exclude removed Casks. Recovery refuses an existing tombstoned ID. Pending projection jobs are cancelled and pending plans invalidated. A database backup includes removal records; recovering into a new database without those records is an explicit conservative recovery action, not ordinary Restore.

Schema 12 preserves existing files, paths, copy/move intents, references, associations, and history. No existing reference is promoted to ownership. New grant rows arise only from an explicit local adoption. Unexecuted legacy plans become invalid because whole-Cask cleanup no longer retires the Cask; historical complete/uncertain plans retain their original lifecycle flag and hash behavior.

The database remains the operational authority. Manifests describe external paths, management, relationship, and provenance, but cannot authorize deletion. Machine exports retain external absolute paths; portable exports mark external resources unresolved and omit machine paths. Recovery demotes imported resources to unverified references and never recreates a grant. Relative paths are used only for internal portable resources with a verified root. External locations do not silently rebase when drives or directories change. Metadata backup does not back up external or managed file bytes; users must back up those locations separately.

## Acceptance evidence

| Requirement                                                        | Automated evidence                                                                  |
| ------------------------------------------------------------------ | ----------------------------------------------------------------------------------- |
| Empty and populated Casks archive/restore without deleting data    | Shared-core lifecycle test and CLI process test; GUI Settings test                  |
| Explicit ownership without relocation, restart consistency         | Consent, path/bytes, grant persistence, and CLI adoption tests                      |
| No deletion when a plan is created; stable repeated plan ID        | Core and CLI equivalent-plan tests; GUI concurrent-submission test                  |
| Clear review and explicit execution confirmation                   | GUI action/gate test and review/approval/plain-confirmation test                    |
| Referenced/shared/overlapping resources preserved                  | Domain policy combinations and overlap integration test                             |
| Changed, missing, hard-linked, broad, or unverified data preserved | Identity-change, missing-path, hard-link, and protected-location tests              |
| Cross-Cask intake and cleanup cannot overlap                       | Active copy progress test rejects cleanup startup; quarantine test rejects archive  |
| External directory quarantine and interruption recoverability      | Simulated handoff interruption test; correct sibling recovery path and archive lock |
| Existing schema and pending-plan migration                         | Schema 1–12 migration suite and legacy-plan invalidation test                       |
| External manifests preserve information but never authority        | Export and manifest-recovery integration test                                       |

Real Windows Recycle Bin handoff races and independent security review remain release gates. These changes do not enable destructive functionality in public builds.

## Second-review acceptance evidence

- Core: confirmation and stale revision; permanent removal across restart; restore and manifest recovery refusal; file/application/unfinished-work blockers; post-reference-cleanup removal; user Copy/Move rejection with legacy managed data eligibility retained.
- GUI: navigation does not create a plan; duplicate submissions blocked; review precedes approval; plain execution confirmation; gate-disabled controls remain visible; advanced-only IDs and paths; application-only Casks cannot be removed; Settings retains Archive and omits Storage Location; Copy/Move choices absent.
- CLI: permanent removal requires `--yes`, fails on remaining dependencies, works after successful cleanup, and cannot restore; Copy/Move parser rejects requests without moving bytes.
- Schema 1–12 migration tests preserve data and generate consistent pre-upgrade backups.

## Dynamic directory contents (October 12)

Owned folders include ordinary contents added later; references and all their descendants remain externally managed. New folder identity evidence separates a stable Windows root from each reviewed cleanup tree. Root replacement, links, overlapping registrations and incomplete scans remain ineligible. Changed legacy folders require Renew folder ownership with fresh explicit consent. Schema 17 separates recorded authority from current cleanup snapshots and invalidates pending old approvals. See ADR 0030.

Shared-core resource queries reconcile current metadata and persist size/availability to SQLite and projections. Desktop polling, open/focus refresh and periodic full metadata reconciliation recover external and offline changes. Unchanged directory listings are reused; routine queries do not hash contents. Expanded folder details show up to 200 current descendants with inherited status and a truncation notice. Full destructive verification remains independent of this display cache.

Requested removals that fail revalidation are FAILED/PARTIAL, preserving associations and recovery controls. Successful disposal requires both source and quarantine disappearance. A completed plan containing planned preservation explicitly states that retained data remains on disk; reference cleanup removes association only. Empty-Cask removal stays separately confirmed.
