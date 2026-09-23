# Provider capability model

Status: Accepted

## Context

Portable ZIPs, WinGet, MSI, and arbitrary EXEs offer different guarantees.

## Decision

Providers advertise explicit capabilities and accept typed requests. Provider identity and verification evidence govern eligible actions.

## Consequences

Unsupported behavior remains unavailable; no provider string becomes a shell command.

## Alternatives considered

A universal installer interface and heuristic capability inference were rejected.
