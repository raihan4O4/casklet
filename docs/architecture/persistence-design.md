# Phase 5 — Persistence, Schema, Manifests, and Recovery

Status: Approved on 2026-09-14

## 1. Purpose

This phase defines how Casklet persists its domain safely. It covers the SQLite database, per-Cask manifests, schema evolution, filesystem consistency, backup, corruption handling, and recovery from interrupted work.

This remains design work. Table definitions below are logical schemas, not migration code.

## 2. Persistence goals

Casklet persistence must provide:

- Correct ownership and association history.
- Durable operation and removal-plan state.
- Safe recovery after crashes, power loss, or terminated installers.
- Fast listing and filtering with at least 100 Casks and 100,000 indexed resources.
- Forward-compatible schema and manifest evolution.
- Human-supportable diagnostics.
- Local-only operation without a server.
- A path to future synchronization without redesigning the domain.

The most important limitation is that SQLite and the filesystem cannot participate in one atomic transaction. Casklet therefore uses durable intent, staged filesystem operations, verification, and reconciliation rather than pretending cross-system atomicity exists.

## 3. Storage layout

Application-owned state:

```text
%LocalAppData%\CaskletData\
├── db\
│   └── casklet.db
├── logs\
├── cache\
├── staging\
├── backups\
└── config.json
```

Default managed Cask storage:

```text
%UserProfile%\Casklets\<cask-storage-key>\
├── content\
│   ├── applications\
│   ├── projects\
│   ├── documents\
│   ├── images\
│   ├── videos\
│   ├── audio\
│   ├── archives\
│   ├── downloads\
│   └── other\
└── .casklet\
    ├── manifest.json
    ├── manifest.sha256
    └── staging\
```

Users may choose another managed root per Cask. The actual path is stored explicitly; it is never reconstructed only from the display name.

The database must remain on a local filesystem in v0.1. Network shares, cloud-synchronized database files, and removable media are unsupported database locations. Managed Cask content may live on other drives, subject to capability and safety checks.

## 4. Sources of truth

### SQLite

SQLite is the operational source of truth for:

- Current Cask and resource state.
- Cross-Cask associations and sharing.
- Application and installation identity.
- Operations, steps, locks, and recovery state.
- Removal plans and approvals.
- Projection work that remains pending.

### Per-Cask manifest

The manifest is a versioned, recoverable, portable projection of committed state. It is not independently edited during ordinary operation and is not the authority for concurrent mutations.

It supports:

- Inventory export.
- Human support and diagnostics.
- Rebuilding essential metadata if the global database is lost.
- Future import and reproducibility workflows.

### Filesystem and providers

The actual filesystem and installed-application providers are authoritative about present external reality. If they disagree with stored metadata, Casklet records drift rather than silently rewriting ownership.

The resulting model is:

```text
SQLite = intended and last-verified logical state
Manifest = portable projection of committed logical state
Filesystem/providers = currently observed external state
Reconciliation = explains and resolves disagreement
```

## 5. SQLite configuration

Recommended connection policy:

- Enable foreign keys for every connection.
- Use WAL journal mode when the database is on a supported local filesystem.
- Use `synchronous = FULL` initially because Casklet records destructive intent.
- Configure a finite busy timeout.
- Keep write transactions short.
- Use a small bounded connection pool or a serialized write connection; do not create unbounded connections.
- Run migrations before serving normal commands.
- Store timestamps as UTC Unix milliseconds in `INTEGER` columns.
- Store booleans as constrained integers.
- Store IDs as canonical lowercase UUID text for debuggability in v0.1.
- Use SQLite `STRICT` tables where library and supported SQLite versions permit.

WAL improves read/write coexistence but does not solve application-level concurrency. Casklet still uses revisions, operation records, and persistent locks.

## 6. General schema conventions

### IDs

Primary IDs are application-generated UUIDs stored as `TEXT`. They are opaque; clients must not infer creation time or ordering from them.

### Timestamps

All persisted timestamps use UTC Unix milliseconds:

```text
created_at_ms
updated_at_ms
finished_at_ms
```

