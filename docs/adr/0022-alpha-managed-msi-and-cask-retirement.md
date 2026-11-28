# ADR 0022 — Managed MSI cleanup and storage retirement

Status: Accepted under the October 2 Alpha follow-up request.

## Decision

Introduce a supported Windows Installer adapter, separate from arbitrary EXE correlation. A new managed MSI claim requires explicit confirmation, a successful MSI launch with exact ProductCode, a durable pre-launch absence check, a single exact matching candidate, and unchanged current registration. Old launches without baseline evidence, existing installations, failed/reboot/uncertain launches, ambiguous candidates, and EXE results remain references. Registration and launch alone never grant uninstall ownership.

Application cleanup has an inspectable, expiring, hash-bound plan and separate approval/execution. Revalidate current registration and active dependents before approval and each uninstall. Run only system msiexec with typed `/x`, exact validated ProductCode, and `/norestart`; never evaluate registry uninstall strings. Journal intent before process launch, verify disappearance afterwards, retain locks and uncertain results across restart, and require explicit preserved closure. File cleanup follows application cleanup, with its own current plan. Alpha enables the MSI adapter explicitly; WinGet remains independently gated. Arbitrary EXE uninstall remains unsupported and clearly explained before launch.

Retiring an empty Cask commits its audit tombstone first, preventing new associations/intake. Move its metadata to retired-casks outside active storage, remove only directories verified empty, and preserve unexpected data, links, staging, or quarantine. Return a storage warning independently of successful logical removal. Never recursively delete Cask storage. Removal events invalidate the library cache immediately; older asynchronous query results cannot reintroduce it.

Activity clearing is a persisted display preference keyed by operation ID and recorded update time. It hides completed entries without deleting durable journals or hiding active/recovery work. Updated entries become visible again. Technical installer metadata remains in collapsed diagnostics. Removal uses a native dialog with safe initial focus.

## Limits

No automatic legacy ownership migration, arbitrary EXE uninstall, shared-product deletion, or independent security certification. MSI packages may perform their own custom actions; normal Windows prompts/UAC remain visible. Test real MSI/UAC and reboot scenarios in an isolated machine before public distribution.
