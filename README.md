# Casklet

Casklet is a local-first lifecycle and resource manager for Windows 11 x64. Milestone 0 established the reproducible foundation; Milestone 1 is the next implementation milestone.

## Bootstrap

Install Git, Rustup, Node.js 24 LTS, Microsoft C++ Build Tools, and WebView2. WinGet is needed for later provider development. The bootstrap script finds Rust in the standard Rustup directory and uses the repository-pinned pnpm through Corepack.

Then run:

```powershell
corepack pnpm install --frozen-lockfile
./scripts/bootstrap.ps1
cargo xtask check
```

Run `./scripts/bootstrap.ps1` in each new PowerShell session if Cargo is not already on `PATH`.

The test profile uses temporary directories. Development data is confined to `.casklet-dev/` and is never shared with a production profile.

See [CONTRIBUTING.md](CONTRIBUTING.md), [project context](docs/PROJECT_CONTEXT.md), and the [delivery roadmap](docs/engineering/delivery-roadmap-v0.1.md).
