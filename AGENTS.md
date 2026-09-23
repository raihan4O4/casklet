# Casklet Project Instructions

## Project purpose

Casklet is a local-first lifecycle and resource manager. A Cask groups files, directories, applications, installations, and related metadata around one activity so users can inspect, export, and safely clean up that activity.

## Required context

Before planning or changing this repository:

1. Read `docs/PROJECT_CONTEXT.md`.
2. Read the documents it marks as required for the current phase or module.
3. Check the working tree and preserve unrelated user changes.

When a task changes an approved decision, milestone, current status, or important unresolved question, update `docs/PROJECT_CONTEXT.md` in the same change. Put detailed design in the appropriate document under `docs/` and keep the context file concise.

## Current boundaries

- Current target: Casklet v0.1 on Windows 11 x64.
- Design cross-platform domain boundaries now; implement Windows adapters first.
- Casklet v0.1 is managed, not isolated.
- No cloud service, account, daemon, public plugin API, or arbitrary installer tracing in the initial release.
- Both CLI and GUI must use the same Rust application core.
- Never infer deletion ownership merely from association or detection.
- Referenced, shared, ambiguous, or unverified resources are preserved by default.
- Every destructive action must use a generated, inspectable, revalidated removal plan and durable operation journal.
- Do not begin implementation until the design phases in `docs/PROJECT_CONTEXT.md` are approved.

## Architecture rules

- Keep the domain and application layers independent of Tauri, SQLite, WinGet, and OS APIs.
- Put platform and provider behavior behind capability-based interfaces.
- Keep installer artifacts, logical applications, installed instances, and Cask associations distinct.
- Use stable IDs; names and paths are not identities.
- Treat SQLite as the operational source of truth and per-Cask manifests as recoverable, portable projections.
- Keep database transactions short; never hold one open while waiting for a user or external installer.
- Use typed commands, queries, DTOs, errors, and operation progress events.
- Prefer a modular monolith and avoid premature services, plugins, or generalized abstractions.

## Engineering expectations once implementation begins

- Explain material technical decisions in plain language for a developer learning Rust/Tauri.
- Use Architecture Decision Records for consequential choices.
- Add tests for domain invariants and all destructive behavior.
- Run formatting, linting, relevant tests, and builds for changed components.
- Never execute arbitrary shell command strings derived from user input.
- Never recursively delete an unresolved, broad, referenced, or insufficiently verified path.
