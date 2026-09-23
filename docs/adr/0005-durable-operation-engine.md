# Durable operation engine

Status: Accepted

## Context

Filesystem and provider actions outlive short database transactions and can be interrupted.

## Decision

Journal typed operations and ordered steps durably, with explicit uncertain and recovery-required states.

## Consequences

Recovery and progress are inspectable and idempotent; external work is never hidden inside a long transaction.

## Alternatives considered

In-memory jobs and long-lived database transactions were rejected.
