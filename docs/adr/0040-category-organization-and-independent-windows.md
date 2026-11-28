# ADR 0040: Category organization, batch intake and independent windows

Status: accepted by the user's October 24, 2026 request.

## Resource presentation

Overview replaces measurement diagnostics with a collapsed category storage disclosure. Files & folders uses expandable groups with the same category assignments, counts and size projection. Only used categories appear; custom names are resolved from the existing category list and unassigned items fall back to Other. The user-requested readability refinement supersedes the original expanded default: groups start collapsed on each fresh entry. Manual expansion remains during mounted-view updates; focus return after recategorization may reveal its destination group. Native details preserve keyboard behavior. All Cask Settings disclosures start collapsed each time the view mounts; drafts and save commands remain intact.

Grouping uses saved ResourceView metadata only, excludes installer artifacts/portable application resources like the existing file list, and never enumerates folders. A separately registered descendant remains visible and counted as an item, but its bytes are attributed to its containing folder instead of counted twice. Unknown/offline/incomplete sizes remain explicitly unmeasured or partial; zero is not substituted for unknown data. Category totals describe files/folders, while the overall summary also includes application sizes. Reassignment, intake, removal and explicit size refresh update the shared reactive projection. Folder descendants retain their existing inspection behavior rather than becoming separate inventory entries.

StorageSummary gains an additive, defaulted applicationSizes list containing source key, nullable bytes and estimated status from the same evidence-bound cached snapshots already used for totals. Application rows resolve Windows/WinGet sizes through that key and portable sizes through the existing resource ID. Unknown and estimated values are labelled. This adds no schema, automatic measurement or filesystem work to opening/navigation. CLI JSON receives an additive field; existing text output and cleanup evidence are unchanged.

## Selection-wide intake

File and folder pickers enable the native dialog's multiple option. Tauri drag/drop passes its entire selection. One review applies explicit ownership/reference consent and automatic categorization to each selected item through the existing typed core commands. Core validation, deduplication, existing-association policy, manifests and journals remain authoritative. The UI serializes per-item work, prevents concurrent submissions, reports item progress, counts added/duplicate-or-existing/failed entries, and continues after individual errors. Failed paths and reasons remain visible and form the retry selection; successful entries are not retried. No implicit Copy/Move or ownership upgrade is introduced.

Explorer's native COM extension already captures up to 32 selected file/folder paths with escaped arguments and supports Owned/Reference batches. Retain that structure and limit. The missing routed GUI path previously acknowledged/reviewed one queued item at a time: pending requests now expose the entire batch and acknowledgement consumes the visible batch. A mixed selection is resource intake, never a sequence of automatically launched installers. Missing/inaccessible members are validated by per-item intake rather than preventing all members from reaching review. The direct path retains its shared-core loop and now reports already included identities separately. Single-installer selection remains the existing guarded installation flow.

## Window lifecycle root cause and fix

Independent native Cask windows were undermined by an explicit main-window Destroyed handler that iterated through every Cask window and destroyed it. This application cleanup survived the earlier removal of native window ownership. A second assumption in background-job completion could exit the process after main closure despite live Cask windows.

Remove the destructive window loop. Main closure updates only main-window readiness/presentation flags. Background completion exits only when no interactive main UI, no Cask windows and no jobs remain. Normal last-window exit still waits for active background jobs. A subsequent ordinary launch rebuilds the missing main window from its existing configuration and readiness gate; existing labelled Cask windows are focused/reused. Cask creation can anchor to another live Cask if main is absent. Closing an individual Cask requeues its pending Explorer review and emits library invalidation only to a currently live main window. No stale main-window handle is retained for focus or mutation.

## Validation and limits

Regression coverage includes category reassignment/removal, nested resources, unknown sizes, batch duplicate identities, continued partial failure, automatic per-item category requests, multiple file/folder picker options, Settings remount defaults, routed Explorer batch acknowledgement, existing native COM argument/selection contracts and cached per-application sizes with a filesystem-forbidden query adapter.

The isolated real desktop smoke uses disposable files and a synthetic native Tauri drop event: four references added, one duplicate skipped, one missing item reported, images/videos/folders categorized correctly, and explicit small-fixture size measurement. It closes main, inspects a resource in the surviving Cask, relaunches main without duplicate Cask windows, and confirms final process exit. Production fixture audits cover both themes, narrow/reduced-motion layouts, modal details and keyboard disclosure toggles. Evidence and final check results are summarized in `docs/engineering/category-batch-ux.md`.

Physical Explorer right-click interaction, real multi-selection picker UI, OS drag gestures, full screen-reader acceptance, clean-machine installer validation and hosted CI remain distinct manual/external checks. No release or independent security approval is implied.
