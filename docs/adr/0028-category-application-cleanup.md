# ADR 0028 - Applications in category cleanup

Status: Accepted for the October 11 unified category-cleanup request.

## Decision

One cleanup selector and review handle Whole Cask, Applications and other categories. Category selection controls scope; persistent ownership controls effects. A selected file/resource never implicitly selects applications. A selected application never selects unrelated resources or other applications.

The shared core composes existing immutable resource plans and managed-application queue plans. The transport envelope contains their IDs/hashes and public views; it is a projection, not a new ownership authority or separate persisted cleanup engine. Approval and execution reload and validate the exact component journals and Cask IDs. All phases must be approved and unexpired, and permanent file deletion confirmed, before any uninstall starts.

Applications uses the existing native application queue: references detach before supported owned MSI/EXE uninstalls, which run sequentially and verify disappearance. Resources categorized as Applications (including managed installer copies and portable allocations) use their existing file ownership/provider rules. Other categories contain only their own resource snapshots. Whole Cask runs the queue before the file phase so uninstallers retain their source files. Partial/uncertain queues stop file execution and preserve its approved snapshot.

The GUI uses the existing CleanupReview and category selector for one combined preview, review, approval and execution. Component hashes bind the combined review; selection changes invalidate it. Running queue journals are polled for progress. Interrupted queues restore into that same review; inspected preserved closure permits a fresh category preview containing the remaining owners. File journals and history retain their existing recovery controls. If the process stops between successful application and file phases, the approved file snapshot remains in existing cleanup history for explicit review/resumption. Completed uninstalls are never replayed.

No schema migration, automatic Cask deletion, new uninstall mechanisms, ownership inference, or provider/build gate expansion. Shared, changed, unsupported and unverified installations remain preserved under existing rules. Existing CLI component commands remain compatible and call the same shared-core use cases.

## Verification

Core fixtures cover category isolation, owned EXE removal, reference detach with Windows registration preserved, Whole Cask application/file execution, approval/permanent guards, wrong-Cask rejection, and file-phase preservation after uninstall failure. Existing MSI/EXE ownership, queue, cancellation, evidence and reinstall regressions remain in the suite. GUI fixtures cover one selector and complete combined review/approve/execute, application-only Casks, selected-application removal and unrelated categories.

The final cargo xtask check passed strict Clippy, Rust formatting, all-feature/default/Alpha suites, desktop/CLI builds, frontend formatting/lint, zero Svelte errors or warnings, 51 GUI tests, generated-contract verification and production frontend build. The 23-test installer suite includes category and failed-queue fixtures; isolated Alpha resource cleanup/storage smoke also passed. Evidence: target/category-cleanup-quality.log, target/category-cleanup-tests.log and target/category-cleanup-smoke.log. Real third-party installer/uninstaller/UAC validation and independent publication gates remain separate.
