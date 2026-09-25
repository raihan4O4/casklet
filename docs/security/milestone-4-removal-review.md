# Milestone 4 removal security review

Status: review requested; `reviewed-removal` is disabled in default builds.

## Scope for the independent reviewer

- Plan policy and property test: `crates/casklet-domain/src/removal.rs`.
- Approval and per-item orchestration: `crates/casklet-application/src/removal.rs`.
- Immutable snapshots, execution journal, lock, and crash detection: `crates/casklet-infrastructure/src/removal.rs` and migrations 0004–0006.
- Windows file identity, link detection, and intake provenance: `crates/casklet-infrastructure/src/resource_storage.rs` and `crates/casklet-application/src/resource.rs`.
- CLI confirmation and desktop review controls: `crates/casklet-cli/src/main.rs`, `apps/desktop/src-tauri/src/lib.rs`, and `apps/desktop/src/app/App.svelte`.
- Decision records: `docs/adr/0012-journaled-recycle-bin-cleanup.md` and `docs/adr/0013-directory-and-explicit-permanent-cleanup.md`.

## Questions that must be answered before enabling the feature

1. Can a file, symlink, junction, reparse point, hard link, or mount transition cause an item other than the approved managed file or directory to reach the Recycle Bin or permanent deletion? Examine path swaps before the quarantine rename and after the quarantine identity check. The final `trash::delete` API accepts a path, so the post-check race needs an explicit judgment or a stronger handle-based design.
2. Does the Windows identity tuple (volume serial, file index, creation time, content digest) establish the required provenance across NTFS/ReFS, file replacement, hard links, and recovery? Check file-index reuse and what happens when metadata or content changes during hashing.
3. Can concurrent intake, categorization, recovery, or another removal mutate a Cask while a run holds its lock? Verify all mutation entry points and SQLite transaction boundaries.
4. At every crash boundary—before quarantine, after quarantine, during Recycle Bin handoff, and before the success transaction—does the journal state accurately distinguish succeeded, preserved, and unknown work? Confirm the retained file can be located and inspected before an uncertain run is closed.
5. Does whole-Cask completion preserve enough association and resource history for audit and future recovery? Verify that `DESTROYED` is never assigned while unresolved owned resources remain.
6. Are CLI `--yes`/`--no-input` semantics and desktop typed-ID confirmation sufficient for the exact approved plan, including stale, expired, or tampered snapshots?
7. Does the directory tree fingerprint reject nested changes, links, junctions, and partial scans? Could a child change after the tree check but before the path-based Recycle Bin handoff?
8. Is file-only permanent deletion constrained to the explicit plan action, second confirmation, verified quarantine, and `remove_file`? Inspect the post-quarantine race and recovery after an interrupted delete.

## Existing evidence

`cargo xtask check` passes formatting, Rust lint/tests/build, and frontend format/lint/type/test/build. Automated tests cover all 1,024 domain policy combinations, replacement with identical bytes, a junction swap after approval, managed-directory tree changes before approval and execution, directory cleanup with a fake Recycle Bin adapter, explicit permanent-file confirmation and retention of an external hard link, uncertain trash with a retained lock and preserved close, and a hard exit after quarantine followed by recovery detection. Isolated Windows CLI smoke tests on 2026-09-25 succeeded with the real Recycle Bin adapter for a managed file and a managed directory. Their generated items remain in the Recycle Bin; source resources were retained and temporary workspaces removed.

## Remaining gate

Run adversarial Windows checks for hard-link swaps, post-quarantine swaps, crash/restart at each durable boundary, and Recycle Bin failures on supported filesystems. Record the independent review findings and disposition here or in a linked review. Keep `reviewed-removal` off in public builds until the reviewer accepts the destructive path. The user explicitly included managed-directory Recycle Bin cleanup and file-only permanent deletion in Milestone 4.
