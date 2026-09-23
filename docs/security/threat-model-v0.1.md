# Phase 6 — Security Threat Model for Casklet v0.1

Status: Approved on 2026-09-14

Last reviewed: 2026-09-14

## 1. Purpose

Casklet installs software, downloads files, records system state, and removes resources. A defect or abuse in these areas can execute untrusted code, expose private data, or delete unrelated user content. Security is therefore part of the product model, not a final hardening pass.

This document defines:

- What must be protected.
- Which actors and failures are in scope.
- Where trust changes across the system.
- Concrete threats and required mitigations.
- Residual risks Casklet must communicate honestly.
- Security tests and the v0.1 release gate.

The model follows a practical STRIDE-style review—spoofing, tampering, repudiation, information disclosure, denial of service, and elevation of privilege—organized around Casklet workflows rather than abstract categories.

## 2. Security objectives

In priority order:

1. Never delete or uninstall a resource without sufficiently verified authority and identity.
2. Never turn untrusted filenames, manifests, URLs, provider output, or metadata into arbitrary code execution.
3. Minimize privileges and keep privileged actions explicit and narrow.
4. Preserve sufficient operation state to diagnose and recover partial work.
5. Prevent the WebView from becoming a general bridge to native system capabilities.
6. Protect release integrity and reduce dependency/build supply-chain risk.
7. Minimize collection and exposure of users' paths, application inventories, and file metadata.
8. Fail closed for destructive actions and fail recoverably for ordinary operations.

Availability matters, but preservation outranks convenience: when Casklet cannot establish safety, it may refuse cleanup and ask the user to review the item.

## 3. Assets

### Highest-value assets

- User files inside and outside managed Cask storage.
- Installed applications and shared dependencies.
- Filesystem roots, home/profile directories, application-data directories, and other broad paths.
- Removal plans and their approvals.
- Resource ownership, provenance, containment, and sharing records.
- Privileged execution boundary.
- Casklet release/update signing keys.

### Sensitive supporting assets

- SQLite database and backups.
- Per-Cask recovery manifests.
- Operation and recovery journal.
- Provider identities and installer hashes.
- Local file paths and installed-application inventory.
- Logs and diagnostic bundles.
- Source repository, CI workflows, package locks, and release artifacts.

### Lower-value but security-relevant assets

- GUI state and progress events.
- Cached metadata and thumbnails.
- Download staging directories.
- Configuration and user preferences.

## 4. Threat actors and failure sources

### In scope

- A malicious installer or portable application selected by the user.
- A malicious or compromised download URL.
- A malicious imported Casklet manifest or archive.
- Crafted filenames, paths, symbolic links, Windows junctions, reparse points, hard links, and race conditions.
- Incorrect or malicious output from an external provider or executable.
- A low-privilege local process racing or modifying files accessible to the same user.
- A compromised dependency, CI action, package registry account, release host, or update feed.
- Cross-site scripting or compromised frontend code attempting to invoke native commands.
- Database corruption, manifest tampering, power loss, crashes, and partial operations.
- Accidental user actions and misunderstood removal scope.
- A future or downgraded Casklet version reading incompatible state.

### Partially in scope

- Another process running as the same OS user. Casklet will revalidate destructive targets and protect its own data with normal user permissions, but cannot provide a strong security boundary from a fully compromised same-user session.
- Malware with ordinary user privileges. Casklet should not amplify its authority, but it is not an antivirus or sandbox.

### Out of scope for v0.1

- An attacker with administrator/kernel control of the machine.
- Physical attacks against an unlocked device.
- Protecting user files from a fully compromised OS account.
- Determining that arbitrary third-party software is benign.
- Containing software after Casklet launches it.
- Enterprise multi-user authorization and policy administration.

These exclusions do not permit unsafe design; they define what Casklet cannot credibly guarantee.

## 5. Trust boundaries

