# ADR 0019 — Cask lifecycle, cleanup, and ownership in place

Status: Accepted for implementation on 2026-10-02 under the user's request.

Archive presentation and user Copy/Move availability are superseded by ADR 0020. Ownership, cleanup eligibility, and recovery semantics remain applicable.

## Decision

Archive and restore are reversible metadata commands for any Cask, including an empty Cask. Archive preserves associations, managed files, installed applications, manifests, and history; it never deletes a directory. Expose these commands directly in both interfaces. Do not introduce hard deletion of Cask metadata in this change. Cleanup plans describe resource actions and never implicitly archive or destroy the Cask. Application uninstall remains a separate provider plan with its existing eligibility rules.

Reference means association without deletion authority. Manage in place means explicit user delegation of cleanup authority over one inspected file or directory at its current absolute location. Copy and move remain physical intake operations into Casklet storage, with their existing journal and recovery behavior. Ownership and containment are independent: managed resources can be internal or external.

An in-place ownership grant records stable resource and Cask IDs, exact normalized absolute path, observed identity, consent time, and provenance. Granting ownership changes metadata only. It requires explicit consent in GUI and CLI and rejects broad/system/profile/application-state roots, links, incomplete scans, unrecordable identities, and overlapping registered resources. Files inside an adopted directory are covered by its inspected tree, never by name or category. An outside application’s dependence cannot be discovered reliably; consent must explicitly state the selected data is dedicated to this activity and not required elsewhere. Known overlapping associations, including a reference to a child or parent, block deletion.

Every cleanup plan is immutable, inspectable, and expires after fifteen minutes. Repeated creation of an equivalent current snapshot reuses the existing draft/approved plan in the shared repository. Changed identities, scope, authority, or associations require a new draft. Plan creation is read-only with respect to files and associations. The UI describes every action, reason, next step, and whether execution is disabled by the security release gate.

In-place cleanup defaults to Recycle Bin. Permanent deletion is not supported for external owned data. Immediately before execution, validate grant, exact path, identity/tree, ownership, overlapping resources, and the mutation lock. Quarantine uses a unique sibling path on the resource's volume so cross-volume relocation is unnecessary; its deterministic path is retained in the journal and shown for uncertain recovery. No broad recursive deletion API is added. The existing destructive feature gate remains in force.

## Compatibility and recovery

Schema 12 adds grant evidence and a cleanup-only flag. Existing copy/move/reference data, paths, identities, and operations remain unchanged. Old pending plans are invalidated because whole-Cask retirement semantics changed; historical completed/uncertain runs remain inspectable. New cleanup preserves Cask lifecycle state.

Manifests include management, containment, relationship, and provenance as descriptive evidence. Machine exports retain external absolute locations; portable exports mark them as unresolved external locations. Recovery never restores a deletion grant from a manifest: recovered resources remain unverified and detach-only until separately reviewed. A database backup protects metadata, not external file content. Offline or replaced objects are preserved; no path fallback or automatic ownership promotion occurs.

## Acceptance tests

- Archive and restore empty and populated Casks without changing resources, files, or application associations; stale revision/active mutation blocks the command.
- Whole-Cask cleanup leaves the Cask active. Empty scopes give a useful archive/restore instruction; GUI never starts a resource plan for an empty Cask.
- Identical plan requests reuse the same current draft/approval in GUI and CLI; relevant changes create a new draft. Creating plans never mutates files or associations.
- In-place adoption leaves bytes and path unchanged, records consent and identity, survives restart, and is available through shared commands.
- Reference, overlap/shared, changed, missing, offline, broad, linked, partial-scan, recovered, and ungranted resources never become deletable.
- External eligible resources use journaled Recycle Bin disposal only; permanent plans preserve them. Quarantine interruption exposes the correct sibling recovery path.
- Schema 1–11 migration retains resources and data and invalidates only pending legacy plans. Manifests describe external ownership but cannot recreate authority.
- GUI presents action explanations and next steps, blocks duplicate concurrent submissions, and exposes archive/restore without a cleanup plan.
