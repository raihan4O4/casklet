# Casklet v0.1 — Consolidated System Design

Status: Approved v0.1 baseline; living document

Last updated: 2026-09-14

## 1. Purpose and diagram policy

This document is the visual entry point to Casklet's system design. Detailed reasoning and rules remain in the linked architecture, persistence, security, and engineering documents.

Only diagrams that answer a distinct engineering question are included:

- System context: who and what interacts with Casklet?
- Component diagram: where responsibilities and dependency boundaries live.
- Data-flow diagrams: how untrusted data and state-changing operations move.
- ER diagram: how durable entities relate.
- Sequence diagrams: how crash-sensitive workflows are ordered.
- Trust-boundary diagram: where security validation is required.
- Deployment diagram: what actually runs and is installed in v0.1.

## 2. System context

```mermaid
flowchart LR
    User[Windows user]
    CLI[Casklet CLI]
    GUI[Casklet desktop GUI]
    Core[Casklet application core]
    FS[(Local files and Cask storage)]
    DB[(Local SQLite database)]
    Provider[WinGet and installed-app inventory]
    Network[Download sources]
    OS[Windows / UAC / Recycle Bin]

    User --> CLI
    User --> GUI
    CLI --> Core
    GUI --> Core
    Core <--> DB
    Core <--> FS
    Core <--> Provider
    Core --> Network
    Core <--> OS
```

Casklet is local-first. There is no Casklet cloud backend, account server, or always-running daemon in v0.1.

## 3. Runtime component architecture

```mermaid
flowchart TB
    subgraph Presentation
        CLI[CLI adapter]
        WebView[Svelte WebView]
        Tauri[Tauri IPC adapter]
        WebView -->|typed IPC| Tauri
    end

    subgraph Core
        Contracts[Transport contracts]
        App[Application services]
        Ops[Durable operation engine]
        Planner[Removal planner]
        Executor[Removal executor]
        Domain[Domain model and policies]

        App --> Domain
        App --> Ops
        Ops --> Planner
        Ops --> Executor
        Planner --> Domain
        Executor --> Domain
    end

    subgraph Infrastructure
        Repos[SQLite repositories]
        Manifest[Manifest projector/importer]
        Files[Windows filesystem adapter]
        Downloads[Download adapter]
        Registry[Provider registry]
        WinGet[WinGet provider]
        Inventory[Installed-app inventory]
        Trust[Signature/trust inspector]

        Registry --> WinGet
        Registry --> Inventory
        Registry --> Trust
    end

    CLI --> Contracts
    Tauri --> Contracts
    Contracts --> App
    App --> Repos
    App --> Manifest
    Ops --> Files
    Ops --> Downloads
    Ops --> Registry
```

Dependency direction is inward: presentation and infrastructure depend on application/domain contracts; the domain does not depend on Tauri, SQLite, WinGet, or OS APIs.

## 4. DFD Level 0 — Cask lifecycle

```mermaid
flowchart LR
    U[User]
    C((Casklet core))
    M[(Cask metadata)]
    R[(Managed and referenced resources)]
    A[Application providers]
    E[Exports and reports]

    U -->|create, add, install, inspect, remove| C
    C -->|status, plans, warnings, progress| U
    C <--> M
    C <--> R
    C <--> A
    C --> E
```

## 5. DFD Level 1 — Add file or folder

```mermaid
flowchart TD
    Input[CLI path or GUI drop]
    Validate[Validate request and Cask state]
    Inspect[Inspect source path and filesystem identity]
    Decide{Copy, move, or reference?}
    Intent[(Persist operation intent)]
    Stage[Copy into operation staging]
    Verify[Verify result]
    Finalize[Finalize managed destination]
    Register[(Register Resource, location, association)]
    Project[Queue manifest projection]
    External[Leave source unchanged]
    Result[Return Resource and operation result]

    Input --> Validate --> Inspect --> Decide
    Decide -->|copy| Intent --> Stage --> Verify --> Finalize --> Register
    Decide -->|move| Intent
    Decide -->|reference| External --> Register
    Register --> Project --> Result
```

