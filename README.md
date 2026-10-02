# Casklet

Casklet is a local-first lifecycle and resource manager for Windows 11 x64. The CLI and desktop share the same Rust core. The release gate remains open; see the [Milestone 10 checklist](docs/engineering/milestone-10-release-gates.md) and [user guide](docs/product/user-guide.md).

## Bootstrap

Install Git, Rustup, Node.js 24 LTS, Microsoft C++ Build Tools, and WebView2. WinGet is needed for later provider development. The bootstrap script finds Rust in the standard Rustup directory and uses the repository-pinned pnpm through Corepack.

Then run:

```powershell
corepack pnpm install --frozen-lockfile
./scripts/bootstrap.ps1
cargo xtask check
```

Run `./scripts/bootstrap.ps1` in each new PowerShell session if Cargo is not already on `PATH`.

The test profile uses temporary directories. Debug builds keep development data in `.casklet-dev/`. Release builds use `%LOCALAPPDATA%\\CaskletData` for metadata and `%USERPROFILE%\\Casklets` for managed content. Neither a database backup nor uninstalling the application is a substitute for backing up managed files.

## Milestone 1 CLI

The user-facing command is `casklet`. Build the development CLI with `cargo build -p casklet-cli`, then run `.\target\debug\casklet.exe` from this checkout. Cargo's `cargo run -p casklet-cli -- ...` form is only a development launcher for the same executable. Release packaging will validate adding `casklet.exe` to `PATH`.

```powershell
.\target\debug\casklet.exe cask create "My Work"
.\target\debug\casklet.exe cask list
.\target\debug\casklet.exe cask list --output json
.\target\debug\casklet.exe cask get <id>
```

The default list output contains only Cask names. JSON output explicitly includes IDs and metadata for scripting or disambiguating duplicate names. Debug binaries use development data paths; Alpha release binaries use the per-user production paths above.

See [CONTRIBUTING.md](CONTRIBUTING.md), [project context](docs/PROJECT_CONTEXT.md), and the [delivery roadmap](docs/engineering/delivery-roadmap-v0.1.md).

## Alpha candidate

Version: 0.1.0-alpha.1. See [Alpha notes](docs/product/alpha-release-notes.md) and [readiness evidence](docs/engineering/alpha-readiness.md). Default builds support previews only. To test actual file cleanup locally:

```powershell
cargo build -p casklet-cli --features alpha-cleanup
corepack pnpm --filter @casklet/desktop tauri dev --features alpha-cleanup
```

To package the Windows x64 candidate, set `SYFT_PATH` to a checksum-verified official Syft executable, then run `cargo xtask package-alpha`. Local validation of uncommitted work can explicitly use `cargo xtask package-alpha --allow-dirty`; provenance records exact source checksums and dirty status. Verify with `pwsh -NoProfile -File scripts/verify-release.ps1 -OutputDirectory release/alpha`. `scripts/alpha-smoke.ps1` tests real cleanup only on newly created isolated workspace fixtures. Independent review, signing, and clean-machine gates remain outstanding; no public release is claimed.
