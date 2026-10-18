# Casklet Project Context

Last updated: 2026-10-18

This is the canonical handoff for humans and new Codex tasks. Keep it concise and current. Detailed specifications belong in linked documents.

## Product vision

Casklet is a lightweight, local-first tool that lets users create a workspace for an activity, associate its software and files, understand what it contains and how much storage it uses, export its inventory, and safely clean it up as one unit.

Core principle:

> Casklet manages the lifecycle and ownership of resources, not necessarily their physical location.

## Current phase

The initial design program is approved. Milestones 0â€“10 are integrated into `develop`. The desktop GUI redesign was merged in PR #14. A follow-up UI polish pass adds consistent menus, focus states, floating notifications, resource/application picker hierarchy, and hidden-window readiness. Manual Windows release gates remain open. Default builds remain review-only. The user-authorized Alpha profile explicitly enables file cleanup through `alpha-cleanup`; WinGet uninstall remains behind `reviewed-removal`. Independent review and clean-machine/publication gates remain open. See ADR 0021.

- Phase 1 â€” Requirements: approved.
- Phase 2 â€” Cross-platform and installation workflow: approved.
- Phase 3 â€” Domain model and state transitions: approved and normalized into `docs/architecture/domain-model.md`; `domain model.txt` is retained temporarily as the source record for review.
- Phase 4 â€” Component architecture and API boundaries: approved.
- Phase 5 â€” Persistence, schema, manifests, and recovery: approved.
- Phase 6 â€” Security threat model: approved.
- Phase 7 â€” Repository structure and engineering toolchain: approved.
- System design synthesis: approved as the living visual map of the v0.1 design.
- Phase 8 â€” Delivery and implementation roadmap: approved on 2026-09-15.

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

- `AGENTS.md` â€” automatically loaded project instructions and safety rules.
- `docs/PROJECT_CONTEXT.md` â€” current status and durable handoff.
- `docs/architecture/component-architecture.md` â€” Phase 4 component and API-boundary design.
- `docs/architecture/domain-model.md` â€” normalized Phase 3 domain language, aggregates, states, and invariants.
- `docs/architecture/persistence-design.md` â€” Phase 5 SQLite, manifest, migration, consistency, and recovery design.
- `docs/security/threat-model-v0.1.md` â€” Phase 6 assets, trust boundaries, threats, mitigations, and security release gate.
- `docs/engineering/repository-and-tooling.md` â€” Phase 7 repository layout, dependencies, tool versions, CI/CD, and workflow.
- `docs/architecture/system-design-v0.1.md` â€” approved consolidated system design with context, component, data-flow, ER, trust-boundary, and sequence diagrams.
- `docs/engineering/delivery-roadmap-v0.1.md` â€” Phase 8 vertical slices, acceptance gates, learning sequence, and release progression.
- `docs/engineering/milestone-5-export-recovery.md` â€” export profiles, CLI and GUI workflows, compatibility, and conservative recovery boundary.
- `docs/engineering/milestone-6-portable-apps.md` â€” ZIP validation, installation identity, launch verification, and removal integration.
- `docs/engineering/milestone-7-windows-inventory.md` â€” Windows registration discovery, reconciliation, and detach-only association.
- `docs/engineering/milestone-8-winget-lifecycle.md` â€” WinGet provider, exact ownership evidence, journal, and uninstall plan.
- `docs/engineering/milestone-9-installer-artifacts.md` â€” MSI/EXE inspection, reviewed launch, candidate correlation, and acceptance evidence.
- `docs/engineering/milestone-10-release-gates.md` â€” build traceability, automated evidence, and pending manual release checks.
- `docs/product/user-guide.md` â€” user workflows, safety, privacy, and troubleshooting.
- `docs/security/milestone-10-review.md` â€” internal boundary audit and outstanding independent security review.
- `docs/adr/0016-apache-2-project-license.md` â€” Apache-2.0 project-license decision and release consequences.
- `docs/adr/0017-windows-install-and-data-paths.md` â€” separation of per-user NSIS program files and persistent data.
- `docs/adr/0019-resource-ownership-and-cleanup.md` â€” explicit ownership at external locations and separation of Cask lifecycle from cleanup.
- `docs/engineering/ownership-cleanup-ux.md` â€” shared commands, interface behavior, schema 12 migration, and acceptance evidence.
- `domain model.txt` â€” original detailed Phase 3 source; preserve through normalization review.

