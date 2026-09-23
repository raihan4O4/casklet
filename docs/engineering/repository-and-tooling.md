# Phase 7 — Repository Structure and Engineering Toolchain

Status: Approved on 2026-09-14

Research baseline: 2026-09-14

## 1. Purpose

This phase chooses the repository layout, build toolchain, dependency policy, development workflow, CI/CD structure, and release artifacts for Casklet v0.1. It is intentionally concrete enough to scaffold after approval without turning this document into implementation code.

The priorities are:

1. Safe and understandable for a solo developer learning the stack.
2. One implementation of business behavior shared by CLI and GUI.
3. Fast local feedback.
4. Reproducible builds through pinned tools and committed lockfiles.
5. Strict boundaries around filesystem, provider, and removal code.
6. Free/open-source tooling wherever practical.

## 2. Toolchain baseline

Pin exact versions in repository files when scaffolding begins:

| Tool | Initial baseline | Pinning mechanism | Reason |
|---|---|---|---|
| Rust | 1.98.1 | `rust-toolchain.toml` | Stable patched compiler; 1.98.1 fixes a 1.98.0 miscompilation |
| Rust edition | 2024 | workspace `Cargo.toml` | Current stable language edition |
| Node.js | 24 LTS | `.node-version` and `package.json` engines | Supported LTS rather than Current |
| pnpm | 12.x exact patch at scaffold time | `packageManager` | Fast workspace package manager with an exact project pin |
| Tauri | 2.11.x exact compatible patch set | Cargo and pnpm lockfiles | Current Tauri 2 stable line at research time |
| Svelte | 5.x exact resolved version | `pnpm-lock.yaml` | Lean compiled UI and explicit Svelte 5 runes model |
| TypeScript | current supported stable at scaffold time | `pnpm-lock.yaml` | Static frontend and contract checking |
| SQLite | bundled through Rust adapter | `Cargo.lock` | Known database engine independent of host installation |