Presentation layers convert them to local time.

### Revisions

Mutable aggregates carry a nonnegative `revision` integer. An update includes the expected revision and increments it on success. Updating zero rows indicates a concurrency conflict.

### Enums

Domain enums are stored as stable uppercase text codes with `CHECK` constraints where practical. Renaming a code requires a migration. UI labels are never persisted as enum values.

### JSON

JSON is reserved for:

- Versioned provider-specific metadata.
- Versioned operation request/result snapshots.
- Structured evidence and preconditions that vary by provider.
- Redacted diagnostic context.

Fields needed for filtering, joining, safety decisions, or uniqueness must be ordinary columns. JSON is not a substitute for schema design.

### Deletion policy

Core historical records use lifecycle states and tombstones rather than immediate row deletion. Temporary projections, expired plans, old diagnostic steps, and explicitly purged history may be physically deleted later.

## 7. Logical relational schema

### `casks`

```text
id                       TEXT PRIMARY KEY
name                     TEXT NOT NULL
storage_key              TEXT NOT NULL
description              TEXT
state                    TEXT NOT NULL
managed_storage_root     TEXT NOT NULL
managed_storage_key      TEXT NOT NULL
created_at_ms            INTEGER NOT NULL
updated_at_ms            INTEGER NOT NULL
archived_at_ms           INTEGER
destroyed_at_ms          INTEGER
revision                 INTEGER NOT NULL
```

Constraints:

- Non-empty trimmed name.
- Unique `storage_key`.
- Unique normalized `managed_storage_key` for active/non-destroyed Casks.
- State/timestamp combinations are validated by application policy and selected database checks.

`managed_storage_key` is a platform-produced comparison key. It helps detect duplicates but never substitutes for runtime path-safety validation.

### `categories`

```text
id                       TEXT PRIMARY KEY
cask_id                  TEXT NULL REFERENCES casks(id)
stable_key               TEXT NOT NULL
display_name             TEXT NOT NULL
icon_key                 TEXT NOT NULL
sort_order               INTEGER NOT NULL
is_builtin               INTEGER NOT NULL
created_at_ms            INTEGER NOT NULL
revision                 INTEGER NOT NULL
```

Rules:

- Built-in categories have `cask_id = NULL` and globally unique stable keys.
- Custom categories belong to exactly one Cask.
- Custom stable keys use generated identities, not normalized display names.
- Built-in categories cannot be deleted.

Partial unique indexes enforce uniqueness separately for global built-ins and per-Cask custom categories.

### `resources`

```text
id                       TEXT PRIMARY KEY
parent_resource_id       TEXT NULL REFERENCES resources(id)
kind                     TEXT NOT NULL
display_name             TEXT NOT NULL
management               TEXT NOT NULL
provenance               TEXT NOT NULL
availability             TEXT NOT NULL
size_bytes               INTEGER
size_measurement         TEXT NOT NULL
size_measured_at_ms      INTEGER
content_sha256           TEXT
media_type               TEXT
source_uri_redacted      TEXT
created_at_ms            INTEGER NOT NULL
observed_at_ms           INTEGER
modified_at_ms           INTEGER
revision                 INTEGER NOT NULL
```

Notes:

- `parent_resource_id` supports directory inventories.
- Negative sizes are prohibited.
- Source URLs stored in ordinary records must have credentials and sensitive query parameters removed.
- Hashing is optional and does not define identity by itself.
- Recursive hierarchy cycles are prevented by application rules and tested reconciliation queries.

### `resource_locations`

A separate table permits moves, multiple observed paths, and platform-specific identity evidence without changing Resource identity.

```text
id                       TEXT PRIMARY KEY
resource_id              TEXT NOT NULL REFERENCES resources(id)
platform                 TEXT NOT NULL
path_display             TEXT NOT NULL
path_key                 TEXT NOT NULL
location_kind            TEXT NOT NULL
containment              TEXT NOT NULL
volume_identity          TEXT
filesystem_identity      TEXT
is_primary               INTEGER NOT NULL
first_seen_at_ms         INTEGER NOT NULL
last_verified_at_ms      INTEGER
retired_at_ms            INTEGER
```

