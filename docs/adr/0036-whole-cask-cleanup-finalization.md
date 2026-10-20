# ADR 0036 - Whole-Cask cleanup finalization

Status: Accepted for the October 21 whole-Cask cleanup request. Supersedes the contents-only Whole Cask behavior in ADRs 0020 and 0028.

## Decision

Whole Cask review now includes permanent removal of the Cask after all approved application and resource phases complete. Existing ownership, verification, build capabilities, phase hashes, removal plans and durable journals remain authoritative. Application cleanup precedes file cleanup; partial or uncertain phases stop finalization. References detach without deleting their external data.

Schema 18 adds an immutable intent binding the exact component plan IDs to this consequence. The shared core validates that binding on every query, approval and execution. Old saved contents-only plans acquire no removal intent. The UI includes the intent ID in its reviewed hash and explicitly explains and confirms Cask removal.

Finalization reuses the existing revision-checked, atomic remove_cask command. Remaining resources, installations or unfinished operations keep the Cask available with an actionable error. It never bypasses preserved dependencies or recursively deletes external paths. Removal tombstones retain audit history and remain the source of truth for completion. Concurrent finalizers converge on that tombstone. Completed phases are not replayed after restart; the persisted intent restores the same review for explicit resumption.

On confirmed removal the native shell notifies the library and destroys the matching Cask window. Frontend state clears before attempting another query of the retired Cask. Empty Casks retain the existing confirmed removal path. Category and selected-item cleanup continue to preserve their Cask.

## Presentation

Item details display the current category and offer Change Category using the ownership editor pattern. Manual edits preserve automatic intake classification and ownership. Cleanup Authority is hidden without changing domain policy. Overview replaces internal storage information with real creation and update timestamps, with calendar-based Today/Yesterday labels and full timestamp tooltips.

## Verification

Regression fixtures cover reference preservation, application-only reference detachment without uninstall, failed and preserved dependencies, restart between phases, tampered phase binding, idempotent repeated execution and concurrent finalization. GUI fixtures cover category cancel/save, metadata, explicit whole-Cask consequence, successful removal and partial failure. Existing provider, ownership, restoration and migration suites remain required. Real third-party uninstall/UAC, clean-machine packaging and independent release gates remain separate.