## Current implementation milestone

Milestones 0â€“10 are integrated into `develop`. Milestone 10 adds release-profile storage separation, database integrity checks and pre-migration backup, initial accessibility improvements, packaging tooling, and product/release documentation. Its release gate remains open for Windows manual testing, installer uninstall evidence, performance profiling, and independent security review. Real installer/UAC behavior still needs isolated manual validation. Keep `main` unchanged until planned milestone integration.
The project license is Apache-2.0 (selected during Milestone 10). CLI and GUI expose metadata-only SQLite backup through the shared application core. The GUI now uses fixed native main and owned Cask windows, monochrome styling, the supplied logo, bundled Inter, and Lucide icons. Cask workspaces retain all resource/provider/recovery/removal workflows across Overview, Files & Folders, Applications, Activity, and Settings. Schema 11 adds descriptions and revision-checked metadata/archive commands shared by CLI and GUI. See `docs/adr/0018-desktop-cask-workspaces.md` and `docs/engineering/desktop-gui-design.md`. The Windows host has no isolated test machine or independent reviewer, so release gates remain open.
The per-user NSIS installer uses `%LOCALAPPDATA%\Casklet` for program files. A local install test exposed that the original release database path collided with this directory; the empty test install was uninstalled and operational state moved to `%LOCALAPPDATA%\CaskletData`. The corrected package passed a local silent install/create/uninstall preservation test; Windows Settings, upgrade, and clean-machine tests remain pending.
The October ownership/cleanup follow-up adds direct reversible archive/restore, explicit in-place resource ownership grants, stable reuse of equivalent current cleanup plans, and resource-only cleanup that preserves the Cask. Reference and explicit in-place ownership remain available; generic user Copy/Move intake is temporarily disabled under ADR 0020 while legacy/provider code and recovery remain supported. Schema 12 preserves existing resource data and invalidates unexecuted legacy plans. External owned resources require exact grant and identity verification, block on overlapping dependencies, and support journaled Recycle Bin disposal only behind `reviewed-removal`. Recovered manifests never recreate deletion authority.

The second October cleanup review introduces schema 13 permanent Cask removal with retained audit tombstones after atomic dependency/operation checks, a simple visible review/approval/execution workflow, and advanced-only cleanup diagnostics. Archive stays in Settings; bottom Cleanup only navigates. Generic user Copy/Move intake is temporarily disabled while legacy/provider implementation and recovery remain intact. See `docs/adr/0020-permanent-cask-removal-and-simple-cleanup.md`.

The latest cleanup refinement uses only Add Items / red Cleanup footer actions, with confirmed empty-Cask removal inside Cleanup. Resources stay collapsed after intake and expand inline for paths/technical details. Cleanup has visible deletion-method choices, compact Reviewed state, and an upfront review-only capability notice. Method/scope changes invalidate the displayed preview; security gating and shared-core approval/execution semantics remain unchanged.

The Alpha readiness pass versions the candidate as 0.1.0-alpha.1, separates file-cleanup capability from provider uninstall, adds shared cleanup history/restart recovery, reconciles worker failures with durable plan state, and extracts cleanup presentation. Migration sequencing is centralized; cleanup scans use blocking workers; GUI startup reports data errors; recovery process checks use system executable resolution. The integration xtask now runs real suites. Patched Vitest/devalue dependencies pass the full frontend audit. The complete local CI gate passed across default/Alpha/reviewed profiles; real Windows fixture cleanup and the release 1,000-resource benchmark passed. Packaging records build capabilities and exact source checksums, including explicitly dirty local candidates. See docs/engineering/alpha-readiness.md for evidence and outstanding release limitations. No public release or independent review is claimed.

The 0.1.0-alpha.1 local package has been built and verified (nine artifacts, SPDX SBOM, checksums, explicit dirty-source snapshot). The optimized CLI passed real fixture cleanup; desktop startup and local silent install/reinstall/uninstall preservation passed. Expired saved plans require a new preview. The candidate remains unsigned and is not cleared for broad public distribution.

