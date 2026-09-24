# Milestone 1 implementation notes

The first vertical slice creates, lists, and inspects empty Casks through the same Rust application service from the CLI and Tauri commands. The domain owns validation: names are trimmed, must contain 1–120 characters, and cannot contain control characters. Duplicate names are allowed because Casks have stable UUID identities.

The development database and Cask storage live under the ignored `.casklet-dev` directory; `CASKLET_DEV_ROOT` can override that root for isolated CLI tests. Tests inject temporary paths. SQLite opens with foreign keys, WAL, a finite busy timeout, and full synchronous writes. The versioned first migration creates a strict Cask table. Each Cask gets a `content` directory and a versioned empty JSON manifest with a SHA-256 checksum in `.casklet`.

After building or installing the CLI, the user-facing commands are:

```text
casklet cask create "My Work"
casklet cask list
casklet cask get <id>
```

During development, `cargo run -p casklet-cli -- cask list` runs the same CLI through Cargo. Cargo builds the binary named `casklet`; after `cargo build -p casklet-cli`, it can also be invoked directly from `target/debug/casklet.exe`. Release packaging will validate adding it to `PATH` so `casklet` works from any terminal.

The CLI defaults to concise human output. `casklet cask list` prints one Cask name per line; it does not reveal IDs or paths. `--output json` explicitly requests structured output, including IDs needed to select a Cask for `get` when names are duplicated. `cargo xtask generate-contracts` writes the TypeScript contract consumed by the desktop. `cargo xtask check` runs the shared quality gate.

The Cask table and manifest are intentionally limited to the empty-Cask slice. Resource and operation data arrives with later milestones; a manifest alone never grants deletion authority. Durable pending-projection retry is scheduled with the operation/recovery work in Milestone 3. The Windows quality gate passed locally on 2026-09-25, including domain, migration/restart, CLI process, and GUI component tests.