Rules:

- At most one active primary location per Resource.
- Retired locations preserve move history.
- `path_key` is used for candidate matching, not unconditional physical identity.
- Hard links and aliases mean path uniqueness cannot prove Resource uniqueness.
- Containment is re-derived during safety-critical work.

### `resource_associations`

```text
id                       TEXT PRIMARY KEY
cask_id                  TEXT NOT NULL REFERENCES casks(id)
resource_id              TEXT NOT NULL REFERENCES resources(id)
category_id              TEXT NOT NULL REFERENCES categories(id)
relationship             TEXT NOT NULL
role                     TEXT
removal_policy           TEXT NOT NULL
added_by_operation_id    TEXT REFERENCES operations(id)
added_at_ms              INTEGER NOT NULL
removed_at_ms            INTEGER
revision                 INTEGER NOT NULL
```

Rules:

- Only one active association between a given Cask and Resource.
- Category must be built-in or belong to the same Cask.
- Removed associations are retained for audit/recovery until explicit history purge.
- Sharing is calculated from active associations.

The category ownership rule requires application validation and may also be enforced by triggers only if those triggers remain simple and well tested. Complex business rules should not be hidden in triggers.

### `installer_artifacts`

```text
resource_id              TEXT PRIMARY KEY REFERENCES resources(id)
format                   TEXT NOT NULL
target_platform          TEXT
signature_status         TEXT NOT NULL
signer                   TEXT
detected_publisher       TEXT
product_hint             TEXT
inspection_confidence    TEXT NOT NULL
metadata_schema_version  INTEGER NOT NULL
metadata_json            TEXT NOT NULL
```

The underlying file remains a Resource. One artifact may later be linked to zero or more installation attempts through operations.

### `applications`

```text
id                       TEXT PRIMARY KEY
canonical_name           TEXT NOT NULL
publisher                TEXT
homepage                 TEXT
created_at_ms            INTEGER NOT NULL
updated_at_ms            INTEGER NOT NULL
revision                 INTEGER NOT NULL
```

Display name and publisher alone do not establish global uniqueness. Provider aliases are stored separately.

### `application_aliases`

```text
id                       TEXT PRIMARY KEY
application_id           TEXT NOT NULL REFERENCES applications(id)
platform                 TEXT NOT NULL
provider                 TEXT NOT NULL
provider_identifier      TEXT NOT NULL
created_at_ms            INTEGER NOT NULL
```

Unique key:

```text
platform + provider + provider_identifier
```

This allows Windows `VideoLAN.VLC`, a future Flatpak ID, and a Homebrew token to map deliberately to the same logical application.

### `installations`

```text
id                       TEXT PRIMARY KEY
application_id           TEXT NOT NULL REFERENCES applications(id)
platform                 TEXT NOT NULL
architecture             TEXT
provider                 TEXT NOT NULL
provider_identifier      TEXT
version                  TEXT
scope                    TEXT NOT NULL
provenance               TEXT NOT NULL
state                    TEXT NOT NULL
detection_confidence     TEXT NOT NULL
uninstall_reference_type TEXT
uninstall_reference      TEXT
installed_at_ms          INTEGER
last_verified_at_ms      INTEGER
provider_metadata_version INTEGER NOT NULL
provider_metadata_json   TEXT NOT NULL
revision                 INTEGER NOT NULL
```

`uninstall_reference` is never interpreted as a free-form shell command outside its owning provider. Imported manifests cannot directly populate an executable uninstall instruction.

### `installation_locations`

```text
id                       TEXT PRIMARY KEY
installation_id          TEXT NOT NULL REFERENCES installations(id)
path_display             TEXT NOT NULL
path_key                 TEXT NOT NULL
location_role            TEXT NOT NULL
estimated_size_bytes     INTEGER
last_verified_at_ms      INTEGER
```

One application may have executable, data, configuration, or cache locations. Estimates are not automatically considered fully reclaimable.

### `cask_installations`