The next Alpha follow-up (ADR 0022) adds explicit, verified new MSI ownership and journaled uninstall planning in the Alpha profile; WinGet remains gated and arbitrary EXE results stay association-only. Schema 14 preserves legacy ownership semantics and records storage-retirement outcomes. Confirmed empty-Cask removal retires audit manifests outside active Cask storage and removes only verified empty directories, preserving unexpected/recovery bytes with a warning. Modal confirmation, contained cleanup history, persisted Activity display clearing, compact installer/Overview details, console suppression, hidden row actions, and immediate library cache invalidation complete the related UI changes. See docs/engineering/alpha-cleanup-followup.md. Real MSI/UAC and public-distribution review gates remain open; the rebuilt Windows Alpha candidate passed nine-artifact verification and isolated release fixture cleanup/storage retirement. The full local pipeline passed 96 Rust and 24 GUI tests plus audits; hosted CI and real MSI/UAC testing are not claimed.

The desktop entry point now explicitly uses the Windows GUI subsystem in all Windows profiles, preventing startup console allocation/attachment and console-close termination. NSIS shortcuts continue to target the desktop executable directly; the CLI retains its console subsystem. Rebuild/reinstall the Alpha package to apply this executable-level fix.

ADR 0023 simplifies application installation to background inspection followed by one Install action through shared GUI/CLI orchestration. Verified new MSI applications gain ownership automatically; successfully tracked matching EXE results gain references only, with automatic uninstall still unsupported. Windows handles requested elevation; EXE process-tree waiting replaces parent-only success. Diagnostics and unusual outcomes remain collapsed, and ambiguous/failed/reboot results never gain ownership. No schema migration is required.

The October 5 installer-flow quality gate passed 101 Rust and 25 GUI tests, including native fake-bootstrapper descendant waiting and immediate GUI application refresh. Real third-party MSI/EXE and UAC validation remain pending; see ADR 0023 for the evidence and unsupported EXE ownership boundary.

The Applications library refinement removes installer history/technical disclosures, introduces a pinned Add Application/search/filter toolbar and focused workflows above the list, and deduplicates provider presentation by stable identity. Installer journals and CLI inspection remain intact; unfinished installs restore a plain-language attention state with confirmed preserved closure. No ownership or cleanup gate changes. See docs/engineering/applications-library-ux.md for design and validation.

ADR 0024 supersedes the EXE reference-only presentation in ADR 0023: a successful, process-tree-tracked install with one current matching new registration now records durable installed-for-Cask provenance automatically, separately from provider uninstall authority. Existing/manual associations remain references. Schema 15 preserves legacy references; detach releases install attribution, and manifests retain informational provenance without recreating cleanup authority. GUI/CLI expose explicit installer-only UAC elevation for MSI and EXE. Typed installer copies are excluded from Files & Folders and shared library counts while staying eligible for separately planned file cleanup. Ambiguous detection remains unresolved; arbitrary EXE automatic uninstall and real UAC validation remain pending. See docs/adr/0024-installer-elevation-and-installation-provenance.md.

The October 6 installer pass passed `cargo xtask check`: 102 all-feature Rust tests (one opt-in benchmark ignored), default/Alpha profile suites, 29 GUI tests, strict Clippy, Rust/frontend formatting, ESLint, Svelte checks (zero errors/warnings), generated-contract verification, and production frontend build. Evidence: `target/installer-elevation-quality-verified.log`. The Windows Alpha packaging run uses `target/installer-elevation-package.log`; real installer/UAC behavior still needs manual validation.

ADR 0025 extends the existing MSI ownership/journal system into the MSI/EXE application lifecycle. Verified new process-tree-completed EXE installs now register persistent ownership atomically and appear as Casklet-owned in Applications; manual associations remain references. Schema 16 stores immutable uninstall-registration evidence and executable digests, preserving older references and MSI evidence. Shared-core single-application previews and mixed application cleanup detach references first, then uninstall supported owned applications sequentially. Completed failures retain ownership and continue; uncertain failures stop the queue. Durable GUI results and reviewed retry previews are available. Whole-Cask removal remains blocked by unresolved owners. Safe direct registered EXE uninstall and typed MSI mechanisms are supported; ambiguous/unsupported results, legacy informational provenance, portable/WinGet provider gates and real third-party/UAC validation limits are documented in docs/adr/0025-application-ownership-and-uninstall-queue.md.