```text
Untrusted user input / dropped files / manifests / URLs
                        |
                        v
              Svelte UI or CLI parser
                        |
               typed validation boundary
                        v
                 Application core
              policy and authorization
                /       |        \
               v        v         v
          Filesystem   SQLite    Providers/processes
          untrusted    trusted   externally controlled
          reality      state*    behavior/output
               \        |         /
                \       v        /
                 Removal executor
                        |
                 OS privilege boundary
```

`*` SQLite is trusted for Casklet's recorded state only after integrity and schema validation. It is not proof of current filesystem identity or removal safety.

Important boundaries:

1. WebView JavaScript to Tauri/Rust IPC.
2. CLI text arguments to typed application requests.
3. Application core to filesystem state.
4. Application core to SQLite and manifests.
5. Application core to WinGet, installers, uninstallers, and portable applications.
6. Unelevated process to UAC-elevated action.
7. Network to download staging.
8. CI/release systems to installed Casklet binaries.
9. Imported/exported data to internal domain state.

Tauri explicitly treats the WebView and Rust core as different trust groups; IPC capabilities and command-level scopes are the bridge. Native/plugin code can access system resources, so frontend access must remain narrow. [Tauri security model](https://v2.tauri.app/security/)

## 6. Security authority model

Casklet authorizes destructive action only when all required evidence agrees.

### File/directory removal eligibility

```text
approved exact removal plan
AND plan is current and unexpired
AND Cask/Resource revisions still match
AND Resource relationship permits deletion
AND no retaining associations remain
AND current target is the expected filesystem object
AND containment/path boundary is revalidated
AND action remains within the approved scope
```

### Application uninstall eligibility

```text
approved exact removal plan
AND association == INSTALLED_FOR_CASK
AND provenance == INSTALLED_BY_CASKLET
AND no retaining Cask associations remain
AND provider supports uninstall
AND provider identity is currently verified
AND installed instance still matches the approved snapshot
```

Neither a high detection score nor an installer exit code grants ownership.

### Approval properties

- Approval references a server/core-generated plan ID and exact canonical plan hash.
- Approval is stored in SQLite with time and plan revision.
- Plan hashes provide integrity/equality, not authenticity against a local attacker who can rewrite the database.
- Execution loads the plan from trusted persistence; the GUI cannot submit arbitrary plan actions.
- A changed target invalidates the item or whole plan.
- Approval does not survive material revisions, expiry, restart recovery uncertainty, or provider identity changes.

## 7. Filesystem threats and controls

### T1: Path traversal

Threat: A manifest, archive, filename, or CLI argument uses `..`, rooted paths, device paths, alternate separators, UNC paths, or encoded forms to escape its intended directory.

Required controls:

- Accept paths as native path objects, not URL/string concatenations.
- Reject unexpected absolute paths in portable imports and archive entries.
- Resolve destination paths under an already verified root.
- Verify components after normalization using platform-aware rules.
- Reject Windows device namespaces and reserved device names where not explicitly supported.
- Treat path comparison as platform-specific.
- Test Unicode, trailing dots/spaces, case differences, extended-length paths, and alternate data streams.

### T2: Junction, symlink, mount, or reparse-point escape

Threat: A path validated inside a Cask is replaced with a link to an external directory before recursive deletion or write.

Required controls:

- Do not follow links, junctions, or mount transitions during ordinary scans by default.
- Record link/reparse metadata during inspection.
- Walk destructive targets without following links.
- Reinspect every path component immediately before action.
- Compare volume/filesystem identity where available.
- Delete a link itself, never its external target, unless an independently approved external action exists.
- Abort when identity changes between plan and execution.

### T3: Time-of-check/time-of-use race

Threat: Another process swaps a validated target after inspection.

Required controls:

- Minimize time between final validation and action.
- Use handle-relative or identity-preserving filesystem APIs where platform support permits.
- Record expected filesystem identity in plan preconditions.
- Revalidate after opening and before recursive traversal.
- Treat unverifiable targets as blocked.

### T4: Broad or catastrophic deletion target

Threat: A bug resolves a Resource to a drive root, user profile, Casklet data root, or parent containing unrelated files.

Required controls:

- Maintain an explicit denylist of protected broad roots plus structural root checks.
- Require a verified Resource identity; raw paths alone cannot reach recursive deletion.
- Refuse empty, relative-at-execution, filesystem-root, profile-root, application-data-root, or unresolved targets.
- Limit each plan item to one exact target.
- Never use wildcard expansion for deletion.
- Prefer platform trash for eligible user content.
- Require stronger confirmation for external permanent deletion.

### T5: Hard-link and shared-content confusion

Threat: Storage is double-counted or deletion is claimed to reclaim bytes still referenced elsewhere.

Required controls:

- Do not treat hashes or paths as proof of exclusive physical storage.
- Track filesystem identities when available.
- Report reclaimability conservatively.
- Do not implement automatic deduplication or hard-link optimization in v0.1.

### T6: Partial copy or move

Threat: A crash leaves incomplete data that appears final, or deletes the source before the destination is valid.

Required controls:

- Copy into operation-specific staging.
- Verify size and requested hash before finalization.
- For cross-volume move, finalize and durably record destination before source removal.
- Never overwrite an unrelated destination silently.
- Preserve operation-owned staging for bounded recovery.

## 8. Archive and portable-application threats

### T7: Archive traversal (Zip Slip)

Required controls:

- Inspect every archive entry before extraction.
- Reject absolute paths and parent traversal.
- Reject entries resolving outside the extraction root.
- Define safe handling for links; default to rejecting them in v0.1 portable packages.
- Never trust a common prefix string comparison.

### T8: Decompression bomb and resource exhaustion

Required controls:

- Limit entry count, total expanded bytes, path depth, and compression ratio.
- Check free space with a safety margin.
- Allow cancellation between entries.
- Extract in staging and clean only operation-owned staging paths.
- Make limits configurable only within safe global ceilings.

### T9: Executable masquerading as media/data

Required controls:

- Detect by content plus extension where practical.
- Display executable status explicitly.
- Never execute as a consequence of add, scan, preview, or classification.
- Require a dedicated installation/launch workflow.

## 9. Installer and provider threats

### T10: Malicious installer execution

Threat: An installer legitimately has permission to modify the system but is malicious.

Required controls:

- Clearly state that Casklet does not sandbox or certify third-party installers.
- Display source, provider, publisher/signature state, requested scope, and hash when known.
- Require explicit user initiation.
- Never run an installer merely because it was dropped into a Cask.
- Prefer trusted package-manager/provider flows over arbitrary installer files.
- Preserve Windows security prompts and OS reputation checks.

### T11: Signature interpreted as safety

Threat: A valid Authenticode signature is presented as proof that software is benign.

Required controls:

- Treat signature verification as publisher/integrity evidence, not malware approval.
- Distinguish valid, invalid, unsigned, expired/revocation-unknown, and unsupported states.
- Use native Windows trust-policy verification rather than home-grown certificate checks. Windows exposes Authenticode trust verification through its trust provider APIs. [Microsoft WinVerifyTrust documentation](https://learn.microsoft.com/en-us/windows/win32/api/wintrust/nf-wintrust-winverifytrustex)
- Do not suppress SmartScreen or UAC warnings.

### T12: Provider spoofing or executable hijacking

Threat: Casklet runs a malicious `winget.exe` or helper found earlier on `PATH`.

Required controls:

- Resolve providers through trusted platform discovery.
- Record and verify resolved executable identity/location.
- Avoid current-directory executable resolution.
- Pass arguments as an array, never through a shell command string.
- Set a minimal, controlled working directory and environment.
- Reject provider identifiers that cannot be represented safely as arguments.
- Capture bounded output and handle invalid encoding.

### T13: Malicious provider output

Required controls:

- Treat stdout/stderr and metadata as untrusted text.
- Prefer structured, versioned provider interfaces where available.
- Escape output in terminal and GUI rendering.
- Cap output size and line length.
- Never interpret returned strings as commands or paths without independent validation.

### T14: Package identity confusion

Threat: A similarly named package is associated with or uninstalls the wrong application.

Required controls:

- Use platform/provider/package ID as identity, not display name.
- Show publisher/source before installation.
- Verify installed provider ID and current version afterward.
- Require user confirmation for inventory-correlation candidates.
- Never grant ownership from fuzzy-name matching.

### T15: Compromised package source or changed installer

Required controls:

- Retain provider source and installer hash when available.
- Do not disable provider certificate or integrity validation.
- Pin exact provider source where the API permits.
- Treat redirects and source changes as new evidence requiring validation.
- Record that WinGet can generate and use SHA-256 installer metadata, while recognizing that Casklet still relies on provider security and repository integrity. [WinGet documentation](https://learn.microsoft.com/en-us/windows/package-manager/winget/)

### T16: Installer child process and ambiguous completion

Required controls:

- Do not assume parent exit means installation completed.
- Use provider-specific completion semantics.
- Compare verified inventory after completion.
- Represent unresolved or reboot-dependent outcomes explicitly.
- Do not offer automatic uninstall for unresolved installations.

## 10. Privilege and elevation threats

### T17: Whole application runs elevated

Required controls:

- GUI and normal CLI run unelevated.
- Do not mark the main executable as requiring administrator privileges.
- Trigger ordinary OS/UAC elevation only for one specific provider action.
- Do not keep privileged state after the action completes.

### T18: Command injection across elevation boundary

Required controls:

- No free-form elevated command API.
- The privileged side, if later required, accepts a versioned allowlisted request type.
- It independently validates provider, package ID, paths, plan ID, nonce, and action.
- It never accepts shell script text, raw uninstall strings, or arbitrary executable paths from the WebView.
- Responses are authenticated to the initiating operation where practical.

### T19: Privileged confused deputy

Threat: An unprivileged caller tricks Casklet into deleting or modifying something it could not otherwise access.

Required controls:

- Avoid a persistent privileged service in v0.1.
- Never expose a broad IPC endpoint to other local processes.
- Recheck authorization and identity after elevation.
- Bind elevation to one approved operation and expire it.
- Keep elevated actions provider-specific and minimal.

## 11. Tauri and frontend threats

Tauri recommends treating lifecycle and dependency security as a whole: compromised frontend code, dependencies, build systems, hosting, or update infrastructure can undermine runtime protections. It also recommends capabilities, CSP, dependency auditing, pinned CI actions, and signed binaries. [Tauri application lifecycle threats](https://v2.tauri.app/security/lifecycle/)

### T20: XSS invokes native capabilities

Required controls:

- Bundle frontend code locally; load no remote scripts.
- Configure a restrictive production Content Security Policy.
- Avoid `unsafe-eval`; avoid `unsafe-inline` where feasible.
- Do not render filenames, provider output, Markdown, or metadata with raw HTML.
- Sanitize any future rich-text rendering.
- Keep dependencies minimal and audited.

Tauri's CSP is opt-in configuration and should be restricted to required sources; its documentation recommends avoiding remote content and CDN scripts. [Tauri CSP guidance](https://v2.tauri.app/security/csp/)

### T21: Overbroad Tauri capabilities

Required controls:

- Explicitly enable only named capability files.
- Grant permissions per window and command.
- Do not expose generic shell, unrestricted filesystem, or generic HTTP capabilities to the main window.
- Validate authorization again inside sensitive Rust commands.
- Separate any future untrusted preview window into a capability set with no destructive/native commands.

### T22: Local file preview exposes arbitrary paths

Required controls:

- Do not grant the asset protocol broad home-drive access.
- Expose only a short-lived, explicitly selected resource path or generated thumbnail.
- Validate the Resource association before preview.
- Avoid inline rendering of active HTML/SVG or other script-capable formats in v0.1.
- Treat media decoders as an attack surface and keep WebView/OS patched.

Tauri's asset protocol requires configured path scopes; broad glob scopes would undermine Casklet's frontend boundary. [Tauri asset protocol scope](https://v2.tauri.app/security/asset-protocol/)

### T23: Forged or replayed IPC requests

Required controls:

- Tauri command accepts typed logical requests, never executable plan items.
- Backend loads authoritative state by ID.
- Destructive calls require current plan ID and stored approval.
- Enforce state transitions and idempotency server/core-side.
- Replayed completed operations return existing results or fail safely.

## 12. Manifest and database threats

### T24: Malicious imported manifest

Required controls:

- Treat every imported manifest as untrusted.
- Enforce maximum file size, nesting depth, collection count, and string length.
- Validate against the exact supported schema before domain conversion.
- Reject unknown future major versions.
- Resolve internal relative paths only beneath an import-selected root.
- Never import executable uninstall commands, privileged actions, locks, or approval state.
- Import creates a preview; it does not immediately install or delete anything.

### T25: Checksum mistaken for authenticity

Required controls:

- Document that `manifest.sha256` detects accidental corruption and mismatched files, not malicious modification by someone who can replace both files.
- Future signed shared manifests require a distinct signature format and trust policy.
- Recovered ownership-sensitive information remains unverified until observed externally.

### T26: Database tampering or corruption authorizes deletion

Required controls:

- Run schema/integrity checks before destructive operations when database health is uncertain.
- Revalidate paths, identity, associations, and provider state outside stored claims.
- Recompute plan hash from canonical loaded plan content.
- Enter read-only recovery mode after integrity failure.
- Preserve the damaged database and avoid destructive repair guesses.

### T27: Migration compromises safety metadata

Required controls:

- Immutable checksummed migrations.
- Pre-migration SQLite backup.
- Tests from every released schema.
- Post-migration invariants for ownership, associations, plan state, and paths.
- Abort and preserve backup on failure.
- Reject running an older application against a newer unsupported schema.

## 13. Download and network threats

### T28: Unsafe URL schemes or redirect targets

Required controls:

- Support HTTP and HTTPS only in v0.1.
- Reject `file:`, `javascript:`, `data:`, custom protocols, UNC paths, and embedded credentials.
- Revalidate every redirect.
- Do not pass downloads through the WebView.
- Store redacted URLs.
- Warn before plain HTTP.
- Decide separately whether private/LAN addresses are allowed; local-first users may legitimately need them, so v0.1 should warn and require confirmation rather than silently block all private addresses.

### T29: Oversized or endless download

Required controls:

- Enforce configurable maximums and available-space checks.
- Do not trust `Content-Length` alone.
- Stream to staging with a hard byte ceiling.
- Apply connection, inactivity, and total-duration policies.
- Allow safe cancellation.

### T30: Filename/header injection

Required controls:

- Sanitize server-provided filenames.
- Strip path components and control characters.
- Resolve conflicts explicitly.
- Never use response filenames in shell commands.
- Render URLs and headers as escaped text.

## 14. Denial-of-service threats

### T31: Huge directory tree or cyclic topology

Required controls:

- Bounded concurrency and memory.
- Batched database writes.
- Maximum traversal depth and configurable item thresholds.
- Do not follow links by default.
- Cancellation checkpoints.
- Progressive UI rather than blocking startup.

### T32: Database/log growth

Required controls:

- Rotate local logs by size/count.
- Bound provider-output persistence.
- Expire old nonessential plan/progress detail through explicit retention policy.
- Keep tombstone/audit retention visible to users.
- Provide storage diagnostics before automated purging is introduced.

### T33: Lock denial or abandoned operation

Required controls:

- Persistent lock owner and heartbeat.
- Verify owner liveness before recovery.
- Never steal a destructive lock solely because time elapsed.
- Provide an inspectable recovery path.
- Bound waits and return `OPERATION_CONFLICT`.

## 15. Privacy and information-disclosure threats

### T34: Logs disclose paths, tokens, or inventory

Required controls:

- Structured logging with field-level redaction.
- Never log file contents, authorization headers, cookies, environment-variable dumps, or raw signed URLs.
- Separate user-facing diagnostics from verbose developer traces.
- Diagnostic bundle preview before export.
- Default local retention limits.

### T35: Shared export leaks machine details

Required controls:

- Shared export is distinct from recovery manifest.
- Replace internal managed paths with relative paths.
- Redact usernames, profile paths, machine IDs, volume IDs, environment variables, and sensitive query parameters.
- Show an export preview and warning for explicitly requested absolute paths.

### T36: UI exposes private thumbnails/content

Required controls:

- Do not create thumbnails unless required for the view.
- Store generated thumbnails in Casklet cache with user-only permissions.
- Clear bounded cache using exact cache ownership.
- Avoid background cloud thumbnail or metadata services.

## 16. Release and supply-chain threats

### T37: Compromised dependency

Required controls:

- Keep Rust and npm dependency counts low.
- Commit lockfiles.
- Run `cargo audit`, `cargo deny`, and appropriate npm/pnpm auditing in CI.
- Review new production dependencies for maintenance, ownership, license, transitive size, native code, and security history.
- Avoid install-time scripts where feasible.
- Generate an SBOM for releases.

### T38: Compromised CI action or workflow

Required controls:

- Pin third-party actions to commit SHAs.
- Grant least-privilege workflow permissions.
- Do not expose release secrets to pull-request workflows.
- Require manual approval for publishing stable releases.
- Protect release tags and main branch.
- Retain build provenance and checksums.

### T39: Signing-key compromise

Required controls:

- Keep signing material outside the repository.
- Prefer hardware-backed or managed signing when financially practical.
- Restrict signing to protected release workflows.
- Document key rotation and revocation.
- Separate development/test signing from production signing.

### T40: Malicious update or downgrade

Required controls:

- No auto-update until signed update verification and rollback behavior are tested.
- Verify signed updater metadata and artifact identity.
- Use HTTPS but do not rely on transport encryption alone.
- Prevent silent downgrade unless a separately approved recovery workflow exists.
- Do not execute updates from user-supplied manifests.

## 17. Security severity model

Each security issue is rated by likelihood and impact:

```text
Likelihood: LOW / MEDIUM / HIGH
Impact:     LOW / MEDIUM / HIGH / CRITICAL
Priority:   P0 / P1 / P2 / P3
```

- P0: credible loss of unrelated data, arbitrary privileged execution, or release-signing compromise. Blocks release immediately.
- P1: credible code execution, destructive-scope bypass, significant sensitive-data exposure, or update/provider identity compromise. Blocks release.
- P2: meaningful weakness requiring specific conditions, recoverable integrity issue, or limited privacy leak. Must be fixed or explicitly accepted before stable release.
- P3: defense-in-depth improvement or low-impact issue. Track with rationale and target release.

Residual acceptance must record owner, reason, affected version, mitigation, and review date. “Unlikely” is not sufficient justification for accepting a destructive-data risk.

## 18. Required security tests

### Domain/property tests

- Referenced resources never produce automatic delete actions.
- Shared resources remain while retaining associations exist.
- Detection confidence alone never grants uninstall ownership.
- Plan mutation invalidates approval.
- Revision or identity changes invalidate destructive actions.
- Corrupt/unknown state produces preservation or blocking.

### Filesystem adversarial tests

- Parent traversal and absolute archive paths.
- Windows junctions and symlinks at every directory level.
- Target swapping between planning and execution.
- Drive roots, UNC paths, device namespaces, alternate data streams, reserved names, Unicode, case changes, and trailing dots/spaces.
- Hard links and shared files.
- Inaccessible children and partial traversal.
- Cross-volume interrupted moves.

### Parser/fuzz tests

- Manifest parser and version migration.
- Archive-entry path validation.
- Provider structured-output parser.
- Redaction and URL parsing.
- Tauri command DTO validation.

### Process/provider tests

- Arguments containing quotes, spaces, metacharacters, and Unicode never invoke a shell.
- Provider executable resolution cannot use the current directory.
- Oversized/malformed output is bounded.
- Exit success plus failed verification does not become installed/uninstalled.
- Elevation requests cannot represent arbitrary commands.

### Frontend tests

- Filenames and provider output containing HTML/script are rendered as text.
- Untrusted SVG/HTML is not previewed actively.
- Windows cannot invoke commands outside their capability set.
- Removal UI displays retained/shared/referenced items accurately.

### Release tests

- Release artifacts match published checksums.
- Signatures and updater metadata validate.
- SBOM is generated.
- Dependency and license checks pass.
- Clean-machine install and uninstall are tested.

## 19. Security implementation requirements

Before feature implementation, create:

- `SECURITY.md` with private vulnerability-reporting instructions.
- A secure-coding checklist for filesystem and provider adapters.
- Central redaction utilities.
- Central path-safety and target-verification services.
- A typed provider process runner that cannot invoke a shell.
- A canonical removal-plan serializer/hash implementation.
- Test helpers for adversarial filesystems and fake providers.

Security-critical code should remain small, explicit, and reviewed separately. Avoid “utility” functions that combine path resolution, permission changes, and deletion.

## 20. Security release gate for v0.1

Casklet v0.1 cannot be called production-ready until:

- No open P0 or P1 security issues exist.
- Every destructive operation uses the approved planner/executor path.
- All path and filesystem adversarial tests pass on supported Windows filesystems.
- Referenced, shared, unresolved, and unverified resources are proven preserved by tests.
- The GUI has no generic shell or unrestricted filesystem capability.
- Production CSP is enabled and tested.
- No remote scripts or privileged remote content are loaded.
- Downloads are staged, bounded, scheme-validated, and never automatically executed.
- Archive extraction prevents traversal and decompression bombs.
- Provider process invocation avoids shells and current-directory hijacking.
- Privileged work is narrow and explicit; the primary process remains unelevated.
- Imported manifests cannot encode executable commands or deletion approvals.
- Database integrity failure disables destructive actions.
- Logs and shared exports pass redaction tests.
- Dependencies and CI actions pass supply-chain review.
- Release artifacts have checksums, an SBOM, and an authenticated publisher path.
- `SECURITY.md` and security limitations are published.
- An external or independent security-focused review of removal and elevation code is completed before broad stable distribution.

## 21. Residual risks to communicate

Even after controls are implemented:

- Casklet does not sandbox installed software.
- A valid signature identifies a signer; it does not guarantee harmless software.
- Arbitrary installers may create changes Casklet cannot discover or remove.
- System package providers and their repositories remain part of the trust chain.
- A same-user or administrator compromise can tamper with Casklet and its data.
- Recycle Bin behavior and recoverability vary by filesystem and target.
- Storage estimates for installed/shared applications can be incomplete.
- Casklet may retain uncertain content rather than reclaim all possible space.

These should appear in product documentation and relevant UI—not only in legal text.

## 22. Decisions proposed for approval

1. Treat preservation of unrelated data as Casklet's highest security objective.
2. Use current external revalidation plus recorded provenance and associations for destructive authorization.
3. Treat the WebView, manifests, paths, providers, installers, downloads, and provider output as untrusted.
4. Keep the main process unelevated and avoid a persistent privileged service in v0.1.
5. Never expose free-form shell or uninstall command execution.
6. Bundle frontend assets locally with strict CSP and minimal Tauri capabilities.
7. Block active preview of untrusted HTML/SVG in v0.1.
8. Use staging, resource ceilings, and traversal protection for downloads and archives.
9. Treat signatures and hashes as evidence with clearly limited meaning.
10. Enter non-destructive recovery mode when database integrity is uncertain.
11. Require locked dependencies, pinned CI actions, checksums, SBOM, and protected signing/update workflows.
12. Require an independent security-focused review before broad stable distribution.

## Completion

Approved on 2026-09-14. Phase 7 now defines the proposed repository structure and exact engineering toolchain in `docs/engineering/repository-and-tooling.md`. The consolidated diagrams in `docs/architecture/system-design-v0.1.md` incorporate these security boundaries and must remain consistent with this threat model.
