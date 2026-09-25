# ADR 0011: Preserve managed resources without a recorded identity

Status: accepted for the first Milestone 4 slice (2026-09-25).

## Context

Milestones 1–3 recorded paths, sizes, and association metadata, but did not persist a stable filesystem identity for each managed object after intake. A path can be replaced by a different object with the same name and size. Current inspection alone cannot prove that it is the object Casklet originally managed.

## Decision

The removal planner records immutable previews but proposes `PRESERVE` for a managed item unless a recorded identity matches the current observation. The Milestone 4 migration adds a nullable identity field; existing records remain null. Newly staged managed files record Windows volume, file index, creation time, and content digest after the staged object is verified at its final location. Recovery records this identity only when it can match the staged object to that final location; an already present final path remains unverified. No approval or execution endpoint is exposed in this slice. A recorded identity makes an item eligible for a `TRASH` proposal, but does not itself authorize execution. Platform-specific, immediate revalidation and an independent security review remain required before execution is exposed.

The domain policy also requires explicit management, containment, provenance, relationship, removal policy, no other association, presence, a narrow path, and a complete observation. A plan hash binds preview content to its snapshot for future approval checks, but is not a signature or a substitute for live revalidation.

## Consequences

Existing managed content will show as preserved in previews. Users can inspect the reason, and no cleanup action can accidentally treat old path metadata as deletion authority. Later work must define a strong Windows identity representation, how it is captured at registration, and how legacy records may be safely reverified without silently trusting their paths.
