# Phase 4 — Component Architecture and API Boundaries

Status: Approved on 2026-09-14

## 1. Purpose

This phase translates the approved domain model into modules with explicit responsibilities and dependency rules. It does not define database tables or contain implementation code.

Casklet will be a local-first modular monolith: one product and one repository with well-defined internal boundaries. This gives a solo developer simple deployment while preserving the seams needed for platform adapters, future providers, and optional isolation.

## 2. Top-level architecture

```text
                         Presentation
             +---------------+---------------+
             |                               |
          Rust CLI                    Tauri/Svelte GUI
             |                               |
             +---------------+---------------+
                             |
                    Application API
             commands / queries / operations
                             |
                       Domain Core
          entities / value objects / policies / plans
                             |
                    Ports (interfaces)
             +---------------+----------------+
             |               |                |
       Persistence      Platform services    Providers
       SQLite/manifest  filesystem/process   WinGet/portable
```

Dependency direction always points inward:

```text
Presentation -> Application -> Domain
Infrastructure -> Application/Domain ports
Domain -> nothing outside the domain
```

The domain does not import Tauri, SQLite, WinGet, operating-system APIs, or frontend types.

## 3. Major components

### Domain core

Owns business meaning and invariants:

- Cask lifecycle.
- Resource and installation identity.
- Resource and installation associations.
- Ownership, provenance, scope, containment, and availability.
- Removal eligibility and safety rules.
- Removal-plan structure and invalidation rules.
- Storage-accounting rules.
- Legal state transitions.
- Stable domain error categories.

The domain accepts already-inspected facts. It does not inspect files, query package managers, or perform deletion itself.

### Application layer

Coordinates use cases:

- Create, rename, archive, restore, and inspect Casks.
- Add, move, copy, reference, recategorize, and remove resources.
- Search, install, associate, verify, and uninstall applications.
- Create and execute removal plans.
- Export manifests and reports.
- Recover incomplete operations.
- Coordinate short database transactions around external work.
- Publish progress and domain events.

The application layer is the only supported entry point for state changes. Both CLI and GUI call it.

### Operation engine

Executes long-running and externally visible work as durable state machines.

Responsibilities:

- Create an operation record before external mutation.
- Run ordered, individually recorded steps.
- Report progress through a transport-neutral sink.
- Respect cancellation only at safe boundaries.
- Record partial success and recovery data.
- Resume, retry, or resolve interrupted operations.
- Apply idempotency keys where a repeated request might duplicate work.

The operation engine is part of the application layer, not a daemon or separate service in v0.1.

### Removal planner and executor

These are intentionally separate.

The planner reads current facts and generates an immutable proposal. It performs no deletion. The executor accepts only an approved plan, revalidates each item, performs allowed actions through ports, and records each outcome.

No presentation client may construct executable removal actions itself.

### Persistence adapter

Implements repositories and transaction boundaries using SQLite.

Responsibilities:

- Store domain state, associations, operations, plans, and schema version.
- Provide pagination and filtered queries without loading entire Casks.
- Enforce relational constraints and uniqueness.
- Support migrations and backups.
- Provide optimistic-concurrency checks.
- Avoid holding transactions across installers, downloads, or user confirmation.

SQL rows are persistence records, not domain or GUI objects.

### Manifest projection

Produces a versioned JSON representation inside each Cask.

Responsibilities:

- Generate manifests from committed application state.
- Record a pending projection if generation fails.
- Validate and migrate imported manifests.
- Provide enough information to recover the essential Cask inventory.
- Exclude unsafe executable instructions from trusted reconstruction.

The manifest is not written independently by the GUI or CLI.

### Filesystem adapter

Owns platform-aware file behavior:

- Inspect paths and filesystem identities.
- Detect internal versus external containment.
- Copy and move through temporary destinations.
- Calculate size progressively.
- Detect links, junctions, mount boundaries, and inaccessible entries.
- Revalidate targets before destructive operations.
- Move eligible content to platform trash.
- Perform narrowly scoped permanent deletion when explicitly approved.

