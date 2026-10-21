# ADR 0037: Metadata-only Cask loading and explicit measurement

Status: accepted by the user's performance request, 2026-10-21.

## Problem

Library aggregation called live resource reconciliation for every Cask, and application-size aggregation walked installation directories. Resource reconciliation enumerated directory trees, with a cache that still visited directories after cold start. Desktop startup awaited these queries; three-second polling and focus repeated them. Additionally, plain reference registration shared deep inspection with Copy/Move/ownership, including Windows content hashing. Moving the same scans to a background startup task would retain unnecessary I/O and contention.

## Decision

Ordinary library/workspace loading, focus and polling read SQLite projections only. Shared application methods expose `list_resource_metadata` and `storage_summary_metadata`; `cask_library` aggregates saved resource and application observations. They never stat referenced paths, enumerate descendants, hash content, reconcile registry inventory, write manifests, or calculate new sizes. Work scales with the number of stored records, not their bytes or descendant counts. The configured storage-root string is presentation metadata.

Keep existing live application methods for explicit CLI inspection and safe policy operations. Desktop `list_resources` and `storage_summary` select metadata queries by default; their optional `refresh: true` deliberately requests current inspection on a blocking worker. Overview's **Refresh sizes** is asynchronous, guarded against duplicate requests, and does not lock navigation. Expanded folder inspection remains available with **Refresh folder contents**; polling/focus never re-enumerate it. Sizes and status are saved observations, potentially stale or unknown, rather than authoritative current facts.

Schema 19 adds presentation-only `application_size_snapshots` with source key, registration evidence digest, nullable size, estimate flag and observation time. Explicit inventory/association captures available Windows EstimatedSize metadata. Explicit size measurement saves new observations. Reads accept snapshots only for the current PRESENT registration and matching evidence; missing or changed evidence never gains a fresh-looking size. Writers carry the evidence they measured and cannot attach stale worker results to a changed registration. These rows never confer ownership or authorize cleanup. Existing resource measurements already persist in SQLite. Migration uses the existing pre-upgrade backup mechanism.

Reference intake uses a required shallow adapter capability: path validation, canonicalization, ordinary type, file length and native object handle, without file-content reads or descendant enumeration. A new directory reference starts with unknown size. It receives no content identity or deletion grant. Ownership adoption, renewal, copy/move verification and all destructive plan/revalidation/journal behavior retain their stronger inspection. Explicit application size scans use metadata measurement rather than content hashing.

## Consequences and validation

Large, offline and unavailable referenced content no longer delays opening its metadata. External changes are observed when explicitly inspected or measured; the UI explains saved values. Unknown values remain unknown until useful evidence is available. Loading is still proportional to Cask/resource/application counts and database/host latency; no universal constant-time startup promise is made.

An integration adapter panics on every stat, inspection, measurement, application scan, allocation, move or projection. The saved library, resource and storage queries pass with it after database reopen, including an offline 1 TiB directory and 1 TiB application snapshot. Tests also reject stale evidence writes and verify explicit refreshing leaves navigation available. Ownership, replacement detection, cleanup verification, provider lifecycle and migration tests remain required.

See [performance evidence](../engineering/metadata-only-loading.md) for measured local results and limits. This supersedes earlier automatic folder/focus measurement behavior; it does not supersede removal or ownership safety decisions.
