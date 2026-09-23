# Plan before destruction

Status: Accepted

## Context

Deleting unrelated data is Casklet's highest-impact failure mode.

## Decision

All deletion and uninstall work requires an immutable inspectable plan, explicit approval, and immediate target revalidation.

## Consequences

Ambiguous, referenced, shared, broad, changed, or unverified targets are preserved by default.

## Alternatives considered

Direct delete commands and confirmation-only safety were rejected.