```text
id                       TEXT PRIMARY KEY
cask_id                  TEXT NOT NULL REFERENCES casks(id)
installation_id          TEXT NOT NULL REFERENCES installations(id)
relationship             TEXT NOT NULL
removal_policy           TEXT NOT NULL
requested_by_operation_id TEXT REFERENCES operations(id)
created_at_ms            INTEGER NOT NULL
removed_at_ms            INTEGER
revision                 INTEGER NOT NULL
```

Only one active association exists for a Cask/Installation pair. Automatic uninstall eligibility is calculated from association, installation provenance, provider capability, verification, and approved plan state.

### `operations`

```text
id                       TEXT PRIMARY KEY
cask_id                  TEXT NULL REFERENCES casks(id)
type                     TEXT NOT NULL
state                    TEXT NOT NULL
requested_by             TEXT NOT NULL
idempotency_key          TEXT
request_schema_version   INTEGER NOT NULL
request_json             TEXT NOT NULL
result_schema_version    INTEGER
result_json              TEXT
progress_completed       INTEGER NOT NULL
progress_total           INTEGER
current_step_code        TEXT
cancellation_requested   INTEGER NOT NULL
error_code               TEXT
error_details_json       TEXT
started_at_ms            INTEGER
updated_at_ms            INTEGER NOT NULL
finished_at_ms           INTEGER
revision                 INTEGER NOT NULL
```

Rules:

- Idempotency keys are unique within operation type when present.
- Terminal states require `finished_at_ms`.
- Error details are structured and redacted.
- Request snapshots contain logical intent, not secrets or arbitrary executable commands.

### `operation_steps`

```text
id                       TEXT PRIMARY KEY
operation_id             TEXT NOT NULL REFERENCES operations(id)
sequence                 INTEGER NOT NULL
step_code                TEXT NOT NULL
state                    TEXT NOT NULL
attempt_count            INTEGER NOT NULL
input_schema_version     INTEGER NOT NULL
input_json               TEXT NOT NULL
result_schema_version    INTEGER
result_json              TEXT
started_at_ms            INTEGER
finished_at_ms           INTEGER
revision                 INTEGER NOT NULL
```

Unique key:

```text
operation_id + sequence
```

Inputs record the validated target and expected identity needed for recovery. They must not become a generic arbitrary-command store.

### `operation_locks`

```text
lock_key                 TEXT PRIMARY KEY
operation_id             TEXT NOT NULL REFERENCES operations(id)
owner_instance_id        TEXT NOT NULL
acquired_at_ms           INTEGER NOT NULL
heartbeat_at_ms          INTEGER NOT NULL
```

Example lock keys:

```text
cask:<cask-id>:mutation
cask:<cask-id>:destruction
platform:windows:installation-mutation
```

A stale heartbeat is evidence of interruption, not automatic permission to steal the lock. Casklet first verifies that the owner is gone and moves the associated operation to recovery handling.

### `removal_plans`

```text
id                       TEXT PRIMARY KEY
cask_id                  TEXT NOT NULL REFERENCES casks(id)
scope                    TEXT NOT NULL
state                    TEXT NOT NULL
based_on_cask_revision   INTEGER NOT NULL
plan_hash                TEXT NOT NULL
estimated_min_bytes      INTEGER NOT NULL
estimated_max_bytes      INTEGER NOT NULL
created_at_ms            INTEGER NOT NULL
expires_at_ms            INTEGER NOT NULL
approved_at_ms           INTEGER
executing_operation_id   TEXT REFERENCES operations(id)
revision                 INTEGER NOT NULL
```

Approval applies to one exact `plan_hash`. Changing items invalidates the approval.

### `removal_plan_items`

```text
id                       TEXT PRIMARY KEY
plan_id                  TEXT NOT NULL REFERENCES removal_plans(id)
sequence                 INTEGER NOT NULL
target_type              TEXT NOT NULL
target_id                TEXT NOT NULL
action                   TEXT NOT NULL
reason_code              TEXT NOT NULL
risk                     TEXT NOT NULL
expected_size_bytes      INTEGER
preconditions_version    INTEGER NOT NULL
preconditions_json       TEXT NOT NULL
target_snapshot_hash     TEXT NOT NULL
state                    TEXT NOT NULL
result_json              TEXT
```

