# Contributing

Read `AGENTS.md` and `docs/PROJECT_CONTEXT.md` before making changes. Keep `main` buildable, use short-lived `codex/` or feature branches, and make one reviewable behavior change per pull request.

## Local gate

```text
cargo xtask bootstrap-check
cargo xtask check
```

`check` formats/checks Rust, runs Clippy and tests, checks the frontend, and verifies generated contracts. Add only dependencies required by the current milestone and document consequential architecture changes with an ADR.

Never test destructive filesystem or provider behavior against real user data. Use the test profile and explicitly allocated temporary roots.
