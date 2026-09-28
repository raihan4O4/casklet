# Milestone 10 internal security and architecture review

Status: internal review only. Independent removal/elevation review remains a release gate.

| Boundary                   | Evidence                                                                                                     | Remaining gate                                                         |
| -------------------------- | ------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------- |
| CLI and GUI business rules | Both compose `CaskService<SqliteCasks, LocalStorage>`; no frontend filesystem or shell capability is granted | Manual Windows workflow parity                                         |
| Referenced/shared data     | Domain invariant and adversarial removal tests preserve references and changed identities                    | Independent destructive-code review                                    |
| Interrupted operations     | Durable journal and recovery tests cover forced exits and uncertain outcomes                                 | Power-loss/manual recovery matrix                                      |
| Database integrity         | Startup `quick_check` rejects corruption; migration creates a SQLite online backup before schema changes     | Validate low-disk and backup retention policy on real Windows profiles |
| Provider execution         | Typed WinGet/installer interfaces use direct process execution and exact evidence                            | Isolated UAC, installer, and provider tests                            |
| Installed profile          | Release binaries use per-user state outside the checkout; NSIS selects current-user installation             | Clean-machine install/uninstall preserves Casks                        |
| WebView                    | CSP and narrowly scoped Tauri capability are present                                                         | Manual CSP and accessibility validation                                |
| Supply chain               | Locked Rust and pnpm dependencies; release script emits checksums, provenance, SPDX SBOM                     | Signed publisher path, license review, candidate artifact inspection   |

This review does not certify absence of critical or high issues. The release must remain gated until an independent reviewer records findings, fixes are verified, and the manual matrix in `docs/engineering/milestone-10-release-gates.md` is complete.
