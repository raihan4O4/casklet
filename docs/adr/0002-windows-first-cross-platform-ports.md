# Windows-first cross-platform ports

Status: Accepted

## Context

v0.1 targets Windows 11 x64, while core policies should not depend on Win32 details.

## Decision

Keep domain/application contracts platform-neutral and implement Windows adapters first behind capability-based ports.

## Consequences

Platform differences remain explicit without speculative non-Windows adapters.

## Alternatives considered

Windows APIs in the core and simultaneous multi-platform delivery were rejected.
