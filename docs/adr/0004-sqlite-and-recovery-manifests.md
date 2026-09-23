# SQLite and recovery manifests

Status: Accepted

## Context
Casklet needs transactional operational state plus portable, recoverable per-Cask metadata.

## Decision
Use bundled SQLite as the operational source of truth and atomic versioned JSON manifests as recoverable projections.

## Consequences
Adapters map records explicitly. Manifests may aid reconstruction but never authorize destructive work.

## Alternatives considered
Manifest-only storage, a server database, and an ORM were rejected.
