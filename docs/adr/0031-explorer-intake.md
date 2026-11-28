# ADR 0031: Explorer intake through the installed desktop shell

Status: Accepted for the requested Explorer integration, 2026-10-19.

## Decision

Keep the per-user NSIS distribution. Register a small native `IShellExtInit`/`IContextMenu` adapter under HKCU Classes for files and directories. On Windows 11 it appears under **Show more options**. The primary Windows 11 surface requires `IExplorerCommand` plus package identity and remains outside this NSIS integration. Do not alter Explorer's global behavior or introduce a package trust workaround.

Native testing rejected the initial static cascading implementation: registry fixtures passed, but Explorer did not display the cascade on this host. The final adapter explicitly renders the native submenu. It reads at most six records from an atomic disposable registry projection, captures Explorer's `CF_HDROP` selection, and launches only the desktop executable beside its installed DLL. It never opens SQLite or performs resource/installer operations. The small C++ boundary uses Windows COM/RAII and compiles through the existing MSVC toolchain with static CRT, `/W4 /WX`, and native contract tests. Rust's unsafe-code policy is unchanged.

The infrastructure adapter projects active Cask names and stable IDs after committed shared-repository Cask mutations, including CLI changes, recovery, and startup reconciliation. Use the core's creation-time/ID ordering. Publish at most five destinations; add More casks when necessary and a chooser/create entry when empty. Publish name/ID pairs together in one atomic bounded REG_MULTI_SZ value. A file lock serializes CLI/GUI projection outside database transactions. Projection failure cannot roll back a successful database mutation; the native adapter falls back to the chooser when the projection is unavailable. Menu construction reads the latest projection each time, without a database or application process in Explorer.

The installed executable receives `--explorer <cask-id|choose> -- <absolute-path>...`. The native boundary encodes Windows arguments explicitly and uses `CreateProcessW` with an explicit executable, without a command interpreter or configurable execution target. MSI/EXE recognition, ownership and mutations occur only after launch in the existing review workflows. Use the official Tauri single-instance plugin before opening the shared service; secondary launches forward intent to the first process. Debug and installed instance identities differ. Invalid contracts, device paths, and oversized requests are rejected visibly.

The primary process retains at most 32 review intents in memory, grouped by launch and scoped to their destination window and acknowledged explicitly by the UI. Polling reads the queue so event timing cannot lose startup requests. An unconsumed request returns to the main chooser if its workspace closes. Closing the whole application cancels uncommitted review intents; they are not resource operations and need no operation journal. All actual mutations retain existing core journals, validation, permissions, locks, and revalidation.

A direct destination bypasses destination selection. Multi-selection chooses one destination for the batch, then reviews items individually through existing consent. Admission and chooser cancellation are atomic for the batch. The searchable chooser lists every active destination by name, refreshes automatically on opening/focus and periodically while visible, and supports cancellation. When no active Casks exist, intake opens the shared Create Cask form directly. Committed creation routes the retained batch to the new stable ID without another destination choice; routing failure retains that destination for retry without another creation. Ownership/reference or installer consent still follows in the destination window. Storage paths and a manual Refresh control are omitted from the chooser. Archived Casks must be restored before intake. Missing/deleted destinations permit choosing another Cask. Source metadata is checked on a worker without holding the queue lock, then checked again by existing core commands when committing.

Workspace intake reuses the existing ownership/reference review, categories, consent, synchronization and storage logic. A file with a case-insensitive MSI/EXE extension offers Install Application or Add Installer to Cask. Directories with those extensions remain ordinary folders. Install only inspects and populates the existing application review; the existing Install action is still required. Explicit installer elevation resets for each Explorer selection. Adding the installer file never invokes installer inspection/execution and never grants application ownership.

## Supported boundary

Windows 11 x64 is the release target. The classic COM interfaces are Windows-supported; older Windows versions are not newly supported or release-tested. Up to 32 filesystem selections are supported, including files and folders from different locations. Larger selections display a disabled explanatory menu entry. Windows' 32,767-character process command-line limit can require selecting fewer long paths, with a visible error. Folder inheritance remains the existing core behavior. The primary Windows 11 menu still requires a separately signed identity package and a different command surface.

The DLL filename includes the product version and a source content identifier, so upgrades register the new DLL without overwriting a library loaded by Explorer. Uninstall unregisters first and schedules a locked DLL for removal with NSIS `/REBOOTOK`; Windows may require a restart to remove that last binary. No Explorer restart or icon-cache deletion is forced. Operational metadata and resource content survive uninstall. Reinstall repairs registration.

## Sources

- [Microsoft: context menu handlers](https://learn.microsoft.com/en-us/windows/win32/shell/context-menu-handlers)
- [Microsoft: IContextMenu](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-icontextmenu)

- [Microsoft: static cascading menus (investigated alternative)](https://learn.microsoft.com/en-us/windows/win32/shell/creating-static-cascading-menus)
- [Microsoft: ExtendedSubCommandsKey](https://learn.microsoft.com/en-us/windows/win32/shell/how-to-create-cascading-menus-with-the-extendedsubcommandskey-registry-entry)
- [Microsoft: Windows 11 packaged context menus and sparse package identity](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/integrate-packaged-app-with-file-explorer)
- [Tauri: single-instance lifecycle](https://v2.tauri.app/plugin/single-instance/)

Validation and remaining release checks are recorded in `docs/engineering/explorer-integration.md`.
