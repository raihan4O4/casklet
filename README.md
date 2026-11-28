<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="apps/desktop/src/assets/casklet-logo-white.png">
    <img src="apps/desktop/src/assets/casklet-logo-black.png" alt="Casklet" width="88">
  </picture>
</p>

<h1 align="center">Casklet</h1>

<p align="center"><strong>Keep every project’s files, apps, and cleanup together.</strong></p>

<p align="center">
  <img alt="Windows 11" src="https://img.shields.io/badge/Windows-11-0078D4?logo=windows11&logoColor=white">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-2024%20edition-000000?logo=rust&logoColor=white">
  <a href="LICENSE"><img alt="Apache 2.0 license" src="https://img.shields.io/badge/License-Apache--2.0-blue.svg"></a>
  <img alt="Developer Alpha" src="https://img.shields.io/badge/status-Developer%20Alpha-orange">
</p>

<p align="center">
  <a href="#getting-started">Get started</a> ·
  <a href="docs/product/user-guide.md">User guide</a> ·
  <a href="CONTRIBUTING.md">Contributing</a>
</p>

---

Casklet is a local-first lifecycle and resource manager for Windows. A **Cask** brings together the files, folders, and software connected to an activity, so you can understand what it uses, where it lives, and what may be safe to clean up.

Use the desktop app or CLI, backed by the same Rust core.

> **Developer Alpha · 0.1.0-alpha.1 · Windows 11 x64**
>
> Local Alpha candidates have been tested, but release checks for clean machines, installer behavior, and independent security review are still open. Casklet is not cleared for broad public distribution. Evaluate with disposable data; cleanup capabilities depend on the build profile.

## What you can do

- **Keep work in context.** Group project resources even when they live in different places.
- **See what’s there.** Review applications, files, activity, and storage estimates in a desktop workspace or CLI.
- **Make informed cleanup decisions.** Inspect a plan before action; shared, ambiguous, or unverified resources are preserved.
- **Take your inventory with you.** Export readable reports and JSON inventories.

### Built around your control

Association does not imply ownership. Referenced items stay yours, and destructive actions require a reviewable, revalidated plan. Casklet is a management tool, not an application sandbox or a guarantee of complete cleanup for arbitrary installers.

## Getting started

Build prerequisites: Windows 11 x64, Git, Rustup, Node.js 24, Corepack, Microsoft C++ Build Tools, and the WebView2 Runtime.

```powershell
git clone --branch develop https://github.com/raihan4O4/casklet.git
cd casklet
corepack pnpm install --frozen-lockfile
./scripts/bootstrap.ps1
corepack pnpm --filter @casklet/desktop tauri dev
```

To build the CLI:

```powershell
cargo build -p casklet-cli --locked
```

Start with references when exploring. Cleanup is profile-gated and should only be evaluated with disposable data; see the [Alpha readiness notes](docs/engineering/alpha-readiness.md).

## Project docs

| Guide | What it covers |
| --- | --- |
| [User guide](docs/product/user-guide.md) | Workflows, storage, safety, and recovery |
| [CLI walkthrough](docs/engineering/cli-terminal-ux.md) | Commands and examples |
| [System design](docs/architecture/system-design-v0.1.md) | Architecture and data flow |
| [Release gates](docs/engineering/milestone-10-release-gates.md) | Current release checks |
| [Security policy](SECURITY.md) | Private vulnerability reporting |

## Development

Casklet is a Rust workspace with a Tauri 2 desktop shell and Svelte/TypeScript UI. Run the shared local checks with:

```powershell
cargo xtask check
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for repository guidance. Please redact personal paths and diagnostics before sharing them in an issue.

## License

[Apache License 2.0](LICENSE). See [NOTICE](NOTICE) and [third-party notices](LICENSES/README.md).
