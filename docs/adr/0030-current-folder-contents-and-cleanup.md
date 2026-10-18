# ADR 0030: Current folder contents and cleanup identity

Status: Accepted by the October 12 dynamic folder ownership and synchronization request.

## Root causes

Resource queries refreshed presence only, retaining intake-time sizes. Directory cleanup authority and immutable plan identity both used the original complete tree fingerprint. Later legitimate contents therefore made an owned folder ineligible. A planned PRESERVE was successfully journaled, but the GUI described the result as finished without explaining retained data. A destructive item that failed execution revalidation was also marked PRESERVED, allowing the run to become COMPLETE even though its requested action was not performed.

## Decision

Ownership covers a registered directory object and its ordinary current descendants. New tree-v2 evidence stores the Windows volume/file ID/creation time separately from the complete tree-v1 digest. The root must still be the same object at the exact registered location. An unchanged file remains subject to its existing identity/content rules. No reference, recovered manifest, path match or size observation grants deletion authority.

Schema 17 adds original authority evidence to each plan item separately from its current immutable tree snapshot. Both are hash-bound; database revalidation compares original authority while filesystem revalidation compares the exact reviewed snapshot. Existing pending approvals are invalidated on migration. Historical journal identities/hashes remain compatible.

Each fresh folder plan snapshots all current contents, with independent full content verification at planning, approval, execution, before quarantine and after quarantine. Changes after review require a new plan. Links/reparse points, hard-linked descendants, unsafe roots, incomplete/oversized scans, and overlapping associations remain preserved. The adapter confirms disappearance at both original and quarantine paths after disposal. Failed execution revalidation is FAILED, producing PARTIAL and retaining the recovery lock and association. Planned preservation and reference detachment are valid successful outcomes, clearly described as keeping data. Cask removal remains separately confirmed.

Legacy tree-v1 evidence has no recoverable root identity. Unchanged legacy trees still verify, but changed ones require explicit ownership renewal. Renewal reuses the original resource ID, requires the existing confirmation, complete safe inspection, an owned external directory/grant, no overlap and no unfinished work. It replaces evidence and increments the revision without moving bytes or promoting references.

## Synchronization strategy

Choose reconciliation on queries/open/focus plus incremental metadata polling while the desktop runs. Native filesystem subscriptions are deferred: they would add platform-specific lifetimes, inaccessible-folder handling and notification-overflow recovery, and would still require authoritative reconciliation. This approach fits the existing shared-core query ports and the desktop's polling lifecycle, adds no daemon/dependency/unsafe OS code, and cannot permanently lose an event.

The adapter caches directory child listings, re-enumerating changed directory timestamps. It stats known files for size changes without reading file contents. A cold adapter reconciles from disk; complete metadata re-enumeration at least once per 60-second active reconciliation cycle recovers unchanged/missed timestamps. Partial scans retry. Repeated summary/list queries share a cache, with a 1–30-second interval based on scan cost; windows request refresh every three seconds and main library every five. Explicit folder inspection bypasses that interval. Idle large folders still require metadata traversal at the bounded interval; this is not an event-driven constant-time watcher. All GUI scans run on blocking workers outside SQLite transactions.

Observed size/presence/unknown counts update SQLite and queued portable projections without changing ownership evidence or safety revisions. Children are a read-only projection inheriting the parent association, not additional independent resource/grant records. Bounded scans retain the existing 100,000-entry/64-level limit; UI contents show at most 200 entries with an explicit truncation message, while storage includes every inspected file. Partial or inaccessible data is labelled unknown rather than silently counted as complete. Thirty-two cached roots bound retention; eviction causes safe reconciliation.

## Verification

Real Windows fixtures cover nested additions/modifications/deletions, owned/reference inheritance, persisted sizes and restart, large bounded listings, replaced roots, new hard links, unchanged legacy evidence and explicit renewal, actual Recycle Bin disposal, reference-only detachment, post-approval changes, locked files, and recovery/new reviewed retry. Cache fixtures cover changed files and reconciliation after a simulated missed directory timestamp. GUI fixtures cover inherited content, focus refresh and renewed consent. No real user resources or third-party applications are removed by these fixtures.

Final local validation passed `cargo xtask check`: 132 all-feature Rust tests (one opt-in benchmark ignored), default/Alpha profile suites, strict Clippy, formatting, frontend lint, generated-contract verification, zero Svelte errors/warnings, 53 GUI tests and desktop/CLI/frontend builds. The final cleanup status presentation also passed all frontend checks. Evidence: target/folder-sync-quality.log. Real owned/reference Windows fixtures verified original/quarantine disappearance or preservation, locked-file failure and reviewed retry. Alpha packaging evidence uses target/folder-sync-alpha-package.log; existing third-party/UAC, independent review and publication gates remain open.
