# ADR 0032: Explicit direct Explorer intake

Status: Accepted for the requested nested ownership/reference actions, 2026-10-20.

## Decision

Retain the Windows-supported classic IContextMenu adapter and per-user NSIS packaging from ADR 0031. Create native child popup menus synchronously for each of the first five destinations. Command offsets identify actions separately from Cask IDs; only leaf actions consume the supplied command range. More casks and the empty-library creation route retain the existing review flow. A single real EXE/MSI file also offers Install Application; mixed/multiple selections offer file-management actions only. An EXE/MSI-named directory remains a folder.

Extend the strict desktop launch contract with --explorer-owned, --explorer-reference and --explorer-install. The DLL launches only its sibling installed executable using the existing Windows argument encoder. It does not modify files or SQLite. Direct requests require a destination ID; chooser requests cannot carry direct ownership authority.

Selecting Add as Owned is explicit consent to grant the selected Cask ownership of dedicated content at its current location. Call the same AdoptResource command with confirmOwnership as the existing consent UI. No data is moved, and protected locations, incomplete scans, sharing, overlap, stable identity and Cask locks remain authoritative. References use the existing reference-only user intake command. Both retain existing journals, folder inheritance, storage queries and cleanup plans. Neither path inspects or executes installer artifacts or creates application associations.

The existing single-instance plugin forwards requests to the primary process. Direct-only startup keeps the existing native main window hidden, suppresses frontend reveal and exits when its bounded background jobs finish. A concurrent ordinary launch or installation request can reveal the initialized main window; an existing visible GUI is neither opened again nor focused by direct actions. Requests arriving during shared-service initialization are bounded and deferred until service setup completes. Closing a GUI while background work is active waits for the jobs; it does not abort a committed operation midway. No daemon, service, runtime privilege toggle or second application core is added.

Windows completion uses the official Tauri notification adapter compatible with the pinned shell. Errors use a native actionable message, and notification delivery failure falls back to a native completion message. Windows may suppress notifications through its own settings. A batch commits items individually and explicitly reports partial results, listing item errors for inspection before retry; it never claims atomic rollback or success for failures. Existing operation records remain authoritative. The GUI refreshes through the existing change event and polling.

Install Application routes to the selected Cask's existing installer inspection directly, bypassing only the redundant installer/file action question. Install, elevation and verified application association still require existing review. Multi-item installation is deliberately not offered; users can select one installer at a time.

## Compatibility and evidence

Windows 11 x64 remains the supported target. This uses Show more options; no global Explorer setting or undocumented registry workaround is added. Source names, Cask order, maximum five destinations, no-Cask creation and the searchable names-only chooser remain unchanged. See docs/engineering/explorer-integration.md for tests and remaining visual/clean-machine release gates.

Sources: [Microsoft QueryContextMenu](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-icontextmenu-querycontextmenu), [Tauri native notifications](https://v2.tauri.app/plugin/notification/).
