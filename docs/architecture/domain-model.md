# Casklet domain model

Status: Approved (normalized from `domain model.txt` on 2026-09-15)

This document preserves the Phase 3 conceptual model independently of database and UI representations. Stable UUIDs identify all important entities; names and paths never serve as identity.

## Aggregates and associations

```text
Cask <-- ResourceAssociation --> Resource
Cask <-- CaskInstallation ----> Installation --> Application
InstallerArtifact ------------> zero or more Installations
Cask --> Operations
Cask --> immutable RemovalPlans --> RemovalPlanItems
```

Associations are first-class because resources and installations may serve multiple Casks. Category, role, relationship, and removal policy belong to an association, not the shared resource.

## Cask

A Cask has an ID, editable name, stable storage slug, description, lifecycle state, managed-storage root, timestamps, and optimistic-concurrency revision. States are `CREATING`, `ACTIVE`, `ARCHIVED`, `DESTROYING`, `PARTIALLY_DESTROYED`, `DESTROYED`, and `RECOVERY_REQUIRED`.

Names are non-empty but need not be globally unique. Storage paths are unique and do not move merely because a Cask is renamed. Archived Casks are read-only; destroyed Casks accept nothing new. Only one destructive operation may act on a Cask at once, and unresolved owned resources prevent a transition to `DESTROYED`. Permanent empty-Cask removal retires it from user queries and restoration using a durable tombstone after atomic resource, application, and unfinished-operation checks. Historical journals and metadata directories remain; no resource bytes are deleted. Archive is reversible. See ADR 0020.

## Resources

A Resource represents a file-like object and records kind, display name, physical location, containment, management, provenance, availability, size evidence, content identity, source, timestamps, and revision.

- Containment is `INTERNAL` only when current path evidence places the object inside a verified managed boundary; otherwise it is `EXTERNAL`.
- Management is `MANAGED`, `REFERENCED`, or `DISCOVERED`.
- Availability distinguishes pending, present, missing, changed, inaccessible, and unknown.
- Size records bytes, measurement quality (`EXACT`, `CALCULATED`, `ESTIMATED`, or `UNKNOWN`), and observation time.

A ResourceAssociation relates one Cask to one Resource with `OWNED`, `REFERENCED`, or `DISCOVERED` relationship and one of `DELETE_WHEN_UNREFERENCED`, `DETACH_ONLY`, `ALWAYS_RETAIN`, or `ASK`. A removal policy is eligibility evidence, never permission to bypass safety checks.

Ownership is independent of containment. Explicit in-place adoption records a local grant over one inspected file or bounded dedicated directory, without relocation (`MANAGED / EXTERNAL / OWNED`). It requires exact path and identity evidence, user consent, and no overlapping registered dependency. Copy and move remain implemented intake operations into managed storage, but generic user intake is temporarily disabled under ADR 0020. References and recovered manifests never grant deletion authority. See ADR 0019.

## Categories

Categories have stable IDs and keys, display metadata, order, and optional Cask scope. Built-ins use stable keys; custom keys contain a stable UUID. Built-ins cannot be deleted, and custom categories must be emptied or reassigned before deletion.

## Applications, installations, and artifacts

Application is the logical product. Installation is a platform-specific instance with provider identity, version, scope, locations, uninstall reference, provenance, state, confidence, verification time, and revision. Display names never identify installations.

CaskInstallation expresses whether an instance was installed for, adopted by, referenced by, or merely discovered near a Cask operation. Automatic uninstall can only be considered when all of the following remain true:

```text
relationship == INSTALLED_FOR_CASK
and provenance == INSTALLED_BY_CASKLET
and no retaining association exists
and the provider supports uninstall
and current identity is verified
and the user approved a current removal plan
```

An InstallerArtifact is a Resource with format, platform, signature/signer evidence, product hints, hash, and inspection confidence. One artifact can yield zero, one, or many installations. Signatures and correlation confidence are evidence, not ownership or safety certification.

## Durable operations

Operations journal long-running or externally visible work with stable ID, optional Cask, type, state, typed request, progress, current step, timestamps, safe error details, cancellation flag, and revision. Ordered OperationSteps record kind, state, attempts, input/result, and timestamps.

An uncertain interruption becomes `RECOVERY_REQUIRED` or `UNKNOWN`, never assumed success. Database transactions remain short; filesystem and provider work is journaled and reconciled around them.

## Removal plans

A RemovalPlan is an immutable proposal based on observed state and a specific Cask revision. Each item identifies a target, proposed action, reason, risk, expected size, preconditions, and state. Plans expire or invalidate when relevant state changes.

New cleanup plans affect resources only and preserve Cask lifecycle state. Archive/restore are separate reversible metadata commands. Equivalent current snapshots reuse a draft or approved plan. Historical legacy destruction plans retain their original semantics for audit and uncertain-run closure.

Immediately before every destructive step, Casklet revalidates identity, path boundaries, associations, provider state, plan integrity, and current revision. Referenced, shared, broad, changed, inaccessible, ambiguous, or unverified targets are retained or blocked by default. No deletion or uninstall entry point bypasses this planner/executor.

## Storage accounting

Storage reports managed internal, referenced external, shared, application-estimated, minimum/maximum reclaimable bytes, and unknown/inaccessible counts separately. Referenced data is never reclaimable. Shared data is reclaimable only if every retaining association is included. Estimates contribute only to the upper bound.

## Domain services and events

Cross-entity rules live in focused services: removal planning, storage calculation, classification, installation correlation, and reconciliation. Correlation proposes evidence; it cannot silently grant ownership.

Local domain events describe completed facts such as Cask creation, resource association, operation failure, removal-plan creation, destruction progress, and required manifest projection. Casklet is not event-sourced; only recovery/audit facts that require durability are persisted.

## Commands and queries

Commands change state and queries only read it. This is a lightweight separation within one application and database, not distributed CQRS. CLI and GUI invoke the same typed application services and receive purpose-built DTOs rather than serialized domain objects.

## Non-negotiable invariants

1. Referencing is not ownership.
2. Association is not installation ownership.
3. Ambiguity means preservation.
4. Sharing is derived from current associations.
5. Paths are located attributes, not identities.
6. Every destructive action originates in an inspectable plan.
7. Plans and targets are revalidated immediately before action.
8. Interrupted external work remains detectable and recoverable.
9. Imported manifests and recovered metadata cannot authorize deletion by themselves.
10. Logical, physical, and reclaimable storage remain distinct.

The original `domain model.txt` remains as the Phase 3 source record during Milestone 0 and can be archived only after this normalization is reviewed.