It must return observations and typed outcomes rather than silently weakening safety checks.

### Provider registry

Resolves platform-specific application providers and exposes capability metadata.

Initial providers:

- WinGet provider.
- Portable-application provider.
- Manual installed-application association provider.
- Cautious Windows installer-artifact inspector.

Future providers are internal adapters until the contract has matured. v0.1 does not expose a public plugin system.

### Installed-application inventory

Produces normalized observations from platform sources. On Windows this may combine WinGet results and uninstall registration metadata. It does not assign ownership.

The installation correlator compares before/after inventories and returns candidates plus evidence and confidence. Only the user or a verified provider result can confirm an association.

### Download service

Downloads into a temporary area, records provenance and integrity metadata, reports progress, and moves verified results into managed storage. It never automatically executes a downloaded artifact.

### Classification service

Suggests categories from extension, content type, directory shape, source operation, and future user rules. Suggestions are advisory and must remain overridable.

### Storage insight service

Calculates managed, referenced, shared, estimated application, and reclaimable storage. It returns measurement type and confidence so the UI does not present false precision.

## 4. Application API

The application API uses commands for mutations and queries for reads. This is a code-organization rule, not distributed CQRS.

### Commands

```text
casklet cask create
casklet cask rename
casklet cask archive
casklet cask restore
casklet resource add
casklet resource categorize
casklet resource remove
casklet app install
casklet app associate
casklet cask export
casklet plan create
casklet plan approve
casklet plan execute
casklet operation cancel
casklet operation recover
```

### Queries

```text
casklet cask get
casklet cask list
casklet resource list
casklet resource get
casklet storage summary
casklet category list
casklet package search
casklet app list
casklet app get
casklet operation get
casklet operation list
casklet plan get
casklet recovery status
```

Each request and response is a dedicated typed data-transfer object. Presentation DTOs may combine fields for display, but domain entities are not serialized directly across the Tauri boundary.

## 5. CLI boundary

The CLI performs four tasks:

1. Parse command-line arguments.
2. Convert them into application requests.
3. Render progress, results, warnings, and errors.
4. Return documented process exit codes.

It contains no business rules. For example, `--dry-run` requests a removal plan; it does not independently calculate deletion actions.

Interactive prompts are presentation behavior. Non-interactive and machine-readable modes must be possible:

```text
--yes
--no-input
--output table|json
```

Destructive commands should not accept `--yes` without a valid current plan or equivalent backend revalidation.

## 6. GUI/Tauri boundary

The Svelte UI invokes narrowly scoped Tauri commands. The Tauri layer validates transport inputs, creates application requests, calls the same application services as the CLI, and maps results to frontend DTOs.

The frontend must not receive:

- unrestricted shell execution;
- direct database access;
- unrestricted filesystem access;
- raw uninstall commands;
- authority to decide deletion eligibility.

For long work, the start command returns an operation ID. Progress is emitted as typed events:

```text
operation.started
operation.progress
operation.awaiting_user
operation.completed
operation.failed
```

The client can always query operation state after reconnecting; events are a responsiveness mechanism, not the source of truth.

## 7. Progress interface

The operation engine publishes neutral progress updates through a port. Presentations supply adapters:

```text
CLI progress sink -> terminal renderer
GUI progress sink -> Tauri event emitter
Test progress sink -> in-memory collector
```

Progress payloads contain stable step codes and structured fields. Human messages are rendered at the presentation edge so future localization does not require changing the operation engine.

## 8. Transaction and external-work boundary

External work must be split into phases:

```text
Transaction 1
  validate state
  create operation and planned steps
  commit

External work
  copy/download/install/uninstall

Transaction 2
  record observation and result
  update domain state
  mark projection pending
  commit

Projection
  regenerate manifest idempotently
```

Waiting for a user, installer, network, or filesystem copy while holding a database transaction is prohibited.

