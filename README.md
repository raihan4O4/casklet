# Casklet

Casklet is a local-first lifecycle and resource manager for Windows 11 x64. The repository is currently at Milestone 0: a reproducible, no-feature foundation.

## Bootstrap

Install Git, Rustup, Node.js 24 LTS, Microsoft C++ Build Tools, WebView2, and WinGet. Then run:

```powershell
corepack enable
pnpm install --frozen-lockfile
./scripts/bootstrap.ps1
cargo xtask check
```

The test profile uses temporary directories. Development data is confined to `.casklet-dev/` and is never shared with a production profile.

See [CONTRIBUTING.md](CONTRIBUTING.md), [project context](docs/PROJECT_CONTEXT.md), and the [delivery roadmap](docs/engineering/delivery-roadmap-v0.1.md).