The application lifecycle quality gate passed `cargo xtask check`: 107 all-feature Rust tests (one opt-in benchmark ignored), default/Alpha profile suites, 29 GUI tests, strict Clippy, formatting, frontend lint/type checks, contract verification and production builds. Final scoped-plan and preservation refinements also passed the 93-test infrastructure suite and strict workspace Clippy. Evidence: `target/application-lifecycle-quality.log`. No real third-party application was installed/uninstalled; real MSI/EXE/UAC and self-deleting uninstaller tests remain pending.

The October 6 develop integration retains the Windows GUI subsystem fix from 126663b (alpha-readiness itself had a console entry point). The previously reported installed build was already uninstalled, so its historical launch failure cannot be reproduced or assigned a confirmed cause. The retained local package passed actual installed launches, console-subsystem checks, reinstall/uninstall and data preservation. A committed installed-desktop smoke test now checks launch instead of only installer exit codes; CI push gates include develop. See docs/engineering/develop-integration.md for branch and verification evidence.

Local develop now contains remote develop f277847 plus the preserved application lifecycle changes and installed-launch guard, without conflicts. Its full local quality gate, bootstrap, frozen dependency install, secret scan and dependency policy passed. The source-map-js advisory was patched with pinned 1.2.2; the frontend audit is clear. The final clean-source Alpha package and installed/fixture verification use target/develop-package.log, target/develop-installed-smoke.log and target/develop-alpha-smoke.log. Historical failure of the already-uninstalled build remains unconfirmed; current console suppression is verified at the PE subsystem level.

Native develop validation found shared HKCU registry-view aliases that made single per-user MSI/EXE installs ambiguous. ADR 0025 now handles exact aliases through correlation, ownership/dependency revalidation and uninstall verification while retaining stable IDs and prior authority. The updated full local gate passed 117 Rust and 29 GUI tests; native registry fixture cleanup and strict Clippy passed. The first hosted run exposed a 15-second success-test deadline on its cold Windows runner; the completion budget is now 60 seconds, with timeout/cancellation and production behavior unchanged. Normal installed GUI/shortcuts and schema-15 to 16 data preservation were verified. Final package/CI evidence is tracked in docs/engineering/develop-integration.md and target/develop-\*-final logs.

The October 7 storage follow-up derives registered owned/reference application bytes from active SQLite associations and current Windows presence. Typed installer copies are excluded from the displayed total; complete folder scans and labelled Windows estimates remain independent of uninstall authority. Reference association/detach refresh storage immediately. The supplied cask-icon.png supplies only Windows package/desktop icons; the original in-app logo is restored byte-for-byte from 37ccf0b. See docs/engineering/application-storage.md; validation is in target/storage-quality.log.

The October 7 appearance pass adds a saved global Light/Dark preference, synchronized Cask workspaces, supplied black/white in-app logos, semantic charcoal palettes, and theme-aware native startup. Light colors remain the component defaults; theme state stays separate from Cask data and safety policy. See docs/adr/0027-desktop-appearance.md.

The October 7 installed-launch investigation reproduced the obsolete September 28 0.0.0 setup exiting with code 101 against schema-16 data; the existing user database has that schema. Current packages retain native startup error reporting, reject newer schemas before journal-mode changes, disable future installer downgrades, and provide verified local setup promotion to replace stale paths. The full gate passed 118 Rust and 37 GUI tests. The repaired exact setup path passed installed launches/reinstall/uninstall preservation; normal per-user install and both shortcuts launched with existing metadata preserved. See docs/engineering/installed-launch-repair.md for reproduction and installed verification evidence.

The October 10 cancellation fix excludes typed installer copies from main-library storage as well as workspace storage and counts only active registered application sizes. Cask list cards include references consistently with workspace totals. Unconfirmed installation presentation now uses one short message without Keep controls; uncertain backend journals/locks remain intact. Successful/cancelled installation regression evidence is in target/cancellation-quality.log.

October 10 Activity follow-up: Activity is now main-window only. Completed display history recognizes core SUCCEEDED/closed states and remains a persisted display filter over durable journals. Main history includes installer outcomes with Cask names and concise descriptions. Confirmed installer cancellation (Windows 1602 after process completion and a complete post-inventory) is terminal preservation, releases its lock, and has no attention action; timeouts, killed/unverified child processes and incomplete inventory retain recovery safeguards. Keyboard focus uses theme tokens across controls. Native validation exposed invalid dotted lifecycle events; producers/listeners now use valid names and matching progress payloads. Full local quality gates and 41 GUI tests pass; light/dark Activity and Cask layout were inspected with isolated native data. See docs/engineering/activity-followup.md.