## 9. Concurrency model

- Multiple readers are allowed.
- Mutations use entity revisions for optimistic concurrency.
- One destructive operation per Cask may run at a time.
- A global installation mutation lock protects provider operations that affect system-wide inventory.
- Database locking and external-operation locking are distinct.
- Conflicts return a typed result rather than waiting indefinitely.
- Operation steps are designed to be idempotent, verifiable, or explicitly non-repeatable.

No in-memory lock may be the sole protection because CLI and GUI are separate processes.

## 10. Dependency injection and construction

Application services receive interfaces for repositories, filesystem behavior, providers, clock, IDs, progress, and operation locks. The executable composition root chooses implementations.

Composition roots:

```text
CLI main -> production dependencies -> application services
Tauri setup -> production dependencies -> application services
Tests -> temporary/fake dependencies -> application services
```

Avoid a global service locator. Explicit construction keeps dependencies visible and tests controlled.

## 11. Platform evolution

The platform-neutral core consumes normalized observations and capabilities. Platform implementations can be added without changing user-facing concepts:

```text
Windows adapters: WinGet, MSI/registry inventory, Windows trash, Authenticode
Linux adapters: Flatpak, apt/dpkg, AppImage, freedesktop trash
macOS adapters: app bundles, Homebrew, DMG/PKG inspection, Finder trash
```

Do not force every provider into identical semantics. Unsupported capabilities remain explicitly unsupported.

Future isolation should enter through storage and execution profiles rather than changing the Cask entity model. `MANAGED` and future `ISOLATED` modes can select different adapter compositions.

## 12. Module boundaries

Initial conceptual modules:

```text
domain
  casks
  resources
  applications
  operations
  removal
  storage_insights

application
  commands
  queries
  services
  ports
  policies

infrastructure
  sqlite
  manifests
  filesystem
  downloads
  windows
  providers

presentation
  cli
  tauri
  web_ui
```

These are logical boundaries. They do not all need separate Rust crates in v0.1. Begin with a small number of crates and split only when dependency direction, compilation isolation, or independent testing makes the boundary valuable.

## 13. Failure behavior

Adapters return typed facts and failures to the application layer. They do not convert uncertain outcomes into success.

Examples:

- Provider process returned success but verification failed: installation state becomes `UNKNOWN` or `PARTIALLY_INSTALLED`, not `INSTALLED`.
- File was copied but database registration failed: operation becomes recovery-required and records the candidate orphan path.
- Manifest update failed after a committed mutation: core state remains committed and projection is marked pending.
- GUI closes during an operation: durable state remains queryable when it reopens.
- Progress event is missed: the client reloads current operation state.

## 14. Architecture fitness checks

Once implementation begins, automated checks should help preserve these boundaries:

- Domain crate has no Tauri, SQLite, WinGet, HTTP, or OS-specific dependencies.
- Presentation code cannot import persistence implementations.
- Removal execution requires an approved plan identifier.
- Provider capability checks precede provider actions.
- Imported manifests pass schema validation and migration.
- Cross-process operation locks are tested.
- CLI and GUI contract tests exercise the same application use cases.

## 15. Decisions proposed for approval

1. Use a local-first modular monolith.
2. Enforce inward dependency direction through ports and adapters.
3. Make the application layer the single entry point for CLI and GUI behavior.
4. Keep the operation engine in-process but durable; do not create a daemon in v0.1.
5. Separate removal planning from removal execution.
6. Use typed Tauri IPC rather than a local HTTP server.
7. Treat progress events as transient notifications backed by queryable operation state.
8. Keep platform and provider capabilities explicit.
9. Keep manifests as recoverable projections of committed state.
10. Begin with a few coarse modules/crates and split only for demonstrated reasons.

## Completion gate

After this design is approved, the next phase is persistence and schema design. That phase will map the domain to SQLite tables, indexes, constraints, migrations, manifest schemas, consistency rules, and backup/recovery behavior.
