# Casklet Project Context

Last updated: 2026-09-30

This is the canonical handoff for humans and new Codex tasks. Keep it concise and current. Detailed specifications belong in linked documents.

## Product vision

Casklet is a lightweight, local-first tool that lets users create a workspace for an activity, associate its software and files, understand what it contains and how much storage it uses, export its inventory, and safely clean it up as one unit.

Core principle:

> Casklet manages the lifecycle and ownership of resources, not necessarily their physical location.

## Current phase

The initial design program is approved. Milestones 0–10 are integrated into `develop`. The desktop GUI redesign is prepared on `ui` for integration into `develop`; manual Windows release gates remain open. Milestone 4 removal and Milestone 8 uninstall remain behind the disabled `reviewed-removal` feature and require independent security review before destructive functionality is enabled in public builds.

- Phase 1 — Requirements: approved.
- Phase 2 — Cross-platform and installation workflow: approved.
- Phase 3 — Domain model and state transitions: approved and normalized into `docs/architecture/domain-model.md`; `domain model.txt` is retained temporarily as the source record for review.
- Phase 4 — Component architecture and API boundaries: approved.
- Phase 5 — Persistence, schema, manifests, and recovery: approved.
- Phase 6 — Security threat model: approved.
- Phase 7 — Repository structure and engineering toolchain: approved.
- System design synthesis: approved as the living visual map of the v0.1 design.
- Phase 8 — Delivery and implementation roadmap: approved on 2026-09-15.

## Approved v0.1 scope

- Windows 11 x64 first.
- Cross-platform domain and application boundaries; Windows adapters implemented first.
- Rust core and CLI.
- Tauri 2 desktop shell with a Svelte/TypeScript UI.
- SQLite operational database plus per-Cask JSON recovery/export manifests.
- Managed, moved, and referenced files/directories.
- Built-in and custom categories.
- Storage and reclaimable-space insights.
- Portable application support.
- WinGet application search/install/inspect/uninstall.
- Existing-application association without implied ownership.
- Installer artifact registration and cautious MSI/EXE correlation.
- Durable operations, removal previews, partial-failure recovery.
- GUI drag-and-drop and equivalent explicit CLI commands.
- JSON inventory export and human-readable report.

## Explicit v0.1 non-goals

- Isolation, containers, VMs, filesystem virtualization, or Registry virtualization.
- Guaranteed cleanup of arbitrary installers.
- Whole-system file/Registry tracing.
- Drivers and services management.
- Browser integration.
- Game launcher integration.
- Cloud accounts, synchronization, collaboration, or telemetry by default.
- Public third-party plugin API.
- Linux or macOS release.

## Safety invariants

1. Referencing is not ownership.
2. Association is not installation ownership.
3. Ambiguity means preservation.
4. Every destructive action has an inspectable plan.
5. Plans are revalidated immediately before destructive steps.
6. External paths are never recursively deleted implicitly.
7. Interrupted operations are detectable and recoverable.
8. CLI and GUI use the same core behavior.
9. Privilege elevation is visible and operation-specific.
10. Logical size, physical size, and reclaimable size are reported separately.

## Core domain model

```text
Cask <-- many-to-many --> Resource
Cask <-- many-to-many --> Installation --> Application
InstallerArtifact --> zero or more Installations
Operation --> durable lifecycle work and recovery
RemovalPlan --> approved and revalidated cleanup intent
```

Important distinctions:

- Scope: CASK, USER, or SYSTEM.
- Management: MANAGED, REFERENCED, or DISCOVERED.
- Containment: INTERNAL or EXTERNAL.
- Provenance records how Casklet learned about or created an item.
- Availability records whether an item is present, missing, changed, inaccessible, or unknown.
- Sharing is derived from associations, not stored as an authoritative flag.
- Category belongs to the Cask-resource association because one resource can serve different roles in different Casks.

Only an installation created by Casklet, still verified, supported by its provider, unneeded by other associations, and approved in a removal plan may qualify for automatic uninstall.

## Cross-platform strategy

Design platform-neutral contracts from the start but ship Windows first. Platform behavior is supplied by adapters for installed-application inventory, providers, filesystem semantics, trash, process execution, signatures, privileges, paths, and application launching.

Installation formats are not treated as equivalent. WinGet, MSI, arbitrary EXE, macOS app/DMG/PKG, Linux AppImage/deb/rpm, Flatpak, and future providers advertise explicit capabilities.

## Chosen architectural direction