The target foreign key is intentionally polymorphic at this conceptual level. Persistence design may later use separate nullable typed IDs if stronger relational enforcement outweighs schema width. The final migration must not accept an unconstrained target that cannot be validated.

### `projection_jobs`

```text
id                       TEXT PRIMARY KEY
projection_type          TEXT NOT NULL
aggregate_type           TEXT NOT NULL
aggregate_id             TEXT NOT NULL
target_revision          INTEGER NOT NULL
state                    TEXT NOT NULL
attempt_count            INTEGER NOT NULL
last_error_code          TEXT
created_at_ms            INTEGER NOT NULL
updated_at_ms            INTEGER NOT NULL
```

Unique active key:

```text
projection_type + aggregate_type + aggregate_id
```

Newer aggregate revisions supersede older pending manifest jobs.

### `schema_migrations`

```text
version                  INTEGER PRIMARY KEY
name                     TEXT NOT NULL
checksum                 TEXT NOT NULL
applied_at_ms            INTEGER NOT NULL
application_version      TEXT NOT NULL
```

Migration checksums detect accidental modification of already released migrations.

## 8. Index strategy

Required indexes include:

- Casks by state and updated time.
- Resources by parent, availability, kind, and observed time.
- Active resource associations by Cask/category.
- Active resource associations by Resource for sharing checks.
- Application aliases by platform/provider/provider ID.
- Installations by platform/provider/provider ID and state.
- Active Cask-installation associations in both directions.
- Operations by Cask/state/updated time.
- Operation steps by operation/sequence.
- Removal plans by Cask/state/creation time.
- Projection jobs by state/update time.

Do not add an index for every column. Query plans and realistic fixtures should guide additional indexes. Every index increases write and migration cost.

Resource listings use cursor/keyset pagination for large Casks. Page-number offset pagination may be offered for small UI lists but must not be the only mechanism.

## 9. Directory indexing policy

When a directory is added, Casklet creates a Resource for the root and may index descendants as child Resources.

v0.1 policy:

- The root is registered immediately after validation.
- Descendant scanning is a durable operation with progressive results.
- The UI can show the root before the scan completes.
- Each indexed child receives its own Resource and association so it can be inspected and selectively managed.
- The root records scan completeness and last scan time in versioned metadata.
- Removal planning collapses eligible descendants into a safe root action only when every relevant child is covered and the root identity is reverified.
- Links, junctions, and mount transitions are recorded but not followed by default.

Large scans use batched inserts and bounded memory. They do not hold one transaction for the entire directory tree.

## 10. Filesystem/database consistency patterns

### Copy into managed storage

```text
1. Persist operation and validated intent.
2. Reserve a destination and create an operation-specific staging path.
3. Persist the staging/final paths before copying.
4. Copy into staging while reporting progress.
5. Verify expected size and, when requested, hash.
6. Atomically rename within the destination filesystem where possible.
7. Persist Resource, location, association, and successful step.
8. Queue manifest projection.
9. Remove leftover staging data.
```

If the process crashes after step 6 but before step 7, recovery finds the operation-owned final path and either completes registration or offers safe cleanup.

### Move into managed storage

Same-volume moves may be atomic. Cross-volume moves are treated as:

```text
copy -> verify -> finalize destination -> remove source
```

The source is removed only after the destination has been verified and the operation has enough durable state to recover. Move is always explicit; drag-and-drop never silently selects it.

### Add a reference

```text
1. Inspect the source without modifying it.
2. Persist Resource/location or match an existing identity.
3. Persist a REFERENCED association.
4. Queue manifest projection.
```

No cleanup of the external object is implied.

### Delete or trash

```text
1. Load an approved plan item.
2. Revalidate its target identity and all preconditions.
3. Persist step RUNNING.
4. Commit the transaction.
5. Perform the exact filesystem action.
6. Persist observed outcome and association/resource state.
7. Queue manifest projection.
```

