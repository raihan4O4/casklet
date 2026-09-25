# Casklet

Casklet is a local-first lifecycle and resource manager for Windows 11 x64. Milestone 1 provides persistent empty Casks through the CLI and desktop.

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

## Milestone 1 CLI

The user-facing command is `casklet`. Build the development CLI with `cargo build -p casklet-cli`, then run `.\target\debug\casklet.exe` from this checkout. Cargo's `cargo run -p casklet-cli -- ...` form is only a development launcher for the same executable. Release packaging will validate adding `casklet.exe` to `PATH`.

```powershell
.\target\debug\casklet.exe cask create "My Work"
.\target\debug\casklet.exe cask list
.\target\debug\casklet.exe cask list --output json
.\target\debug\casklet.exe cask get <id>
```

The default list output contains only Cask names. JSON output explicitly includes IDs and metadata for scripting or disambiguating duplicate names. The current binary uses development data paths and is not yet an installable release.

See [CONTRIBUTING.md](CONTRIBUTING.md), [project context](docs/PROJECT_CONTEXT.md), and the [delivery roadmap](docs/engineering/delivery-roadmap-v0.1.md).