- Local-first modular monolith.
- Hexagonal/ports-and-adapters boundaries without excessive ceremony.
- Domain layer contains business rules and no infrastructure dependencies.
- Application layer orchestrates commands, queries, transactions, operations, and policies.
- Infrastructure adapters implement persistence, filesystem, providers, process execution, manifests, and platform behavior.
- CLI calls application services directly.
- Tauri commands adapt typed GUI requests to the same services.
- Long work returns an operation ID and reports progress through a common event interface.
- No local HTTP server or always-running daemon in v0.1.

## Tentative technology direction

These choices are not yet implementation authorization:

- Stable Rust; Cargo workspace.
- `clap` for CLI.
- Tauri 2.
- Svelte 5, TypeScript, and Vite.
- SQLite through a Rust persistence adapter.
- Tokio for cancellable asynchronous work.
- Serde for versioned data contracts.
- `tracing` for structured local diagnostics.

Exact dependencies will be approved during repository/tooling design; do not add them merely because they appear here.

## Documentation map

- `AGENTS.md` — automatically loaded project instructions and safety rules.
- `docs/PROJECT_CONTEXT.md` — current status and durable handoff.
- `docs/architecture/component-architecture.md` — Phase 4 component and API-boundary design.
- `docs/architecture/domain-model.md` — normalized Phase 3 domain language, aggregates, states, and invariants.
- `docs/architecture/persistence-design.md` — Phase 5 SQLite, manifest, migration, consistency, and recovery design.
- `docs/security/threat-model-v0.1.md` — Phase 6 assets, trust boundaries, threats, mitigations, and security release gate.
- `docs/engineering/repository-and-tooling.md` — Phase 7 repository layout, dependencies, tool versions, CI/CD, and workflow.
- `docs/architecture/system-design-v0.1.md` — approved consolidated system design with context, component, data-flow, ER, trust-boundary, and sequence diagrams.
- `docs/engineering/delivery-roadmap-v0.1.md` — Phase 8 vertical slices, acceptance gates, learning sequence, and release progression.
- `docs/engineering/milestone-5-export-recovery.md` — export profiles, CLI and GUI workflows, compatibility, and conservative recovery boundary.
- `docs/engineering/milestone-6-portable-apps.md` — ZIP validation, installation identity, launch verification, and removal integration.
- `docs/engineering/milestone-7-windows-inventory.md` — Windows registration discovery, reconciliation, and detach-only association.
- `docs/engineering/milestone-8-winget-lifecycle.md` — WinGet provider, exact ownership evidence, journal, and uninstall plan.
- `docs/engineering/milestone-9-installer-artifacts.md` — MSI/EXE inspection, reviewed launch, candidate correlation, and acceptance evidence.
- `docs/engineering/milestone-10-release-gates.md` — build traceability, automated evidence, and pending manual release checks.
- `docs/product/user-guide.md` — user workflows, safety, privacy, and troubleshooting.
- `docs/security/milestone-10-review.md` — internal boundary audit and outstanding independent security review.
- `docs/adr/0016-apache-2-project-license.md` — Apache-2.0 project-license decision and release consequences.
- `docs/adr/0017-windows-install-and-data-paths.md` — separation of per-user NSIS program files and persistent data.
- `domain model.txt` — original detailed Phase 3 source; preserve through normalization review.

## Current implementation milestone

Milestones 0–10 are integrated into `develop`. Milestone 10 adds release-profile storage separation, database integrity checks and pre-migration backup, initial accessibility improvements, packaging tooling, and product/release documentation. Its release gate remains open for Windows manual testing, installer uninstall evidence, performance profiling, and independent security review. Real installer/UAC behavior still needs isolated manual validation. Keep `main` unchanged until planned milestone integration.
The project license is Apache-2.0 (selected during Milestone 10). CLI and GUI expose metadata-only SQLite backup through the shared application core. The GUI now uses fixed native main and owned Cask windows, monochrome styling, the supplied logo, bundled Inter, and Lucide icons. Cask workspaces retain all resource/provider/recovery/removal workflows across Overview, Files & Folders, Applications, Activity, and Settings. Schema 11 adds descriptions and revision-checked metadata/archive commands shared by CLI and GUI. See `docs/adr/0018-desktop-cask-workspaces.md` and `docs/engineering/desktop-gui-design.md`. The Windows host has no isolated test machine or independent reviewer, so release gates remain open.
The per-user NSIS installer uses `%LOCALAPPDATA%\Casklet` for program files. A local install test exposed that the original release database path collided with this directory; the empty test install was uninstalled and operational state moved to `%LOCALAPPDATA%\CaskletData`. The corrected package passed a local silent install/create/uninstall preservation test; Windows Settings, upgrade, and clean-machine tests remain pending.