If outcome is uncertain, record `RECOVERY_REQUIRED`; do not guess success.

### Install/uninstall

Provider calls follow the same intent/external-work/verification/result sequence. Exit code alone does not prove final installation state.

## 11. Manifest design

Each manifest uses a stable envelope:

```text
schemaVersion
manifestId
generatedAt
generator
cask
categories
resources
applications
installations
associations
storageSummary
warnings
extensions
```

### Manifest requirements

- UTF-8 JSON.
- Deterministic ordering for meaningful diffs.
- Explicit schema version independent of application version.
- Stable enum codes and IDs.
- Relative paths for internal content where possible.
- Platform-tagged external paths.
- Redacted source URLs.
- Hash for the complete finalized manifest.
- Unknown extension fields preserved during compatible read/write cycles where feasible.

### Manifest exclusions

Do not export:

- Authentication tokens or cookies.
- Raw environment variables.
- Arbitrary uninstall commands.
- Unredacted signed URLs.
- Temporary staging paths.
- Internal lock ownership.
- Sensitive diagnostic details.

### Atomic projection

```text
write manifest.json.tmp
flush file
validate by reading it back
replace manifest.json atomically where supported
write/replace manifest.sha256
mark projection job complete
```

The old valid manifest remains available if generation fails before replacement.

## 12. Export versus recovery manifest

The on-disk recovery manifest and user export share a schema family but are not necessarily byte-identical.

- Recovery manifest may contain local IDs and platform paths needed for reconstruction.
- Portable inventory export defaults to privacy-safe paths and omits machine-specific internals.
- Reproducible recipes will later add provider mappings without executable commands.

This separation prevents a local recovery requirement from forcing private machine details into a shareable report.

## 13. Migration strategy

Database migrations are:

- Ordered.
- Immutable after release.
- Checksummed.
- Forward-only in production.
- Tested from every released schema to the current schema.
- Applied before ordinary application use.

Before a migration:

1. Verify database integrity sufficiently to avoid migrating known corruption.
2. Create a consistent backup through SQLite’s backup mechanism, not a naive copy of an open WAL database.
3. Confirm available disk space.
4. Apply transactional migrations where SQLite permits.
5. Run post-migration validation.
6. Preserve the backup until the upgraded application has opened successfully.

For large or destructive changes, use expand/migrate/contract:

```text
Release A: add new representation and write both where necessary
Migration: backfill and verify
Release B: stop reading old representation
Later release: remove old representation
```

Downgrading the executable against a newer database is rejected with an understandable message. Restore-from-backup is distinct from automatic down-migration.

Manifest migrations are pure transformations from one validated version to the next. Imported future major versions are rejected safely rather than partially interpreted.

## 14. Backup policy

v0.1 automatic backups:

- Before every database schema migration.
- Before explicit database repair or rebuild.
- Retain the most recent small set, initially three, subject to successful validation and user-visible storage reporting.

Optional manual command and GUI action:

```text
casklet diagnostics backup
```

A database backup does not contain managed user files. The UI and documentation must state this clearly.

Future full Cask archival is a separate feature.

## 15. Startup recovery and reconciliation

Startup performs bounded checks:

1. Open and configure SQLite.
2. Verify schema compatibility.
3. Apply approved migrations.
4. Find nonterminal operations and persistent locks.
5. Determine whether their owning processes still exist.
6. Mark abandoned work as recovery-required.
7. Resume pending manifest projections.
8. Perform lightweight validation of configured Cask roots.

It must not scan every file and installed application at every launch. Deep reconciliation is scheduled per Cask or requested by the user.

Recovery options depend on the operation:

```text
Resume
Retry verification
Complete registration
Clean operation-owned staging data
Accept current external state
Detach uncertain association
Keep everything and close operation with warning
```

Recovery never broadens deletion scope beyond the original validated intent.

## 16. Database loss and rebuild

If the database is missing or irreparably corrupt:

