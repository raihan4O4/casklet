# ADR 0015: Installer artifacts yield review candidates, not ownership

Status: Implemented for Milestone 9.

## Context

An MSI or EXE can change global Windows state, spawn child processes, request elevation, require a reboot, or return a misleading exit code. Filename, product name, signer, and nearby registry changes cannot prove which installation an arbitrary installer created.

## Decision

Inspect an explicitly chosen MSI or EXE without running it. Record its SHA-256, size, native Windows Authenticode signer/status evidence, source path, and available product hints. Read MSI ProductCode and properties through the Windows Installer COM database in read-only mode. Store a verified copy as a managed Cask resource. Require a matching hash and explicit risk acknowledgement before launching the copy.

Use a full executable path and typed process arguments. Immediately before launch, hold a read-only Windows file handle that denies writes and renames, and hash that handle against the reviewed copy. MSI goes through system `msiexec.exe /i ... /norestart`; EXE uses its exact path with no inferred switches. Optional EXE elevation launches only the installer through a visible Windows UAC prompt; the Casklet GUI stays unelevated. A fixed PowerShell script used for inspection and elevation receives paths through environment variables, never interpolated command text. Its module path is restricted to Windows system modules.

Journal intent and lock installation mutations before taking the pre-launch inventory snapshot. Compare complete Windows uninstall-registration snapshots after launch and offer each new registration as a candidate with explicit evidence and confidence. MSI ProductCode match is stronger than name or publisher matching. Candidate confirmation rechecks exact registry evidence and records only a detach-only association. It never grants uninstall ownership. Ambiguity, EXE child-process uncertainty, nonzero exit, timeout, cancellation, missing inventory, and reboot-required exit all preserve a recovery or review state.

## Consequences

Some installers produce no candidate or multiple candidates. They remain unowned. Casklet cannot know whether an EXE's descendants completed, and killing a parent does not guarantee that descendants stopped. An uncertain operation retains its lock until a user inspects and closes it or confirms a current candidate. Real installer and UAC behavior needs isolated Windows validation before public release.
