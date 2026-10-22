<picture>
  <source media="(prefers-color-scheme: dark)" srcset="apps/desktop/src/assets/casklet-logo-white.png">
  <img src="apps/desktop/src/assets/casklet-logo-black.png" alt="Casklet logo" width="72">
</picture>

# Casklet

**Keep an activity's files, applications, and cleanup in one place.**

Casklet is a local-first lifecycle and resource manager for Windows. A **Cask** groups the files, folders, and software associated with a project or activity, giving you a clear inventory of what it uses, where it lives, and what can be safely cleaned up when the work is done.

Use the desktop app or the CLI. Both run on the same Rust application core and follow the same ownership and cleanup rules.

[Getting started](#getting-started) · [CLI quick start](#cli-quick-start) · [User guide](docs/product/user-guide.md) · [Contributing](CONTRIBUTING.md)

> **Developer Alpha · 0.1.0-alpha.1 · Windows 11 x64**
>
> Casklet is under active development. Local Alpha candidates have been tested, but signing, independent security review, and clean-machine release checks remain open. It is not cleared for broad public distribution. Use disposable data when evaluating cleanup.

## Why Casklet?

A project can leave behind more than a folder: downloaded installers, reference documents, portable tools, and installed applications. Casklet records those relationships so you can understand the whole activity without moving everything into a single directory.

For example, a Cask for a video project could reference footage on another drive, own dedicated working files, and link the tools used to edit them. When the project ends, a cleanup preview distinguishes eligible removals from items that must stay.

**Association never implies ownership.** Linking a file or an existing application does not give Casklet permission to delete or uninstall it.

## What you can do

| Capability                       | How it helps                                                                                                                            |
| -------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| Organize activities              | Create Casks with descriptions, categories, and reversible archiving.                                                                   |
| Track files and folders          | Reference existing content or explicitly manage dedicated data at its current location.                                                 |
| Manage application relationships | Associate existing Windows applications, inspect portable ZIPs, and review WinGet or MSI/EXE installation workflows.                    |
| Understand storage               | View item counts, saved logical sizes, owned versus referenced content, and reclaimable estimates. Unknown measurements remain visible. |
| Review cleanup                   | Inspect planned actions before approval; shared, ambiguous, changed, and unverified resources are preserved.                            |
| Export and recover               | Export JSON inventories or Markdown reports, inspect interrupted operations, and back up database metadata.                             |
| Work your way                    | Use a native desktop workspace or a CLI with readable tables, Cask overviews, verbose diagnostics, and JSON output.                     |

The desktop includes separate Cask workspaces, file and application management, activity history, and Light/Dark appearance with custom accents. Installed Windows builds also provide Casklet actions under File Explorer's **Show more options** menu.

## Safety by design

Casklet manages resources; it does not isolate applications or trace every change an installer makes.

- **References stay references.** Adding an association grants no deletion or uninstall authority.
- **Ownership is explicit or verified.** Dedicated file ownership requires consent. Application ownership requires evidence from a supported installation workflow.
- **Destructive work starts with a plan.** Cleanup requires inspection, approval, confirmation, and revalidation immediately before execution.
- **Uncertainty means preservation.** Shared dependencies, changed identities, and unsupported uninstall mechanisms block automatic removal.
- **Interrupted work remains inspectable.** Durable operation journals support recovery and explain partial or uncertain results.

Archive hides a Cask without deleting its contents. Resource cleanup and permanent Cask removal are separate operations; the CLI removes a Cask only after remaining dependencies and unfinished work are resolved.

### Build capabilities

| Build profile      | Cleanup capability                                                                                                              |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------- |
| Default            | Generate and inspect cleanup previews; destructive cleanup is disabled.                                                         |
| `alpha-cleanup`    | Enables reviewed file cleanup and supported, verified MSI/EXE application cleanup. WinGet uninstall remains disabled.           |
| `reviewed-removal` | Additional developer capability gate, including WinGet uninstall; enabling it does not constitute security or release approval. |

Generic Copy/Move intake is temporarily disabled. Reference and explicit ownership at the current location remain available. Arbitrary installer cleanup, application sandboxing, Linux/macOS releases, cloud synchronization, accounts, and a public plugin API are outside v0.1 scope.

## Getting started

The instructions below build Casklet from source on **Windows 11 x64**, using PowerShell from the repository root. Active development targets `develop`.

### Prerequisites

- Git and Rustup. The repository pins Rust and its `rustfmt`/Clippy components in [`rust-toolchain.toml`](rust-toolchain.toml).
- Node.js 24 and Corepack. The pnpm version is pinned in [`package.json`](package.json).
- Microsoft C++ Build Tools with the C++ desktop workload and a Windows SDK.
- Microsoft Edge WebView2 Runtime for the desktop app.
- WinGet when testing WinGet provider workflows.

### Set up the checkout

```powershell
git clone --branch develop https://github.com/raihan4O4/casklet.git
cd casklet
corepack pnpm install --frozen-lockfile
./scripts/bootstrap.ps1
```

The bootstrap script checks prerequisites and locates Rust in the standard Rustup directory. If Cargo is not on your session's `PATH`, run the script again in each new PowerShell session.

### Run the desktop app

```powershell
corepack pnpm --filter @casklet/desktop tauri dev
```

Create a Cask, then use **Add Items** to reference a file or folder. Its workspace brings together **Overview**, **Files & Folders**, **Applications**, **Activity**, and **Settings**. Start with references while learning the workflow.

### CLI quick start

Build the CLI, then use an isolated development directory for this example:

```powershell
cargo build -p casklet-cli --locked
$cli = Join-Path $PWD 'target/debug/casklet.exe'
$env:CASKLET_DEV_ROOT = Join-Path $PWD ("target/casklet-demo-" + [guid]::NewGuid())

& $cli --help
$cask = & $cli --output json cask create 'My Project' | ConvertFrom-Json
& $cli cask list
& $cli cask get $cask.id

# Reference this checkout's README without moving it or granting ownership.
& $cli resource add $cask.id "$PWD/README.md" --mode reference
& $cli resource list $cask.id
& $cli storage summary $cask.id

# Preview only: this does not delete files or detach the reference.
& $cli cleanup create $cask.id
```

Human output hides unnecessary internal identifiers. Use `--verbose` for technical details and `--output json` for stable IDs and machine-readable DTOs; duplicate Cask names are allowed. `cask overview` is an alias for `cask get`. Color follows terminal capabilities and can be controlled with `--color auto|always|never` or `NO_COLOR`.

See the [complete CLI walkthrough](docs/engineering/cli-terminal-ux.md#manual-testing-powershell) for ownership inspection, cleanup approval/execution, recovery, output modes, and sample terminal output.

### Evaluate Alpha cleanup

Use an explicitly enabled build and disposable fixtures:

```powershell
cargo build -p casklet-cli --features alpha-cleanup --locked
corepack pnpm --filter @casklet/desktop tauri dev --features alpha-cleanup
```

These commands enable cleanup capabilities; they do not execute cleanup. The normal plan, approval, confirmation, ownership, and revalidation requirements still apply. Review the [Alpha readiness notes](docs/engineering/alpha-readiness.md) before testing installers or destructive operations.

## Your data stays local

Casklet has no required account, cloud service, or always-running daemon. SQLite holds operational metadata; per-Cask manifests provide recoverable inventory projections. Exports and manifest recovery do not recreate deletion authority.

| Data                                             | Default location                                                                         |
| ------------------------------------------------ | ---------------------------------------------------------------------------------------- |
| Development metadata and managed content         | `.casklet-dev/` in the checkout; debug builds can override this with `CASKLET_DEV_ROOT`. |
| Installed/release metadata and backups           | `%LOCALAPPDATA%\CaskletData`                                                             |
| Installed/release managed content                | `%USERPROFILE%\Casklets`                                                                 |
| Referenced and explicitly owned external content | Its original location                                                                    |

Release builds use production paths; `CASKLET_DEV_ROOT` is a debug override. Database backups contain metadata, not resource bytes. Back up files separately. Uninstalling Casklet preserves its data and is not a cleanup operation. Exports and diagnostics can contain private paths and application details; inspect them before sharing.

## Development

The repository is a Rust workspace with a Tauri 2 / Svelte 5 / TypeScript desktop frontend. Domain rules and application services are independent of the desktop shell, SQLite, and Windows APIs; adapters provide persistence and platform behavior.

```text
crates/
  casklet-domain/          Ownership rules, identities, and state transitions
  casklet-application/     Shared commands, queries, and operation workflows
  casklet-contracts/       Typed transport contracts and generated frontend types
  casklet-infrastructure/  SQLite, filesystem, installer, and Windows adapters
  casklet-cli/             Terminal interface and composition root
apps/desktop/             Tauri shell and Svelte frontend
xtask/                    Shared checks and packaging tasks
docs/                     Product, architecture, engineering, and security docs
```

Run the shared local CI gate before submitting changes:

```powershell
cargo xtask check
```

It checks Rust formatting and strict Clippy, runs workspace and capability-profile tests, builds the workspace, checks frontend formatting/lint/types/tests/build, and verifies generated contracts. Dependency checks are separate: `cargo deny check` and `corepack pnpm audit --audit-level high`.

Read [CONTRIBUTING.md](CONTRIBUTING.md), [AGENTS.md](AGENTS.md), and the [project context](docs/PROJECT_CONTEXT.md) before changing behavior. Test destructive operations only against explicitly created disposable fixtures.

## Documentation and support

| Resource                                                            | Start here for                                                       |
| ------------------------------------------------------------------- | -------------------------------------------------------------------- |
| [User guide](docs/product/user-guide.md)                            | Desktop workflows, ownership, storage, recovery, and troubleshooting |
| [CLI walkthrough](docs/engineering/cli-terminal-ux.md)              | Real commands, terminal output, JSON, and cleanup examples           |
| [Architecture](docs/architecture/system-design-v0.1.md)             | System boundaries, data flow, and persistence design                 |
| [Architecture decisions](docs/adr/)                                 | The reasoning behind consequential technical choices                 |
| [Release checklist](docs/engineering/milestone-10-release-gates.md) | Packaging requirements and outstanding release gates                 |
| [Security policy](SECURITY.md)                                      | Reporting vulnerabilities privately                                  |
| [GitHub issues](https://github.com/raihan4O4/casklet/issues)        | Bugs and feature requests; redact personal paths and diagnostics     |

## License

Casklet is licensed under [Apache License 2.0](LICENSE). See [NOTICE](NOTICE) and [third-party notices](LICENSES/README.md) for bundled components and attribution.