1. Preserve the damaged file for diagnostics.
2. Create a fresh database through normal migrations.
3. Discover configured/default Cask roots.
4. Validate manifest checksums and schema versions.
5. Import manifests as recovery candidates.
6. Rebuild Casks, Resources, Installations, and associations conservatively.
7. Mark external and ownership-sensitive state as requiring verification.
8. Recalculate sharing only after all available manifests are imported.

A manifest cannot prove that a system application is still installed or still safe to uninstall. Recovered installation ownership remains unverified until its provider confirms current identity.

## 17. Corruption and integrity policy

Casklet distinguishes:

- Database cannot be opened.
- Schema is newer than the application supports.
- Integrity check failed.
- Manifest checksum failed.
- Manifest schema unsupported.
- Resource hierarchy invalid.
- Association points to unavailable state.
- Filesystem/provider reality drifted from stored state.

The application should enter a limited diagnostic/recovery mode when core database integrity is uncertain. It must not execute destructive plans based on a database that failed integrity validation.

## 18. Privacy and sensitive data

- Store the minimum path and provider metadata necessary for local function.
- Redact credentials and sensitive query parameters before persisting URLs.
- Do not place secrets in operation request/result JSON.
- Diagnostic exports require an explicit preview and user action.
- Shared exports replace absolute user paths with relative or redacted forms by default.
- Database and manifests inherit current-user filesystem permissions.
- Optional database encryption is deferred unless the threat model identifies protected secrets that must be stored. Casklet v0.1 should avoid storing such secrets instead.

## 19. Scalability expectations

SQLite is appropriate for one local user and multiple Casklet processes with controlled write coordination.

Design targets:

- 100 or more Casks.
- 100,000 indexed resources.
- Incremental directory scanning.
- Keyset pagination.
- Batched writes.
- Cached summaries treated as rebuildable projections, not truth.
- No full object-graph loading.

Future cloud synchronization uses a separate remote model and synchronization adapter. It does not expose the local SQLite file or attempt multi-machine SQLite sharing.

## 20. Testing obligations created by this design

The persistence implementation must later test:

- Every foreign key and uniqueness rule.
- Enum/check constraints.
- Concurrent revision conflicts.
- CLI/GUI cross-process mutation conflicts.
- Interrupted copy before and after final rename.
- Interrupted cross-volume move.
- Database failure after filesystem success.
- Filesystem failure after durable intent.
- Stale operation-lock recovery.
- Manifest projection retry.
- Manifest checksum failure.
- Database rebuild from several sharing manifests.
- Migration from every released schema.
- Migration failure with backup preservation.
- Large directory pagination and batching.
- Symlink/junction changes between planning and execution.
- Removal-plan invalidation after association changes.

## 21. Deliberate deferrals

Not part of v0.1 persistence:

- Event sourcing.
- Cloud replication.
- CRDTs or distributed conflict resolution.
- Database encryption unless later security analysis requires it.
- Full content-addressed object storage.
- Automatic duplicate-file merging.
- User-editable manifests as a live configuration interface.
- Long-term audit-log retention guarantees.
- Multi-user machine authorization rules.

## 22. Decisions proposed for approval

1. Use one local SQLite database as operational truth.
2. Keep per-Cask manifests as versioned, recoverable projections.
3. Store current and historical paths in `resource_locations` rather than using paths as identity.
4. Preserve many-to-many Cask associations explicitly.
5. Persist operations, steps, locks, removal plans, and projection jobs.
6. Use durable intent plus staging and reconciliation for filesystem/database consistency.
7. Retain tombstones and association history until explicit purge.
8. Use forward-only, checksummed migrations with consistent pre-migration backups.
9. Separate local recovery manifests from privacy-safe shared exports.
10. Default uncertainty and corruption handling toward preservation.
11. Keep the database local and use a separate synchronization architecture if cloud features arrive.
12. Do not use event sourcing, a server database, or distributed infrastructure for v0.1.

## Completion gate

After approval, Phase 6 will create Casklet’s security threat model. It will identify assets, trust boundaries, attacker capabilities, destructive-operation threats, installer and provider risks, Tauri/WebView risks, supply-chain risks, mitigations, and release security requirements.
