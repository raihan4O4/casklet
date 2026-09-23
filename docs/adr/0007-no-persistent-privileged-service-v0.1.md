# No persistent privileged service in v0.1

Status: Accepted

## Context

A broad elevated helper would create a durable confused-deputy attack surface.

## Decision

Run CLI and GUI unelevated and use visible, operation-specific OS elevation only where a provider requires it.

## Consequences

There is no generic elevated command API or lasting privileged state.

## Alternatives considered

An always-elevated application and persistent Windows service were rejected.