For cross-volume moves, the staged copy is verified and registered before source removal. Drag-and-drop never silently defaults to move.

## 6. DFD Level 1 — Application installation

```mermaid
flowchart TD
    Request[Provider package or installer artifact]
    Resolve[Resolve provider and package identity]
    Existing[Capture existing installation state]
    Plan[Create installation plan]
    Confirm[User review and confirmation]
    Journal[(Persist operation and steps)]
    Provider[Invoke provider without a shell]
    Observe[Inspect resulting installed-app inventory]
    Verify{Identity verified?}
    Installed[(Record Installation and Cask association)]
    Unknown[(Record unresolved or partial state)]

    Request --> Resolve --> Existing --> Plan --> Confirm --> Journal --> Provider --> Observe --> Verify
    Verify -->|yes| Installed
    Verify -->|no| Unknown
```

Installer exit success does not independently prove installation success or ownership.

## 7. DFD Level 1 — Cask destruction

```mermaid
flowchart TD
    Request[Destroy request]
    Reconcile[Reconcile relevant current state]
    Build[Build immutable removal plan]
    Review[Show delete, uninstall, detach, retain, and warnings]
    Approve[(Store approval for exact plan hash)]
    Revalidate{Revalidate item and plan}
    Execute[Execute exact provider/filesystem action]
    Record[(Record item outcome)]
    More{More items?}
    Finish[Mark destroyed, partial, or recovery-required]

    Request --> Reconcile --> Build --> Review --> Approve --> Revalidate
    Revalidate -->|valid| Execute --> Record --> More
    Revalidate -->|changed/unsafe| Record
    More -->|yes| Revalidate
    More -->|no| Finish
```

The GUI cannot submit executable deletion items. It approves an exact plan generated and stored by the core.

## 8. Entity relationship model

```mermaid
erDiagram
    CASKS ||--o{ RESOURCE_ASSOCIATIONS : groups
    RESOURCES ||--o{ RESOURCE_ASSOCIATIONS : associated_with
    CATEGORIES ||--o{ RESOURCE_ASSOCIATIONS : classifies
    RESOURCES ||--o{ RESOURCE_LOCATIONS : observed_at
    RESOURCES ||--o| INSTALLER_ARTIFACTS : may_describe
    RESOURCES ||--o{ RESOURCES : parent_of

    APPLICATIONS ||--o{ APPLICATION_ALIASES : identified_by
    APPLICATIONS ||--o{ INSTALLATIONS : has
    INSTALLATIONS ||--o{ INSTALLATION_LOCATIONS : observed_at
    CASKS ||--o{ CASK_INSTALLATIONS : uses
    INSTALLATIONS ||--o{ CASK_INSTALLATIONS : associated_with

    CASKS ||--o{ OPERATIONS : owns
    OPERATIONS ||--|{ OPERATION_STEPS : contains
    OPERATIONS ||--o{ OPERATION_LOCKS : holds

    CASKS ||--o{ REMOVAL_PLANS : plans
    REMOVAL_PLANS ||--|{ REMOVAL_PLAN_ITEMS : contains
    OPERATIONS o|--o| REMOVAL_PLANS : executes

    CASKS ||--o{ PROJECTION_JOBS : projects
```

Key modeling rules:

- Paths are not Resource identities.
- Cask-to-Resource and Cask-to-Installation relationships are many-to-many.
- Category belongs to the Resource association.
- Installer artifact, logical Application, and installed instance are distinct.
- Sharing is derived from active associations.

## 9. Sequence — crash-safe managed copy

```mermaid
sequenceDiagram
    actor User
    participant UI as CLI/GUI
    participant App as Application service
    participant DB as SQLite
    participant FS as Filesystem adapter
    participant MP as Manifest projector

    User->>UI: Add file --copy
    UI->>App: AddResource request
    App->>FS: Inspect source and destination
    FS-->>App: Identity, size, safety facts
    App->>DB: Persist operation, steps, staging/final paths
    DB-->>App: Commit
    App->>FS: Copy to staging
    FS-->>UI: Progress events
    App->>FS: Verify and finalize destination
    FS-->>App: Final identity
    App->>DB: Persist Resource and association
    DB-->>App: Commit and projection pending
    App->>MP: Generate manifest idempotently
    MP-->>App: Projection result
    App-->>UI: Completed or recovery-required
```

