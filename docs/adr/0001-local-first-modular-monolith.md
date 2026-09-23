# Local-first modular monolith

Status: Accepted

## Context
Casklet coordinates local data and lifecycle work without needing a hosted service.

## Decision
Use one local-first modular monolith with explicit domain, application, adapter, CLI, and desktop boundaries.

## Consequences
CLI and GUI share behavior and local state. Module boundaries require discipline but avoid distributed failure modes.

## Alternatives considered
Cloud backend, local HTTP service, and microservices were rejected for v0.1.