October 10 drag feedback: individual Cask windows now highlight their existing native drop target with a themed border and compact centered card containing a folder icon, named instruction and review guidance. Drag leave/drop/Escape/focus loss clear the indication; one-item explicit intake and ownership consent are unchanged. Scope is individual Cask windows only; the main-list drop behavior remains unchanged. See docs/engineering/desktop-gui-design.md.

The shared desktop shell suppresses the native WebView context menu across main and Cask windows. Explicit application menus and keyboard focus behavior remain available.

October 11 installer recovery fixes verified reinstall ownership collisions on stable Windows IDs: only retired owners can be replaced after full verification. Cask windows retry saved successful confirmation without rerunning installers, and completed review no longer disables installation selection. Uncertain running/recovery operations retain safety locks. See docs/engineering/installer-reinstallation.md.

October 11 removal-plan follow-up: non-destructive PRESERVE/DETACH steps retain recorded metadata identity for database revalidation, while destructive steps still require independently verified identity. The shared core and CLI expose explicit preservation-only detachment plans (cleanup create --detach-only), with normal hash-bound approval and journaling, for permanent Cask removal that retains unverifiable files. An isolated changed-file fixture passed; real-test was removed through reviewed cleanup, preservation-only detachment and the core Cask removal command after metadata backup.

October 11 category cleanup integrates application queues into the existing scope selector and Cleanup Review. Whole Cask executes sequential owned-app uninstalls/reference detachment before file cleanup; Applications excludes other categories, and other categories exclude native application queues. Existing component plans, journals, verification, recovery and execution gates remain authoritative. See docs/adr/0028-category-application-cleanup.md.

October 11 PR #21 CI correction: preservation/detachment plans now snapshot recorded database identity, while destructive actions retain independently verified identity. The omitted local dependency caused two clean-checkout approval tests to fail; the isolated fix passed all 108 infrastructure tests, application-core tests, formatting and strict Clippy.

October 12 desktop follow-up: Cask windows use independent native top-level windows to avoid owner minimization, with explicit main-close destruction preserving the close lifecycle. Cleanup Review defaults to a compact collapsible action-count summary; workflow buttons and confirmations remain outside, with existing review/approval/execution guards unchanged. See docs/adr/0029-independent-cask-windows.md and docs/engineering/desktop-gui-design.md. Native multi-window regression checks remain manual.

October 12 folder synchronization/cleanup: metadata reconciliation updates current folder sizes and bounded inherited content listings through the shared core, with incremental directory-list caching, desktop polling and periodic full reconciliation. New folder grants separate stable root identity from current cleanup trees. Schema 17 binds original authority separately from plan snapshots and invalidates pending old plans; changed legacy folders require explicit consent renewal. Failed requested removal now produces FAILED/PARTIAL, and successful disposal verifies source/quarantine absence. Planned preservation is explicitly described as retained data. See docs/adr/0030-current-folder-contents-and-cleanup.md and docs/engineering/ownership-cleanup-ux.md.

Folder synchronization validation passed the full local gate (132 all-feature Rust tests, one opt-in benchmark ignored; default/Alpha profiles; 53 GUI tests; strict lint/format/types/contracts and builds), plus final frontend presentation checks. Real Windows owned/reference, locked-file, changed-plan and restart fixtures passed. Evidence: target/folder-sync-quality.log; refreshed local Alpha packaging uses target/folder-sync-alpha-package.log. Existing independent review/publication gates remain open.

October 18 PR #22 CI correction: windows-quality passed, but dependency-policy rejected zip 8.6.0 for RUSTSEC-2026-0367, RUSTSEC-2026-0368 and RUSTSEC-2026-0369. The infrastructure dependency and lockfile now pin patched zip 9.0.0. Portable ZIP inspection/extraction propagate its fallible filename decoding through the existing path validator; a malformed-name fixture verifies rejection before staging. The dependency policy and all three focused ZIP tests pass without advisory exceptions or policy changes. Full local gate evidence uses target/pr22-zip9-quality.log; audit evidence uses target/pr22-dependency-policy-fixed.log.
