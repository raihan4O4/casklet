# ADR 0017 — Separate Windows installer and data paths

Status: Accepted on 2026-09-28

## Context

The per-user NSIS installer uses `%LOCALAPPDATA%\Casklet` as its program directory. A local installation test found that the original release profile also placed the SQLite database, backups, and metadata there. Uninstall could therefore remove user state alongside the program. The test install held no Cask data and was removed.

## Decision

Place release operational state in `%LOCALAPPDATA%\CaskletData` and default managed Cask content in `%USERPROFILE%\Casklets`. Keep the installer program directory at `%LOCALAPPDATA%\Casklet`. Do not silently migrate or delete files found in the former data path: it is an installer-controlled directory and cannot be treated as authoritative user state. No public version has used the former path.

## Consequences

Uninstall and upgrades must preserve both data roots. The corrected candidate requires an install, create, uninstall, and preservation test on the local host, followed by clean-machine verification before public release. A regression test fixes the root relationship in the Windows adapter.