No database transaction remains open during the copy.

## 10. Sequence — plan-before-destruction

```mermaid
sequenceDiagram
    actor User
    participant UI as CLI/GUI
    participant App as Application service
    participant DB as SQLite
    participant Planner as Removal planner
    participant Exec as Removal executor
    participant Ext as Filesystem/provider

    User->>UI: Destroy Cask
    UI->>App: CreateRemovalPlan
    App->>DB: Load current revisions and associations
    App->>Planner: Generate plan from current facts
    Planner-->>App: Immutable plan and warnings
    App->>DB: Persist plan and hash
    App-->>UI: Plan for review
    User->>UI: Approve exact plan
    UI->>App: Approve and execute plan ID
    App->>DB: Verify stored plan, approval, revisions, lock
    App->>Exec: Execute approved plan
    loop Each plan item
        Exec->>Ext: Revalidate target/provider identity
        alt Valid and authorized
            Exec->>Ext: Perform exact action
            Exec->>DB: Record outcome
        else Changed, shared, or unsafe
            Exec->>DB: Record retained/blocked outcome
        end
    end
    Exec-->>App: Final result
    App-->>UI: Destroyed, partial, or recovery-required
```

## 11. Trust boundaries

```mermaid
flowchart TB
    subgraph U[Untrusted inputs]
        Args[CLI arguments]
        Drop[Dropped files and folders]
        Import[Imported manifests/archives]
        Urls[URLs and network responses]
        Output[Provider and installer output]
    end

    subgraph P[Presentation boundary]
        GUI[Svelte WebView]
        CLI[CLI parser]
    end

    subgraph T[Trusted policy boundary]
        IPC[Typed IPC/request validation]
        Core[Application/domain authorization]
        Plan[Removal plan and revalidation]
    end

    subgraph X[External mutable state]
        FS[Filesystem]
        Providers[WinGet/installers]
        OS[Windows/UAC/Recycle Bin]
    end

    U --> P
    GUI --> IPC
    CLI --> IPC
    IPC --> Core --> Plan
    Plan <--> X
```

Every arrow crossing into the trusted policy boundary requires typed validation. Data returning from the filesystem or providers is also revalidated; external reality is not inherently trusted.

## 12. v0.1 deployment model

```mermaid
flowchart LR
    subgraph Machine[Windows 11 x64 user machine]
        CLI[casklet.exe]
        Desktop[Casklet desktop.exe]
        WebView[WebView2 runtime]
        Core[Shared Rust libraries compiled into each executable]
        DB[(casklet.db)]
        Storage[(Cask managed storage)]
        WinGet[Windows Package Manager]

        CLI --> Core
        Desktop --> WebView
        Desktop --> Core
        Core <--> DB
        Core <--> Storage
        Core <--> WinGet
    end

    Releases[GitHub Releases] -->|manual install; signed updater deferred| Machine
```

The shared core is a source/library boundary, not a separate network service. CLI and desktop are separate processes and coordinate through SQLite operations and persistent locks.

## 13. Document relationships

```text
Product requirements and cross-platform decisions
        -> domain model
        -> component architecture
        -> persistence design
        -> security threat model
        -> repository/tooling design
        -> this consolidated visual map
```

Detailed sources:

- `docs/architecture/component-architecture.md`
- `docs/architecture/persistence-design.md`
- `docs/security/threat-model-v0.1.md`
- `docs/engineering/repository-and-tooling.md`
- `domain model.txt` until normalized into the architecture directory

## 14. Deferred diagrams

The following diagrams would not add enough value yet and are deliberately deferred:

- Cloud deployment and synchronization topology.
- Public plugin architecture.
- Isolation/container/VM topology.
- Linux and macOS provider detail.
- UI screen navigation map and wireframes.
- Class diagrams mirroring every Rust type.

They should be added only when their corresponding features enter an approved scope.
