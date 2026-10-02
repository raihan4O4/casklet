# Casklet user guide and safety

Casklet groups an activity's files and applications in a Cask. Create a Cask in the desktop app or with `casklet cask create <name>`. A Cask has a stable ID; its name can be reused. Development builds store data under `.casklet-dev`; release builds use `%LOCALAPPDATA%\\CaskletData` for the database and `%USERPROFILE%\\Casklets` for managed files.

## Add and inspect resources

The fixed main window opens to **Casks**, with search, sortable inventory, and **New Cask**. Selecting a row opens a separate desktop workspace; selecting it again focuses that workspace. Use its back arrow or close control to return to the inventory. Both windows provide minimize and close controls without maximize.

Each workspace has **Overview**, **Files & Folders**, **Applications**, **Activity**, and **Settings**. The bottom action bar has **Add Items** and **Cleanup**. Cleanup opens the cleanup screen. Cask Settings edits the name and description and can archive a Cask without removing its resources. Global sidebar Settings provides database backup and diagnostics; global Activity includes interrupted operations and manifest recovery. Installed applications are discovered through the Installations page and associated from a Cask's Applications tab.

Storage summaries show known managed logical sizes, not invented volume capacity or installed-application measurements. Shared labels describe multiple active associations and never imply cleanup ownership.

Choose or drop one file or folder. Review **Reference** or **Manage at current location** before pressing **Add resource**. Reference leaves it in place and never grants deletion ownership. Copy and Move are temporarily disabled; existing managed resources and their recovery remain supported. Manage at current location leaves data where it is and records explicit cleanup authority after you confirm that it is dedicated to this Cask and is not needed elsewhere. Broad folders, links, application state, and overlapping registered resources cannot be adopted. Use Reference when ownership is uncertain. Resource rows show names and sizes; expand a row for location, management, cleanup authority, availability, and category. Details stay collapsed after adding an item. Storage values distinguish managed, referenced, physical, and reclaimable estimates; unknown values are shown as unknown.

Portable ZIPs are inspected before installation. WinGet packages require explicit review. Existing Windows applications are associated as references only. A registered MSI/EXE installer is copied and inspected; launching it is an explicit separate action. A resulting Windows registration is a candidate for review, never automatic ownership.

## Export, recovery, and cleanup

Export portable JSON for sharing or a readable report. Exports contain paths and application details, so inspect before sharing. The recovery area lists interrupted operations; inspect source, staging, and destination before choosing recovery or **Keep files and close**. Use **Back up database metadata** in Settings or `casklet diagnostics backup` to make a consistent local SQLite backup. It contains metadata only, never managed files. If startup reports database corruption, preserve the database and its backup and use local manifests for conservative recovery; do not replace the damaged file in place.

Use the Archive checkbox in Cask Settings to hide a Cask from active work and later restore it. Archive preserves files, associations, and history. Open **Cleanup**, then use **Remove Cask** to permanently remove an empty Cask after confirmation. Remaining files, application dependencies, or unfinished work block removal. Removal retains audit/recovery metadata but never deletes resource bytes. CLI equivalents are `casklet cask archive <id>` and `casklet cask restore <id>`.

The bottom **Cleanup** button opens the cleanup screen; it never creates a plan. Select **Whole Cask** or a category and a visible **Deletion method**, then **Create cleanup plan**. Changing the scope or deletion method clears the preview. Review item names and effects. **Review Cleanup** marks the exact preview **Reviewed**. In enabled builds, continue with **Approve Cleanup**, then confirm **Execute Cleanup**; permanent deletion needs an additional confirmation. Default builds show **Review only** up front and explain disabled approval/execution controls. Paths, IDs, reasons, and recovery evidence are in **Advanced details**. Creating a plan changes no files. Recent cleanups reload durable plans and reset review/confirmations. Interrupted plans reappear after reopening the Cask; inspect recovery locations before explicitly keeping unresolved items and closing the run. Closing does not restore quarantined data. Equivalent current plans are reused; review, approval, and execution are separate steps. Cleanup leaves the Cask intact until you separately confirm **Remove Cask**, which becomes available when no dependencies remain. Application associations and uninstall plans are reviewed in Applications. CLI removal uses `casklet cask remove <id> --yes`; inspect blockers with `casklet cask removal-status <id>`. CLI cleanup commands use `casklet cleanup create|list|get|approve|execute|close-uncertain`; `plan` remains an alias. Referenced, shared, changed, and uncertain items are preserved. Approved plans are revalidated immediately before execution. Recycle Bin recovery depends on the volume and Windows settings. Permanent deletion cannot be undone. Default builds keep destructive cleanup disabled. The Alpha profile enables file cleanup; WinGet uninstall remains disabled pending independent security review.

To manage existing data without relocation, use `casklet resource adopt <cask-id> <path> --confirm-ownership`. External owned data supports Recycle Bin cleanup only. Back up resource bytes separately from database metadata. Machine manifests retain external paths; portable exports omit them. Manifest recovery restores unverified references, never deletion authority.

## Privacy and limitations

Casklet works locally without an account, cloud sync, or default telemetry. It stores paths, application metadata, and operation history on this machine. Diagnostics and exports can expose private paths; review them before sharing. Casklet does not sandbox applications, trace arbitrary installers, or guarantee complete cleanup of an application's external changes. Signed installers can still be harmful. Inaccessible or ambiguous items remain for manual review.

## Troubleshooting

- If a Cask or resource is missing, check the recovery area and manifest scan before changing files manually.
- If an operation is uncertain, inspect all listed paths; preservation is the safe choice when identity cannot be confirmed.
- If WinGet cannot find or verify a package, refresh provider inventory and verify the exact package ID and version.
- If startup rejects the database, retain `%LOCALAPPDATA%\\CaskletData\\db` and `backups`; a fresh database must not be created over a corrupt one automatically.
- Use **Save diagnostics** when reporting a failure, then inspect the bundle for private paths before sharing.
