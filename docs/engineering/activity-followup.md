# Activity and cancellation follow-up

Activity belongs exclusively to the main window. Cask windows retain Overview, Files & Folders, Applications, Settings and cleanup. Clear history hides terminal outcomes by ID and update timestamp in the existing persisted display preference; it never deletes audit journals or hides active/recovery work. The previous filter checked COMPLETE while core resource operations return SUCCEEDED.

The existing operation query now includes installer journals, with an INSTALL_APPLICATION step. Installer records stay separate from file recovery: they cannot be recovered or cancelled through file-operation handlers. Details resolve current Cask names and describe outcomes; removed Casks have an explicit fallback. Genuine pending installer outcomes link to the Cask window.

Windows cancellation exit 1602 is terminal only after MSI completion or verified EXE process-tree completion plus complete post-inventory. It grants no application ownership and makes no rollback claim. Persistence uses the existing CLOSED_PRESERVED journal state with INSTALLER_CANCELLED_CONFIRMED evidence; query DTOs expose CANCELLED. The transaction releases installer locks atomically. Other provider errors, timeouts, killed-process cancellation and missing inventory remain uncertain. No schema migration or retrofit of old uncertain evidence is performed.

Focus-visible covers buttons, links, summaries, inputs, selects, textareas and explicit tab stops with existing theme tokens. Pointer focus suppresses browser outlines; input borders/rings and intentional workflow focus remain themed. Existing checkbox accent colors are retained. Tap highlights are suppressed.

Native visual verification used an isolated profile and a real resource-adoption activity. Light/dark Activity details, enabled-to-disabled Clear history, immediate empty state, visible keyboard focus, and the four-tab Cask layout were inspected through the computer-use skill. No user database or installed applications were modified. Synthetic installer regressions cover successful ownership/storage and confirmed cancellation; real third-party installer/UAC cancellation remains a release test.

Native testing also exposed rejected dotted Tauri event names. Internal Cask-change, removal, operation-start and progress events now use valid hyphenated names consistently on both sides. The resource-adoption progress payload now matches the existing tuple listener. GUI mocks validate the same event-name character restriction so lifecycle notifications cannot silently regress.

Final verification: cargo xtask check passed strict Clippy, formatting, 119 all-feature Rust tests (one benchmark ignored), default/Alpha checks, 41 GUI tests, frontend lint/types/contracts and builds. Evidence: target/activity-quality-complete.log. The rebuilt native application and main Activity loaded without the former event-subscription rejection; evidence: target/activity-native-events.log.