Rust 1.98.1 was released specifically to fix a compiler miscompilation in 1.98.0. Pinning the point release avoids silently building with the affected compiler. [Rust 1.98.1 announcement](https://blog.rust-lang.org/releases/latest/)

Node 24 is LTS as of this design, while Node 26 remains Current until its LTS transition. Node recommends production applications use Active or Maintenance LTS lines. [Node.js release schedule](https://nodejs.org/en/about/previous-releases)

pnpm 12 is the current release line and supports Windows; the exact patch will be pinned in the repository. [pnpm installation documentation](https://pnpm.io/installation)

Tauri 2.11 is the current stable family at this design checkpoint. All directly declared Tauri Rust and npm packages must use mutually compatible versions locked together. [Tauri releases](https://v2.tauri.app/release/)

Version numbers are a reproducibility baseline, not a promise to remain frozen. Dependency/tool upgrades use explicit pull requests, changelog review, CI, and security checks.

## 3. Repository layout

Proposed monorepo:

```text
casklet/
├── AGENTS.md
├── README.md
├── CONTRIBUTING.md
├── SECURITY.md
├── LICENSES/
├── CHANGELOG.md
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── rustfmt.toml
├── deny.toml
├── package.json
├── pnpm-lock.yaml
├── pnpm-workspace.yaml
├── .node-version
├── .editorconfig
├── .gitignore
├── crates/
│   ├── casklet-domain/
│   ├── casklet-contracts/
│   ├── casklet-application/
│   ├── casklet-infrastructure/
│   └── casklet-cli/
├── apps/
│   └── desktop/
│       ├── src/
│       ├── static/
│       ├── tests/
│       ├── src-tauri/
│       ├── package.json
│       ├── vite.config.ts
│       └── svelte.config.js
├── xtask/
├── migrations/
├── schemas/
│   ├── ipc/
│   └── manifests/
├── tests/
│   ├── fixtures/
│   ├── integration/
│   ├── adversarial-filesystems/
│   └── provider-fixtures/
├── docs/
│   ├── adr/
│   ├── architecture/
│   ├── engineering/
│   ├── product/
│   └── security/
├── scripts/
│   ├── bootstrap.ps1
│   └── bootstrap.sh
└── .github/
    ├── workflows/
    ├── ISSUE_TEMPLATE/
    └── pull_request_template.md
```

The repository root is a Cargo workspace and pnpm workspace. One repository is simpler for atomic changes to Rust contracts, frontend generated types, migrations, and documentation.

## 4. Rust crate boundaries

### `casklet-domain`

Contains:

- Entities and value objects.
- State transitions.
- Ownership and removal eligibility policies.
- Removal-plan domain model.
- Storage-accounting rules.
- Stable domain error codes.

Allowed production dependencies should remain extremely small, initially:

- `uuid` for typed stable identifiers.
- `thiserror` if domain error definitions benefit from it.

Avoid Serde in the domain initially. Persistence and IPC serialization should not define the domain model.

Forbidden dependencies include Tauri, Tokio, SQLite, HTTP clients, WinGet/Windows APIs, and frontend contracts.

### `casklet-contracts`

Contains versioned transport types:

- Command request DTOs.
- Query request/response DTOs.
- Operation progress events.
- Public error DTOs.
- Manifest/export envelopes or references to their dedicated schema module.
- TypeScript generation support.

Dependencies:

- `serde` and `serde_json`.
- `ts-rs` or another reviewed minimal generator for TypeScript declarations.
- `schemars` only where JSON Schema generation is actually required.

Contracts use transport primitives such as ID strings and validate/convert at the application boundary. They do not expose domain objects directly.

### `casklet-application`

Contains:

- Commands and queries.
- Application services.
- Port traits.
- Operation engine.
- Removal planner/executor orchestration.
- Reconciliation coordination.
- Transaction scripts and policy composition.

Depends on:

- `casklet-domain`.
- `casklet-contracts`.
- `tokio` primitives needed for cancellation and asynchronous orchestration.
- `async-trait` only if object-safe asynchronous ports require it after a small prototype proves the need.

It does not depend on concrete persistence, Tauri, WinGet, or Windows APIs.

### `casklet-infrastructure`

Contains adapters:

- SQLite repositories and migrations.
- Manifest projection/import.
- Windows filesystem identity and safe file operations.
- Downloads.
- WinGet provider.
- Portable ZIP provider.
- Windows installed-application inventory.
- Authenticode inspection.
- Structured logging adapters.

Candidate dependencies:

- `rusqlite` with bundled SQLite.
- `rusqlite_migration` or another small reviewed migration layer.
- `tokio`.
- `reqwest` using Rustls TLS.
- `url`.
- `sha2`.
- `tracing`, `tracing-subscriber`, and `tracing-appender`.
- `windows` for narrowly selected Win32 APIs.
- `zip` for validated ZIP extraction.
- `infer` and/or `mime_guess` for classification evidence.

Dependency inclusion is not automatic: every candidate is reviewed when its feature is implemented.

### `casklet-cli`

Contains only:

- Argument parsing.
- Interactive confirmation.
- Human/table/JSON rendering.
- Progress rendering.
- Exit-code mapping.
- CLI composition root.

Candidate dependencies:

- `clap` with derive.
- `indicatif` for progress.
- `comfy-table` or `tabled`, selecting one after a small display spike.
- `serde_json` for machine-readable output.
- `anyhow` only at the binary boundary for final context/reporting; typed application errors remain intact.

### Tauri desktop crate

`apps/desktop/src-tauri` is the desktop composition root and IPC adapter. It depends on application, infrastructure, and contracts but contains no duplicate business policy.

Use only required Tauri features and plugins. Initial likely needs:

- Native file/folder dialog.
- External opener for explicitly approved paths/URLs.
- Single-instance behavior if testing proves simultaneous GUI instances confusing.
- Logging integration only if it does not bypass central redaction.

Do not use Tauri SQL, generic filesystem, generic shell, or generic HTTP access from the frontend. Rust infrastructure owns those operations.

## 5. Frontend structure

```text
apps/desktop/src/
├── app/
│   ├── App.svelte
│   ├── navigation.ts
│   └── initialization.ts
├── features/
│   ├── casks/
│   ├── resources/
│   ├── applications/
│   ├── operations/
│   ├── removal/
│   └── settings/
├── components/
│   ├── primitives/
│   └── layout/
├── contracts/
│   └── generated/
├── ipc/
│   ├── client.ts
│   ├── events.ts
│   └── errors.ts
├── state/
├── styles/
└── test/
```

Frontend choices:

- Svelte 5 runes for new reactive code.
- TypeScript strict mode.
- Vite.
- Plain scoped CSS plus global design tokens/CSS custom properties.
- Native semantic HTML and accessible custom primitives.
- No SvelteKit server, Tailwind, Redux-style state library, chart library, or full component framework in the initial scaffold.
- No raw `{@html}` for application/provider/user content.

Svelte compiles declarative components into optimized JavaScript, and Svelte 5's runes make state and derived behavior explicit. [Svelte documentation](https://svelte.dev/docs/svelte/overview)

State is divided into:

- Server/core state: loaded through typed query functions and invalidated after commands.
- View state: local component or feature state.
- Operation state: queryable durable status plus transient progress events.

Do not duplicate domain entities into a complex frontend store. The UI receives purpose-built views.

## 6. Type and schema generation

The Rust contracts crate is authoritative for IPC DTO shape.

Workflow:

```text
Rust DTOs
  -> deterministic TypeScript generation
  -> apps/desktop/src/contracts/generated
  -> formatting and clean-tree check in CI
```

Generated TypeScript is committed so frontend tooling does not need to compile Rust merely to type-check. CI regenerates it and fails if the working tree differs.

JSON manifests additionally have checked-in JSON Schemas under `schemas/manifests`. Schema fixtures test both Rust parsing and published schema behavior.

Generated files carry a clear header and are never hand-edited.

## 7. Database choice

Use `rusqlite` with bundled SQLite rather than a system SQLite installation.

Reasoning:

- Casklet is an embedded local application, not a network service.
- Direct synchronous database calls are easier to reason about than pretending SQLite is natively asynchronous.
- We can guarantee a tested SQLite version across user machines.
- Queries remain explicit and reviewable.
- Short database work can run through a dedicated blocking executor without freezing the Tauri runtime.

Application ports remain unaware of rusqlite. If a future architecture needs another database, only the adapter and composition change.

Rules:

- No SQL in CLI, Tauri commands, or domain code.
- Migration SQL is checked in and immutable after release.
- Repository integration tests use real temporary SQLite databases.
- No ORM or active-record layer.
- Map rows explicitly into persistence records, then into domain objects.

## 8. Asynchronous model

Use Tokio for:

- Downloads.
- Provider processes.
- Cancellation tokens.
- Progress channels.
- Coordinating long operations.

Use blocking tasks or a small database executor for rusqlite and blocking filesystem APIs. CPU-heavy scans/hashes also run on bounded blocking workers.

Do not mark every function async. Pure domain logic and ordinary mapping remain synchronous.

Bound concurrency globally and per operation so scanning or hashing a large directory cannot exhaust threads, memory, handles, or disk bandwidth.

## 9. Development command interface

Use a Rust `xtask` crate as the cross-platform project task runner rather than relying on Make or shell-specific command chains.

Expected commands after scaffolding:

```text
cargo xtask bootstrap-check
cargo xtask generate-contracts
cargo xtask check
cargo xtask test
cargo xtask test-integration
cargo xtask package
cargo xtask verify-release
```

PowerShell and shell bootstrap scripts only locate/install prerequisites with explicit consent and then delegate project work to `xtask`, Cargo, or pnpm.

The bootstrap script must not pipe remote scripts directly into a shell. It detects missing tools and gives verified installation instructions or uses explicit package-manager commands approved by the developer.

## 10. Formatting and linting

### Rust

- `cargo fmt --all --check`.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- Workspace-level lint configuration in root `Cargo.toml`.
- Forbid unsafe code by default in domain, contracts, application, and CLI.
- Permit narrowly scoped `unsafe` only in reviewed Windows infrastructure modules where safe wrappers cannot provide required identity behavior.
- Require a safety comment and targeted tests for every unsafe block.

### Frontend

- TypeScript strict mode.
- Svelte type checking.
- ESLint using the supported flat configuration.
- Prettier for TypeScript, Svelte, JSON, YAML, and Markdown where formatting is stable.
- Generated files excluded from lint mutation but validated by regeneration.

### Documentation

- Markdown linting with narrowly documented exceptions for long tables/diagrams.
- Mermaid syntax validation in CI after the system-design document is added.
- Link checking on scheduled or release workflows to reduce PR flakiness from external sites.

## 11. Testing tools

### Rust

- Built-in test harness for unit tests.
- `cargo-nextest` for CI execution once configured.
- `proptest` for ownership, removal-plan, path, and state-machine invariants.
- `tempfile` for isolated filesystem/database tests.
- `assert_cmd` and `predicates` for CLI integration tests.
- `cargo-llvm-cov` for informative coverage reporting.
- `cargo-fuzz` for manifest, archive-path, URL-redaction, and provider-output parsers on scheduled/manual jobs.

Avoid snapshot-only tests for destructive decisions. Expected removal actions should be asserted structurally so reviewers notice behavioral changes.

### Frontend

- Vitest.
- Svelte Testing Library.
- Playwright for critical end-to-end workflows.
- `axe-core` integration for automated accessibility checks, supplemented by keyboard/manual testing.

### Provider tests

- Fake provider for ordinary CI.
- Recorded redacted provider fixtures for parsing.
- Real WinGet smoke tests only in a controlled disposable Windows environment, never a shared developer machine or generic pull-request runner.

## 12. Dependency policy

Every production dependency must answer:

1. What capability does it provide?
2. Why is standard-library or existing dependency code insufficient?
3. Is it actively maintained?
4. What are its direct and transitive licenses?
5. Does it execute build/install scripts or include native code?
6. Does it expand Tauri permissions or network/filesystem access?
7. What is the recovery plan if maintenance stops?

Policy:

- Commit `Cargo.lock` and `pnpm-lock.yaml`.
- Avoid wildcard versions and git branch dependencies.
- Git dependencies require immutable commit hashes and a documented reason.
- Add dependencies through reviewable changes, even as a solo developer.
- Use Renovate or Dependabot for grouped, scheduled update proposals.
- Apply urgent security patches immediately after focused regression testing.
- Perform routine updates on a predictable cadence rather than allowing years of drift.
- Use `cargo deny` for advisories, duplicate-version visibility, sources, and licenses.
- Use `cargo audit` as an additional advisory signal.
- Audit npm/pnpm dependencies and review lifecycle scripts.

Initial license allowlist should include permissive licenses such as MIT, Apache-2.0, BSD variants, ISC, Unicode, and SQLite public-domain terms. Copyleft or unusual licenses require deliberate product/distribution review rather than silent rejection or acceptance.

Casklet's own license remains a product decision before public release. A dual MIT OR Apache-2.0 license is compatible with much of the Rust ecosystem, but trademark, contributor, and business-model goals should be reviewed first.

## 13. Local developer setup

Required Windows prerequisites:

- Windows 11 x64.
- Git.
- Rustup and the repository-pinned Rust toolchain.
- Node 24 LTS.
- Repository-pinned pnpm.
- Microsoft C++ Build Tools required by Tauri/Rust dependencies.
- WebView2 runtime.
- WinGet for provider development.

Recommended editor:

- VS Code with rust-analyzer, Even Better TOML, Svelte, ESLint, and CodeLLDB; or an equivalent IDE.

Do not require a globally installed Tauri CLI if the project can invoke the locked workspace/npm version. Project-local tooling reduces machine drift.

Developer data must use a separate configurable root and database, for example a debug profile under the repository ignored by Git or a per-test temporary directory. Development must never open a user's production Casklet database accidentally.

## 14. Environment and configuration

Configuration precedence:

```text
compiled safe defaults
< user config file
< explicit CLI flag / test injection
```

Environment variables are reserved for development, CI, and test overrides. The production GUI should not depend on hidden environment configuration.

Profiles:

```text
development
test
production
```

Each profile uses distinct application identifiers, database paths, storage roots, logs, and update channels. Test profile refuses to act outside explicitly allocated temporary roots unless a dedicated destructive integration test opts in.

No production secrets are required for normal local operation.

## 15. Git workflow

For a solo developer:

- `main` remains buildable and releasable.
- Use short-lived `codex/` or feature branches.
- Open pull requests for consequential changes, even when self-reviewing.
- Squash merge ordinary feature branches.
- Use semantic version tags such as `v0.1.0-alpha.1`.
- Keep commits cohesive and explain reasons, not only mechanics.
- Update the relevant ADR/design/context file with architectural changes.

Do not use GitFlow or maintain permanent `develop` and release branches for v0.1.

Branch protection becomes mandatory before accepting outside contributions or automating signed production releases.

## 16. Architecture Decision Records

Create ADRs using a small template:

```text
Title
Status: Proposed | Accepted | Superseded
Context
Decision
Consequences
Alternatives considered
```

Initial ADR set:

```text
0001-local-first-modular-monolith.md
0002-windows-first-cross-platform-ports.md
0003-rust-tauri-svelte.md
0004-sqlite-and-recovery-manifests.md
0005-durable-operation-engine.md
0006-plan-before-destruction.md
0007-no-persistent-privileged-service-v0.1.md
0008-provider-capability-model.md
```

ADRs summarize decisions; detailed design remains in architecture documents.

## 17. CI design

Use GitHub Actions initially.

### Pull-request jobs

#### Documentation

- Markdown lint.
- Mermaid validation.
- Internal-link validation.

#### Rust quality

- Format check.
- Clippy with warnings denied.
- Unit tests on Windows and Linux for platform-neutral crates.
- Dependency/license/source policy.
- Advisory scan.

#### Frontend quality

- Frozen pnpm install.
- TypeScript/Svelte check.
- ESLint.
- Prettier check.
- Vitest.
- Production frontend build.

#### Contract consistency

- Generate TypeScript and JSON Schemas.
- Fail if generated outputs differ from the committed tree.

#### Windows integration

- Build CLI and Tauri desktop.
- Run SQLite/filesystem integration tests using temporary roots.
- Run non-mutating fake-provider contracts.
- Do not install/uninstall real third-party applications.

### Scheduled/manual jobs

- Parser fuzzing.
- Link checking.
- Dependency drift report.
- Controlled disposable-machine WinGet smoke tests when infrastructure exists.
- Longer adversarial filesystem tests.

### CI security

- Pin third-party actions by commit SHA.
- Default workflow permissions to read-only.
- Grant only job-specific permissions.
- Do not expose secrets to fork pull requests.
- Cache dependencies by lockfile hash, not mutable branch name.

## 18. Release pipeline

Trigger from a protected semantic-version tag after a manual approval gate:

```text
clean checkout
-> verify version consistency
-> frozen dependency restore
-> quality and security checks
-> Windows x64 CLI build
-> Windows x64 Tauri build
-> installer packaging
-> SBOM generation
-> checksums
-> code signing when available
-> clean-machine smoke test
-> draft GitHub Release
-> manual publish
-> WinGet submission/update later
```

Initial artifacts:

```text
casklet-cli-v<version>-windows-x64.zip
Casklet-v<version>-windows-x64-setup.exe
SHA256SUMS
SBOM.spdx.json
release-notes.md
```

Use a per-user NSIS-style installer initially if Tauri's supported packaging and required CLI PATH integration test cleanly. MSI can be added for enterprise deployment later. Installer selection is confirmed during the packaging spike before the first public alpha.

No automatic updater ships until signed update metadata, channel separation, downgrade protection, and recovery have been tested.

## 19. Observability during development

- `tracing` spans around application commands, operation steps, provider invocations, and database transactions.
- Human-readable console logs in development.
- Structured rotating local logs in production.
- Operation IDs connect CLI/GUI messages to durable records and logs.
- Redaction occurs before data reaches any log sink.
- No remote telemetry by default.

CI retains failing test logs and artifacts after redaction. Never upload a developer's real Casklet database or personal paths as CI artifacts.

## 20. Documentation workflow

- `docs/PROJECT_CONTEXT.md` remains the concise cross-task handoff.
- `AGENTS.md` remains concise and points Codex to required context.
- Detailed architecture lives under `docs/architecture`.
- Security design lives under `docs/security`.
- Tooling and contributor workflows live under `docs/engineering`.
- Every architecture-changing PR updates an ADR and project context where relevant.
- User documentation must distinguish managed, referenced, shared, and estimated state in plain language.

## 21. Deliberate exclusions from the initial scaffold

- Web server/backend framework.
- Docker development requirement.
- Kubernetes or infrastructure-as-code.
- Cloud database SDK.
- Authentication library.
- Plugin runtime.
- Telemetry SDK.
- Full UI component system.
- CSS framework.
- ORM.
- Generic dependency-injection container.
- Automatic updater.
- Persistent Windows service.

These may be added only when a planned feature demonstrates the need.

## 22. Decisions proposed for approval

1. Use a single Cargo/pnpm monorepo.
2. Start with domain, contracts, application, infrastructure, CLI, and Tauri desktop crates/components.
3. Pin Rust 1.98.1, Node 24 LTS, pnpm 12, and a compatible Tauri 2.11 patch set at scaffolding time.
4. Use Svelte 5 with TypeScript/Vite and no SvelteKit server.
5. Use rusqlite with bundled SQLite and explicit repositories/migrations rather than an ORM.
6. Use Tokio selectively for long operations and bounded blocking work for SQLite/filesystem calls.
7. Generate and commit TypeScript IPC contracts from Rust definitions.
8. Use a Rust `xtask` for cross-platform developer commands.
9. Require lockfiles, dependency review, license/advisory checks, and pinned CI actions.
10. Use GitHub Actions and GitHub Releases initially.
11. Ship Windows x64 CLI ZIP and desktop installer artifacts with checksums and an SBOM.
12. Defer auto-update, service, cloud, plugin runtime, heavy UI frameworks, and real provider mutation in ordinary CI.

## Completion

Approved on 2026-09-14. Phase 8 defines the delivery roadmap in `docs/engineering/delivery-roadmap-v0.1.md`.
